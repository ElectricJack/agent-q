//! Actual AQ08 machine authority; no caller-selected lifecycle Binding is trusted.
use super::*;
use crate::execution::authority::{self, Kind, Policy};
use aq_session_protocol::{ProfiledOffer, Validate as _};
use sqlx::{Postgres, Row, Transaction};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Anchor {
    pub node_id: Uuid,
    pub attempt_id: Uuid,
    pub scope: wire::Scope,
    pub binding: wire::Binding,
    pub principal: wire::Digest,
    pub source_access: wire::Digest,
    pub grant_id: Uuid,
}
pub(super) struct Current {
    pub anchor: Anchor,
    pub fence: engine::AuthorityFence,
    pub cancellation: Option<Uuid>,
    pub now: u64,
    live: authority::AuthorityView,
    allow_cancel: bool,
}
impl Current {
    pub(super) fn refresh(&mut self, state: &AppState) -> Result<()> {
        self.now = (state.clock)()
            .timestamp_millis()
            .try_into()
            .map_err(|_| Error::Authority)?;
        if !(self.allow_cancel && self.cancellation.is_some()) {
            self.live
                .active_at(self.now)
                .map_err(|_| Error::Authority)?;
        }
        Ok(())
    }
}
impl Owner {
    pub(super) async fn current(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        node: &AuthenticatedNode,
        attempt: Uuid,
        allow_cancel: bool,
    ) -> Result<Current> {
        let node_id = crate::worker_api::lock_current_node(tx, node)
            .await
            .map_err(|_| Error::Authority)?;
        let view =
            authority::load_attempt_locked(&self.state, tx, node_id, attempt, Policy::IdentityOnly)
                .await
                .map_err(|_| Error::Authority)?;
        let now = (self.state.clock)()
            .timestamp_millis()
            .try_into()
            .map_err(|_| Error::Authority)?;
        if view.kind != Kind::ProfiledV1 || view.identity.node_id != node_id {
            return Err(Error::Authority);
        }
        if !(allow_cancel && view.cancellation_id.is_some()) {
            view.active_at(now).map_err(|_| Error::Authority)?;
        }
        // The common authority owner has already validated these immutable rows,
        // exact execution/selection/source joins, current boot and task claim.
        let row = sqlx::query("SELECT l.launch_binding,l.launch_manifest,a.state->'offer' AS offer FROM profiled_launches l JOIN execution_attempts a USING(attempt_id) WHERE l.attempt_id=$1 FOR SHARE OF l")
            .bind(attempt).fetch_optional(&mut **tx).await?.ok_or(Error::Corrupt)?;
        let offer: ProfiledOffer =
            serde_json::from_value(row.get("offer")).map_err(|_| Error::Corrupt)?;
        if serde_json::to_value(&offer.binding)?
            != row.get::<serde_json::Value, _>("launch_binding")
            || serde_json::to_value(&offer.manifest)?
                != row.get::<serde_json::Value, _>("launch_manifest")
        {
            return Err(Error::Corrupt);
        }
        offer.validate().map_err(|_| Error::Corrupt)?;
        let binding =
            aq_admission::transport::lifecycle_binding(&offer).map_err(|_| Error::Corrupt)?;
        if binding.attempt_id != attempt
            || binding.session != view.session
            || binding.task_id != view.task_id
            || binding.project_id != view.project_id
            || view.launch_manifest_digest.as_ref() != Some(&offer.binding.launch_manifest_digest)
        {
            return Err(Error::Corrupt);
        }
        let grant_id = view.grant_id.ok_or(Error::Authority)?;
        let scope = wire::Scope {
            project_id: binding.project_id,
            workspace_id: offer.manifest.bindings.graph_id,
        };
        let principal =
            wire::canonical_digest(&("aq.wait.machine-principal.1", node_id, &binding.session))?;
        // Renewal revisions/expiry are deliberately excluded: current validity is
        // checked above on every lock acquisition, original source authority is not rebound.
        let source_access = wire::canonical_digest(&(
            "aq.wait.source-access.1",
            &scope,
            &binding,
            &offer.binding.launch_manifest_digest,
            &view.selection_digest,
            grant_id,
        ))?;
        let anchor = Anchor {
            node_id,
            attempt_id: attempt,
            scope: scope.clone(),
            binding: binding.clone(),
            principal: principal.clone(),
            source_access: source_access.clone(),
            grant_id,
        };
        let fence = engine::AuthorityFence {
            scope,
            binding,
            principal,
            source_access,
            task_cancelled: view.cancellation_id.is_some(),
        };
        let mut current = Current {
            anchor,
            fence,
            cancellation: view.cancellation_id,
            now,
            live: view,
            allow_cancel,
        };
        current.refresh(&self.state)?;
        Ok(current)
    }
    pub(super) async fn preflight(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        allow_cancel: bool,
    ) -> Result<Current> {
        let mut tx = self.transaction().await?;
        let current = self.current(&mut tx, node, attempt, allow_cancel).await?;
        tx.commit().await?;
        Ok(current)
    }
}
