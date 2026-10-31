//! Decision authority is retained beside immutable Decision JSON. Scheduled
//! selection derives authority from installed daemon policy, never a user UUID.
use super::resolver::Decision;
use super::*;
use crate::profiled_execution::config::ProjectPolicyRecord;
use aq_selection::{ArtifactRef, Digest};
use sqlx::{Postgres, Transaction};

#[derive(Clone)]
pub(super) enum Principal {
    Human(Uuid),
    Policy {
        project: Uuid,
        reference: ArtifactRef,
        revision: u64,
    },
}
impl Principal {
    pub fn human(actor: Uuid) -> Result<Self> {
        if actor.is_nil() {
            Err(bad("invalid decision principal"))
        } else {
            Ok(Self::Human(actor))
        }
    }
    pub fn configured(state: &AppState, project: Uuid) -> Result<Self> {
        let config = state
            .config
            .profiled_execution
            .as_deref()
            .ok_or_else(|| conflict("profiled project policy not configured"))?;
        let record = config
            .project_policy_record(project)
            .map_err(|_| conflict("project policy unavailable"))?;
        if !record.enabled {
            return Err(conflict("project policy disabled"));
        }
        let reference = record.artifact().map_err(|_| AppError::Internal)?;
        Ok(Self::Policy {
            project,
            reference,
            revision: record.revision,
        })
    }
    fn actor(&self) -> Option<Uuid> {
        match self {
            Self::Human(id) => Some(*id),
            _ => None,
        }
    }
    fn project(&self) -> Option<Uuid> {
        match self {
            Self::Policy { project, .. } => Some(*project),
            _ => None,
        }
    }
    fn reference(&self) -> Option<&ArtifactRef> {
        match self {
            Self::Policy { reference, .. } => Some(reference),
            _ => None,
        }
    }
    pub async fn authorize(
        &self,
        state: &AppState,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<()> {
        match self {
            Self::Human(actor) => {
                let role: Option<String> =
                    sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR SHARE")
                        .bind(actor)
                        .fetch_optional(&mut **tx)
                        .await
                        .map_err(db)?;
                if role.as_deref() != Some("owner") {
                    return Err(AppError::Forbidden(
                        "decision requires current owner authority".into(),
                    ));
                }
            }
            Self::Policy {
                project,
                reference,
                revision,
            } => {
                let Self::Policy {
                    reference: configured,
                    revision: configured_revision,
                    ..
                } = Self::configured(state, *project)?
                else {
                    return Err(AppError::Internal);
                };
                if reference != &configured || *revision != configured_revision {
                    return Err(conflict("configured project policy changed"));
                }
                let row=sqlx::query("SELECT revision,policy_digest,policy FROM admission_source_policies WHERE project_id=$1 FOR SHARE").bind(project).fetch_optional(&mut **tx).await.map_err(db)?.ok_or_else(||conflict("project policy has not been installed"))?;
                let installed: ProjectPolicyRecord =
                    serde_json::from_value(row.get("policy")).map_err(|_| AppError::Internal)?;
                if !installed.enabled
                    || installed.project_id != *project
                    || installed.revision != *revision
                    || row.get::<i64, _>("revision") != *revision as i64
                    || row.get::<String, _>("policy_digest") != reference.digest.as_str()
                    || installed.artifact().map_err(|_| AppError::Internal)? != *reference
                {
                    return Err(conflict("installed project policy changed or disabled"));
                }
            }
        }
        Ok(())
    }
    pub async fn lookup(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        operation: Uuid,
        payload: &Digest,
    ) -> Result<Option<Decision>> {
        let row=sqlx::query("SELECT payload_digest,decision FROM intelligence_decisions WHERE operation_id=$1 AND ((authority_kind='human_owner' AND actor_id=$2) OR (authority_kind='project_policy' AND authority_project_id=$3 AND authority_policy_digest=$4))").bind(operation).bind(self.actor()).bind(self.project()).bind(self.reference().map(|r|r.digest.as_str())).fetch_optional(&mut **tx).await.map_err(db)?;
        row.map(|row| {
            if row.get::<String, _>("payload_digest") != payload.as_str() {
                return Err(conflict("resolution operation ID conflict"));
            }
            serde_json::from_value(row.get("decision")).map_err(|_| AppError::Internal)
        })
        .transpose()
    }
    pub async fn insert(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        operation: Uuid,
        payload: &Digest,
        decision: &Decision,
        encoded: Value,
    ) -> Result<()> {
        let (kind, revision) = match self {
            Self::Human(_) => ("human_owner", None),
            Self::Policy { revision, .. } => ("project_policy", Some(*revision as i64)),
        };
        sqlx::query("INSERT INTO intelligence_decisions(decision_id,actor_id,operation_id,payload_digest,decision,authority_kind,authority_project_id,authority_policy_ref,authority_policy_revision,authority_policy_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(decision.decision_id).bind(self.actor()).bind(operation).bind(payload.as_str()).bind(encoded).bind(kind).bind(self.project()).bind(self.reference().map(serde_json::to_value).transpose().map_err(|_|AppError::Internal)?).bind(revision).bind(self.reference().map(|r|r.digest.as_str())).execute(&mut **tx).await.map_err(db)?;
        Ok(())
    }
}
/// Called while the immutable decision and project/node predicates stay locked.
pub(super) async fn fence(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    decision: &Decision,
) -> Result<()> {
    let row=sqlx::query("SELECT actor_id,authority_kind,authority_project_id,authority_policy_ref,authority_policy_revision,authority_policy_digest FROM intelligence_decisions WHERE decision_id=$1 FOR SHARE").bind(decision.decision_id).fetch_one(&mut **tx).await.map_err(db)?;
    let principal = match row.get::<String, _>("authority_kind").as_str() {
        "human_owner" => Principal::human(
            row.get::<Option<Uuid>, _>("actor_id")
                .ok_or(AppError::Internal)?,
        )?,
        "project_policy" => {
            let project = row
                .get::<Option<Uuid>, _>("authority_project_id")
                .ok_or(AppError::Internal)?;
            if project != decision.inputs.project.lineage.project_id
                || project != decision.project.project.binding.project_id
            {
                return Err(conflict("decision policy project mismatch"));
            }
            let reference: ArtifactRef = serde_json::from_value(
                row.get::<Option<Value>, _>("authority_policy_ref")
                    .ok_or(AppError::Internal)?,
            )
            .map_err(|_| AppError::Internal)?;
            let revision = row
                .get::<Option<i64>, _>("authority_policy_revision")
                .ok_or(AppError::Internal)?;
            if revision <= 0
                || row
                    .get::<Option<String>, _>("authority_policy_digest")
                    .as_deref()
                    != Some(reference.digest.as_str())
            {
                return Err(AppError::Internal);
            }
            Principal::Policy {
                project,
                reference,
                revision: revision as u64,
            }
        }
        _ => return Err(AppError::Internal),
    };
    principal.authorize(state, tx).await
}
