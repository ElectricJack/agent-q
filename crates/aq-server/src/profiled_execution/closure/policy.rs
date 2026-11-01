//! Explicit trusted verifier policy, independent of active launch eligibility.
//! Installation is meta-only; requests check its revision before node locks.
use super::{canonical, conflict, database};
use crate::{AppState, error::AppError};
use aq_admission::{ArtifactRef, Digest, granted_closure::Evidence};
use aq_session_protocol::Validate;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
const KEY: &str = "profiled_launch_closure_verifier_policy_v1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Suppressed,
    HostStopped,
}
impl EvidenceKind {
    pub(super) fn of(e: &Evidence) -> Self {
        match e {
            Evidence::Suppressed { .. } => Self::Suppressed,
            Evidence::HostStopped { .. } => Self::HostStopped,
        }
    }
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Suppressed => "suppressed",
            Self::HostStopped => "host_stopped",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verifier {
    pub runtime: ArtifactRef,
    pub qualification: ArtifactRef,
    /// Operator-pinned qualification of this recovery/proof profile. Serde
    /// validity alone never qualifies a worker's actual opaque proof producer.
    pub profile: ArtifactRef,
    pub kinds: Vec<EvidenceKind>,
}
#[derive(Clone, Serialize)]
pub struct HistoricalVerifierPolicy {
    revision: u64,
    entries: Vec<Verifier>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installed {
    schema_version: u32,
    revision: u64,
    digest: Digest,
}
pub struct InstalledPolicy {
    policy: HistoricalVerifierPolicy,
    digest: Digest,
}
impl HistoricalVerifierPolicy {
    pub fn new(revision: u64, entries: Vec<Verifier>) -> Result<Self, AppError> {
        if revision == 0 || revision > i64::MAX as u64 || entries.len() > 64 {
            return Err(conflict());
        }
        let mut seen = std::collections::BTreeSet::new();
        for e in &entries {
            for r in [&e.runtime, &e.qualification, &e.profile] {
                r.id.validate().map_err(|_| conflict())?;
                r.version.validate().map_err(|_| conflict())?;
            }
            if e.kinds.is_empty()
                || e.kinds.len() > 2
                || e.kinds
                    .iter()
                    .enumerate()
                    .any(|(i, k)| e.kinds[..i].contains(k))
                || canonical(e)?.len() > 3072
                || !seen.insert(
                    aq_selection::digest(&(&e.runtime, &e.qualification))
                        .map_err(|_| AppError::Internal)?,
                )
            {
                return Err(conflict());
            }
        }
        Ok(Self { revision, entries })
    }
    /// Explicit trusted startup action; never called by an ordinary request.
    /// Identical revision/digest is idempotent. Higher revision fences old policy.
    pub async fn install(self, state: &AppState) -> Result<Arc<InstalledPolicy>, AppError> {
        let digest = aq_selection::digest(&self).map_err(|_| AppError::Internal)?;
        let mut tx = state.store.pool.begin().await.map_err(database)?;
        for sql in [
            "SET LOCAL lock_timeout='2s'",
            "SET LOCAL statement_timeout='3s'",
        ] {
            sqlx::query(sql).execute(&mut *tx).await.map_err(database)?;
        }
        let format: Option<String> =
            sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
                .fetch_optional(&mut *tx)
                .await
                .map_err(database)?;
        let ready: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=28)")
                .fetch_one(&mut *tx)
                .await
                .map_err(database)?;
        if format.as_deref() != Some(crate::STATE_FORMAT_VERSION.to_string().as_str())
            || !ready
            || crate::STATE_FORMAT_VERSION < 28
        {
            return Err(conflict());
        }
        sqlx::query("SELECT attempt_id,operation_id FROM profiled_launch_closures LIMIT 0")
            .fetch_all(&mut *tx)
            .await
            .map_err(database)?;
        // Serialize initial installation too: a missing meta row cannot be locked.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aq-profiled-launch-closure-policy-install-v1',0))").execute(&mut *tx).await.map_err(database)?;
        sqlx::query("SELECT registration_id FROM profiled_native_registration_history LIMIT 0")
            .fetch_all(&mut *tx)
            .await
            .map_err(database)?;
        sqlx::query("SELECT attempt_id FROM profiled_launch_registrations LIMIT 0")
            .fetch_all(&mut *tx)
            .await
            .map_err(database)?;
        let old=sqlx::query("SELECT CASE WHEN octet_length(value)<=4096 THEN value END AS value FROM meta WHERE key=$1 FOR UPDATE").bind(KEY).fetch_optional(&mut *tx).await.map_err(database)?;
        if let Some(row) = old {
            let text: Option<String> = row.get("value");
            let old: Installed =
                serde_json::from_str(&text.ok_or_else(conflict)?).map_err(|_| conflict())?;
            if old.schema_version != 1
                || old.revision == 0
                || self.revision < old.revision
                || (self.revision == old.revision && digest != old.digest)
            {
                return Err(conflict());
            }
        }
        let metadata = Installed {
            schema_version: 1,
            revision: self.revision,
            digest: digest.clone(),
        };
        sqlx::query("INSERT INTO meta(key,value) VALUES($1,$2) ON CONFLICT(key) DO UPDATE SET value=EXCLUDED.value").bind(KEY).bind(String::from_utf8(canonical(&metadata)?).map_err(|_|AppError::Internal)?).execute(&mut *tx).await.map_err(database)?;
        tx.commit().await.map_err(database)?;
        Ok(Arc::new(InstalledPolicy {
            policy: self,
            digest,
        }))
    }
}
impl InstalledPolicy {
    pub(super) async fn fence(&self, tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
        let row=sqlx::query("SELECT CASE WHEN octet_length(value)<=4096 THEN value END AS value FROM meta WHERE key=$1 FOR SHARE").bind(KEY).fetch_optional(&mut **tx).await.map_err(database)?.ok_or_else(conflict)?;
        let text: Option<String> = row.get("value");
        let current: Installed =
            serde_json::from_str(&text.ok_or_else(conflict)?).map_err(|_| conflict())?;
        if current.schema_version != 1
            || current.revision != self.policy.revision
            || current.digest != self.digest
        {
            return Err(AppError::Conflict(
                "historical verifier policy is no longer current".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn verifier(
        &self,
        runtime: &ArtifactRef,
        qualification: &ArtifactRef,
        kind: &EvidenceKind,
    ) -> Result<Verifier, AppError> {
        self.policy
            .entries
            .iter()
            .find(|e| {
                e.runtime == *runtime && e.qualification == *qualification && e.kinds.contains(kind)
            })
            .cloned()
            .ok_or_else(|| AppError::Conflict("historical proof profile is not installed".into()))
    }
}
