//! INTERNAL verified-wait persistence. No mounted endpoint, scheduler hold,
//! WaitRegistered, physical stop or grant is produced by this owner.
use crate::{AppState, protected_storage::Keyring, worker_api::AuthenticatedNode};
use aq_wait_engine::{self as engine, wire};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;
use wire::Validate;
mod authority;
mod slots;
mod storage;
mod work;
use authority::{Anchor, Current};
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("current wait-owner execution authority unavailable")]
    Authority,
    #[error("wait identity or immutable payload conflict")]
    Conflict,
    #[error("wait not found")]
    NotFound,
    #[error("wait envelope or physical-work capacity exceeded")]
    Capacity,
    #[error("wait source profile unsupported")]
    Unsupported,
    #[error("wait storage or projection corrupt")]
    Corrupt,
    #[error("wait protected storage unavailable or invalid")]
    Protection,
    #[error("wait work is stale")]
    Stale,
    #[error("wait database failure")]
    Database(#[from] sqlx::Error),
    #[error("invalid bounded wait JSON")]
    Json(#[from] serde_json::Error),
    #[error("invalid wait contract")]
    Contract(#[from] wire::ContractError),
    #[error("wait reducer refusal")]
    Engine(#[from] engine::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone)]
pub struct Owner {
    state: AppState,
    keys: Arc<Keyring>,
    policy: engine::ResolvedPolicy,
    physical: Arc<tokio::sync::Semaphore>,
    active: Arc<std::sync::Mutex<std::collections::BTreeSet<Uuid>>>,
    fixture_sources: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegistrationReceipt {
    pub contract: String,
    pub wait_id: Uuid,
    pub operation_id: Uuid,
    pub request_digest: String,
    pub created_at_ms: u64,
    pub deadline_ms: u64,
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub registration: RegistrationReceipt,
    pub snapshot: engine::Snapshot,
    pub retired: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Original {
    request: String,
    anchor: Anchor,
    policy: engine::ResolvedPolicy,
    ids: engine::StableIds,
    graph: GraphProof,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphProof {
    revision: u64,
    edge_count: usize,
    digest: wire::Digest,
}
impl Owner {
    /// Keys/policy are trusted daemon-owned inputs, never request data. No runtime
    /// wiring is installed in this increment. Open authenticates retained records.
    pub async fn open(
        state: AppState,
        keys: Arc<Keyring>,
        policy: engine::ResolvedPolicy,
    ) -> Result<Self> {
        policy.body.validate()?;
        if wire::canonical_digest(&policy.body)? != policy.reference.digest {
            return Err(Error::Corrupt);
        }
        let (physical, active) = work::limits();
        let owner = Self {
            state,
            keys,
            policy,
            physical,
            active,
            fixture_sources: false,
        };
        owner.validate_retained().await?;
        Ok(owner)
    }
    #[cfg(feature = "test-support")]
    pub async fn open_fixture(
        state: AppState,
        keys: Arc<Keyring>,
        policy: engine::ResolvedPolicy,
    ) -> Result<Self> {
        let mut owner = Self::open(state, keys, policy).await?;
        owner.fixture_sources = true;
        Ok(owner)
    }
    async fn transaction(&self) -> Result<Transaction<'_, Postgres>> {
        let mut tx = self.state.store.pool.begin().await?;
        sqlx::query("SET LOCAL application_name='aq-wait-owner38'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout='5s'")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
