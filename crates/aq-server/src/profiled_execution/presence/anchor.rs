use super::changed;
use crate::{
    error::AppError,
    profiled_execution::{initial_grant::OriginalGrant, renewal},
};
use aq_admission::{
    LaunchBinding,
    scoped_presence::{AuthorityAnchor, AuthorityKind},
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
    grant: Uuid,
    anchor: &AuthorityAnchor,
    original: &OriginalGrant,
) -> Result<renewal::Issuance, AppError> {
    match anchor.kind {
        AuthorityKind::InitialGrant => {
            if anchor.operation_id != original.operation_id
                || anchor.grant_revision != original.versions.grant_revision
                || anchor.expires_at_ms != original.body.grant.expires_at_ms
            {
                return Err(changed());
            }
            Ok(renewal::Issuance {
                issued_at_ms: original.body.clock.server_time_ms,
                expires_at_ms: original.body.grant.expires_at_ms,
                execution_deadline_ms: original.execution_deadline_ms,
            })
        }
        AuthorityKind::Renewal => renewal::issuance_locked(tx, node, binding, grant, anchor).await,
    }
}
