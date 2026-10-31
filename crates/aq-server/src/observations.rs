//! Durable AQ22 ledger adapter with separate human and worker HTTP surfaces.
//! Trusted catalog admission remains pending; source registration is not a worker API.
use crate::{
    AppState,
    execution::authority::{self, AuthorityView, Kind, Policy as AuthorityPolicy},
};
use aq_accounting::{
    projection::Projection,
    reconcile::{self, Context, Decision, Fact, Policy, UnresolvedReason},
    wire::*,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
pub mod api;
pub mod inspection;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("observation scope or evidence refused")]
    Refused,
    #[error("observation identity conflicts")]
    Conflict,
    #[error("observation retained quota exhausted")]
    Capacity,
    #[error("observation database failed")]
    Database(#[from] sqlx::Error),
    #[error("observation authority failed")]
    Authority(#[from] authority::AuthorityError),
    #[error("invalid observation document")]
    Invalid,
}
pub type Result<T> = std::result::Result<T, Error>;
fn value<T: Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|_| Error::Invalid)
}
fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|_| Error::Invalid)
}
fn integer(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| Error::Invalid)
}

/// Constructed by the daemon's trusted catalog/native-session admission adapter,
/// never deserialized as a worker request. Account/namespace policy is owner data.
/// This checkpoint does not implement that producer or claim native qualification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerSource {
    pub binding: Binding,
    pub source_id: Uuid,
    pub generation: u64,
    pub native_session_key: Text,
    pub account_claim: Text,
    pub policy: Policy,
}
impl OwnerSource {
    fn validate(&self) -> Result<()> {
        self.binding.validate().map_err(|_| Error::Invalid)?;
        self.native_session_key
            .validate()
            .map_err(|_| Error::Invalid)?;
        self.account_claim.validate().map_err(|_| Error::Invalid)?;
        self.policy.validate().map_err(|_| Error::Invalid)?;
        if self.source_id.is_nil()
            || self.generation == 0
            || integer(self.generation).is_err()
            || serde_json::to_vec(self).map_err(|_| Error::Invalid)?.len() > 32768
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn digest(&self) -> Result<Digest> {
        Ok(Digest::of(
            serde_json::to_vec(self)
                .map_err(|_| Error::Invalid)?
                .as_slice(),
        ))
    }
    fn matches(&self, event: &ObservationRecord) -> bool {
        let p = &event.body.provenance;
        self.binding == event.body.binding
            && self.source_id == p.source_id
            && self.generation == p.source_generation
            && self.native_session_key == p.native_session_key
            && self.policy.parsers.iter().any(|x| x.parser == p.parser)
    }
}
fn node_of(binding: &Binding) -> Result<Uuid> {
    match binding.session {
        SessionIdentity::Remote { node_id, .. } => Ok(node_id),
        _ => Err(Error::Refused),
    }
}
fn verify_binding(view: &AuthorityView, binding: &Binding) -> Result<()> {
    let graph = view.graph.as_ref().ok_or(Error::Refused)?;
    let lineage =
        aq_admission::transport::task_lineage_id(view.project_id, graph.workspace_id, view.task_id)
            .map_err(|_| Error::Invalid)?;
    if view.kind != Kind::ProfiledV1
        || view.session != binding.session
        || view.project_id != binding.project_id
        || view.task_id != binding.task_id
        || view.attempt.attempt_id != binding.attempt_id
        || view.attempt.generation != binding.attempt_generation
        || lineage != binding.lineage_id
        || view.grant_id.is_none()
    {
        return Err(Error::Refused);
    }
    Ok(())
}
async fn live_node(tx: &mut Transaction<'_, Postgres>, node: Uuid) -> Result<()> {
    let revoked: Option<bool> =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM nodes WHERE id=$1 FOR UPDATE")
            .bind(node)
            .fetch_optional(&mut **tx)
            .await?;
    if revoked != Some(false) {
        return Err(Error::Refused);
    }
    Ok(())
}
/// Source admission requires a real active profiled grant. Replays after stop
/// may use only this retained exact authority; they cannot enroll new sources.
pub async fn register_source(state: &AppState, source: &OwnerSource) -> Result<()> {
    source.validate()?;
    let node = node_of(&source.binding)?;
    let mut tx = state.store.observation_pool.begin().await?;
    live_node(&mut tx, node).await?;
    let view = authority::load_locked(
        state,
        &mut tx,
        &source.binding.session,
        AuthorityPolicy::TerminalHistorical,
    )
    .await?;
    verify_binding(&view, &source.binding)?;
    // The source may not relabel the selected account/provider. Stable account
    // scope and native namespace are still supplied by the trusted owner catalog.
    let selected: Value = sqlx::query_scalar(
        "SELECT selection FROM intelligence_attempt_selections WHERE attempt_id=$1",
    )
    .bind(source.binding.attempt_id)
    .fetch_one(&mut *tx)
    .await?;
    let selected: aq_selection::ResolvedSelection = parse(selected)?;
    if source.policy.provider.as_str() != selected.choice.provider
        || source.account_claim.as_str() != selected.choice.account_reference
    {
        return Err(Error::Refused);
    }
    sqlx::query("INSERT INTO observation_nodes(node_id) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(node)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT retained_bytes,source_count FROM observation_nodes WHERE node_id=$1 FOR UPDATE",
    )
    .bind(node)
    .fetch_one(&mut *tx)
    .await?;
    let old:Option<String>=sqlx::query_scalar("SELECT authority_digest FROM observation_sources WHERE node_id=$1 AND source_id=$2 AND generation=$3").bind(node).bind(source.source_id).bind(integer(source.generation)?).fetch_optional(&mut *tx).await?;
    let digest = source.digest()?;
    if let Some(old) = old {
        if old != digest.as_str() {
            return Err(Error::Conflict);
        }
        tx.commit().await?;
        return Ok(());
    }
    authority::load_locked(
        state,
        &mut tx,
        &source.binding.session,
        AuthorityPolicy::TerminalActive,
    )
    .await?;
    let bytes = serde_json::to_vec(source)
        .map_err(|_| Error::Invalid)?
        .len() as i64;
    if row.get::<i32, _>("source_count") >= 256
        || row.get::<i64, _>("retained_bytes") + bytes > 134217728
    {
        return Err(Error::Capacity);
    }
    sqlx::query("INSERT INTO observation_sources(node_id,source_id,generation,attempt_id,authority,authority_digest) VALUES($1,$2,$3,$4,$5,$6)").bind(node).bind(source.source_id).bind(integer(source.generation)?).bind(source.binding.attempt_id).bind(value(source)?).bind(digest.as_str()).execute(&mut *tx).await?;
    sqlx::query("UPDATE observation_nodes SET source_count=source_count+1,retained_bytes=retained_bytes+$2 WHERE node_id=$1").bind(node).bind(bytes).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// `authenticated_node` must come from current node credential authentication.
/// The transaction checks revocation again after taking the node lock. This API
/// admits observations only; it never renews execution or releases a workspace.
pub async fn ingest(
    state: &AppState,
    authenticated_node: Uuid,
    packet: &Packet,
) -> Result<Receipt> {
    packet.validate().map_err(|_| Error::Invalid)?;
    let Message::Batch(batch) = &packet.body.message else {
        return Err(Error::Refused);
    };
    if authenticated_node != batch.node_id {
        return Err(Error::Refused);
    }
    let mut tx = state.store.observation_pool.begin().await?;
    live_node(&mut tx, authenticated_node).await?;
    // Stable ordering across all attempts, calls and aggregate rows in a batch.
    let sessions: BTreeMap<_, _> = batch
        .events
        .iter()
        .map(|e| (e.body.binding.attempt_id, &e.body.binding))
        .collect();
    for binding in sessions.values() {
        let view = authority::load_locked(
            state,
            &mut tx,
            &binding.session,
            AuthorityPolicy::TerminalHistorical,
        )
        .await?;
        verify_binding(&view, binding)?;
    }
    let row=sqlx::query("SELECT through_sequence,retained_bytes,event_count FROM observation_nodes WHERE node_id=$1 FOR UPDATE").bind(authenticated_node).fetch_optional(&mut *tx).await?.ok_or(Error::Refused)?;
    let old: Option<Value> = sqlx::query_scalar(
        "SELECT receipt FROM observation_packets WHERE node_id=$1 AND message_id=$2",
    )
    .bind(authenticated_node)
    .bind(packet.body.message_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(old) = old {
        let receipt: Receipt = parse(old)?;
        receipt
            .validate_against(packet)
            .map_err(|_| Error::Conflict)?;
        tx.commit().await?;
        return Ok(receipt);
    }
    if integer(batch.first_sequence)?
        != row
            .get::<i64, _>("through_sequence")
            .checked_add(1)
            .ok_or(Error::Capacity)?
    {
        return Err(Error::Conflict);
    }
    let mut sources = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for event in &batch.events {
        let p = &event.body.provenance;
        let source_key = (p.source_id, p.source_generation);
        if let std::collections::btree_map::Entry::Vacant(entry) = sources.entry(source_key) {
            let source_row=sqlx::query("SELECT authority,authority_digest FROM observation_sources WHERE node_id=$1 AND source_id=$2 AND generation=$3").bind(authenticated_node).bind(p.source_id).bind(integer(p.source_generation)?).fetch_optional(&mut *tx).await?.ok_or(Error::Refused)?;
            let source: OwnerSource = parse(source_row.get("authority"))?;
            source.validate()?;
            if source.digest()?.as_str() != source_row.get::<String, _>("authority_digest") {
                return Err(Error::Invalid);
            }
            entry.insert(source);
        }
        let source = &sources[&source_key];
        if !source.matches(event) {
            return Err(Error::Refused);
        }
        if event.body.kind.is_usage() {
            keys.insert(
                reconcile::key(event, &source.policy)
                    .map_err(|_| Error::Invalid)?
                    .as_str()
                    .to_owned(),
            );
        }
    }
    let current_attempts: BTreeSet<_> = batch
        .events
        .iter()
        .map(|e| e.body.binding.attempt_id)
        .collect();
    let mut affected_attempts = current_attempts.clone();
    let mut updated_attempts = current_attempts.clone();
    for key in &keys {
        sqlx::query(
            "INSERT INTO observation_call_heads(call_key) VALUES($1) ON CONFLICT DO NOTHING",
        )
        .bind(key)
        .execute(&mut *tx)
        .await?;
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT owner_attempt_id FROM observation_call_heads WHERE call_key=$1 FOR UPDATE",
        )
        .bind(key)
        .fetch_one(&mut *tx)
        .await?;
        if let Some(owner) = owner {
            affected_attempts.insert(owner);
        }
    }
    let lineages: BTreeSet<_> = batch
        .events
        .iter()
        .map(|e| e.body.binding.lineage_id)
        .collect();
    let mut projections = BTreeMap::new();
    for lineage in lineages {
        sqlx::query("INSERT INTO observation_projections(lineage_id,projection) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(lineage).bind(value(&Projection::default())?).execute(&mut *tx).await?;
        let p: Value = sqlx::query_scalar(
            "SELECT projection FROM observation_projections WHERE lineage_id=$1 FOR UPDATE",
        )
        .bind(lineage)
        .fetch_one(&mut *tx)
        .await?;
        projections.insert(lineage, parse::<Projection>(p)?);
    }
    let mut attempt_projections = BTreeMap::new();
    for attempt in affected_attempts {
        if current_attempts.contains(&attempt) {
            sqlx::query("INSERT INTO observation_attempt_projections(attempt_id,projection) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(attempt).bind(value(&Projection::default())?).execute(&mut *tx).await?;
        }
        let projection: Value = sqlx::query_scalar(
            "SELECT projection FROM observation_attempt_projections WHERE attempt_id=$1 FOR UPDATE",
        )
        .bind(attempt)
        .fetch_one(&mut *tx)
        .await?;
        attempt_projections.insert(attempt, parse::<Projection>(projection)?);
    }
    let recorded_at = (state.clock)().timestamp_millis();
    if recorded_at <= 0 {
        return Err(Error::Invalid);
    }
    let mut retained = 0i64;
    for event in &batch.events {
        let source = &sources[&(
            event.body.provenance.source_id,
            event.body.provenance.source_generation,
        )];
        let mut appended = None;
        let projection = projections
            .get_mut(&event.body.binding.lineage_id)
            .ok_or(Error::Invalid)?;
        let decision = if let ObservationKind::Usage { usage } = &event.body.kind {
            let key = reconcile::key(event, &source.policy).map_err(|_| Error::Invalid)?;
            let (current, owner): (Option<Value>, Option<Uuid>) = sqlx::query_as(
                "SELECT fact,owner_attempt_id FROM observation_call_heads WHERE call_key=$1",
            )
            .bind(key.as_str())
            .fetch_one(&mut *tx)
            .await?;
            let current = current.map(parse::<Fact>).transpose()?;
            let target = if let Some(reference) = &usage.correction_of {
                let v: Option<Value> = sqlx::query_scalar(
                    "SELECT fact FROM observation_facts WHERE event_id=$1 AND call_key=$2",
                )
                .bind(reference.event_id)
                .bind(key.as_str())
                .fetch_optional(&mut *tx)
                .await?;
                v.map(parse::<Fact>).transpose()?
            } else {
                None
            };
            let decision = if usage.call.account_reference != source.account_claim {
                Decision::Unresolved {
                    reason: UnresolvedReason::AttributionChanged,
                }
            } else {
                match reconcile::decide(
                    event,
                    Context {
                        lineage: source.binding.lineage_id,
                        policy: &source.policy,
                        current: current.as_ref(),
                        correction_target: target.as_ref(),
                        baseline: None,
                        pricing_reference: None,
                    },
                ) {
                    Ok(v) => v,
                    Err(aq_accounting::Error::Conflict | aq_accounting::Error::Unsupported) => {
                        Decision::Unresolved {
                            reason: UnresolvedReason::AttributionChanged,
                        }
                    }
                    Err(_) => return Err(Error::Invalid),
                }
            };
            match &decision {
                Decision::Append { fact, .. } => {
                    *projection = projection
                        .replace_fact(current.as_ref(), fact)
                        .map_err(|_| Error::Capacity)?;
                    let owner = owner.unwrap_or(event.body.binding.attempt_id);
                    updated_attempts.insert(owner);
                    let owned = attempt_projections.get_mut(&owner).ok_or(Error::Invalid)?;
                    *owned = owned
                        .replace_fact(current.as_ref(), fact)
                        .map_err(|_| Error::Capacity)?;
                    sqlx::query("UPDATE observation_call_heads SET fact=$2,owner_attempt_id=$3 WHERE call_key=$1")
                        .bind(key.as_str())
                        .bind(value(fact)?)
                        .bind(owner)
                        .execute(&mut *tx)
                        .await?;
                    appended = Some((key, fact.clone()));
                }
                Decision::Unresolved { .. } => {
                    projection.unresolved_events = projection
                        .unresolved_events
                        .checked_add(1)
                        .ok_or(Error::Capacity)?;
                    let owned = attempt_projections
                        .get_mut(&event.body.binding.attempt_id)
                        .ok_or(Error::Invalid)?;
                    owned.unresolved_events = owned
                        .unresolved_events
                        .checked_add(1)
                        .ok_or(Error::Capacity)?;
                }
                Decision::NoCharge { .. } => {}
            }
            value(&decision)?
        } else {
            if matches!(event.body.kind, ObservationKind::Gap { .. }) {
                projection.unresolved_events = projection
                    .unresolved_events
                    .checked_add(1)
                    .ok_or(Error::Capacity)?;
                let owned = attempt_projections
                    .get_mut(&event.body.binding.attempt_id)
                    .ok_or(Error::Invalid)?;
                owned.unresolved_events = owned
                    .unresolved_events
                    .checked_add(1)
                    .ok_or(Error::Capacity)?;
            }
            json!({"kind":"observation_only"})
        };
        retained = retained
            .checked_add(serde_json::to_vec(event).map_err(|_| Error::Invalid)?.len() as i64)
            .and_then(|v| v.checked_add(decision.to_string().len() as i64))
            .ok_or(Error::Capacity)?;
        sqlx::query("INSERT INTO observation_events(event_id,node_id,sequence,lineage_id,attempt_id,record,decision,recorded_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(event.event_id).bind(authenticated_node).bind(integer(event.body.sequence)?).bind(event.body.binding.lineage_id).bind(event.body.binding.attempt_id).bind(value(event)?).bind(decision).bind(recorded_at).execute(&mut *tx).await?;
        if let Some((key, fact)) = appended {
            retained = retained
                .checked_add(
                    (serde_json::to_vec(&fact).map_err(|_| Error::Invalid)?.len() * 2) as i64,
                )
                .ok_or(Error::Capacity)?;
            sqlx::query("INSERT INTO observation_facts(event_id,call_key,fact) VALUES($1,$2,$3)")
                .bind(event.event_id)
                .bind(key.as_str())
                .bind(value(&fact)?)
                .execute(&mut *tx)
                .await?;
        }
    }
    let receipt = Receipt {
        schema_version: SCHEMA_VERSION,
        message_id: packet.body.message_id,
        body_digest: packet.body_digest.clone(),
        disposition: ReceiptDisposition::ObservationRecorded,
        through_sequence: Some(batch.last_sequence),
    };
    receipt
        .validate_against(packet)
        .map_err(|_| Error::Invalid)?;
    retained = retained
        .checked_add(
            serde_json::to_vec(&receipt)
                .map_err(|_| Error::Invalid)?
                .len() as i64
                + 4096 * (projections.len() + attempt_projections.len()) as i64,
        )
        .ok_or(Error::Capacity)?;
    if row
        .get::<i64, _>("retained_bytes")
        .checked_add(retained)
        .is_none_or(|n| n > 134217728)
        || row.get::<i64, _>("event_count") + batch.events.len() as i64 > 65536
    {
        return Err(Error::Capacity);
    }
    for (lineage, projection) in projections {
        projection.validate().map_err(|_| Error::Invalid)?;
        sqlx::query("UPDATE observation_projections SET revision=revision+1,projection=$2,last_recorded_at_ms=GREATEST(last_recorded_at_ms,$3) WHERE lineage_id=$1").bind(lineage).bind(value(&projection)?).bind(recorded_at).execute(&mut *tx).await?;
    }
    for (attempt, projection) in attempt_projections {
        if !updated_attempts.contains(&attempt) {
            continue;
        }
        projection.validate().map_err(|_| Error::Invalid)?;
        sqlx::query("UPDATE observation_attempt_projections SET revision=revision+1,projection=$2,last_recorded_at_ms=GREATEST(last_recorded_at_ms,$3) WHERE attempt_id=$1").bind(attempt).bind(value(&projection)?).bind(recorded_at).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO observation_packets(node_id,message_id,body_digest,receipt) VALUES($1,$2,$3,$4)").bind(authenticated_node).bind(packet.body.message_id).bind(packet.body_digest.as_str()).bind(value(&receipt)?).execute(&mut *tx).await?;
    sqlx::query("UPDATE observation_nodes SET through_sequence=$2,retained_bytes=retained_bytes+$3,event_count=event_count+$4 WHERE node_id=$1").bind(authenticated_node).bind(integer(batch.last_sequence)?).bind(retained).bind(batch.events.len() as i64).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}
