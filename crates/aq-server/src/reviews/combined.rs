//! One explicit human decision over exact code AND a declared effect scope.
//! Serialized subjects are data. Only the protected owner can produce the
//! verified decision product consumed by settlement.
use super::*;
use crate::profiled_execution::settlement::effects::{self, AqEffectsSnapshot, LedgerDisposition};
use serde::Deserialize;
use subject_owner::{Profile, Subject};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CombinedProfile {
    #[serde(rename = "source_and_effects_review_v1")]
    V1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ExternalScope {
    #[serde(rename = "trusted_node_source_and_aq_reads_only_v1")]
    TrustedNodeSourceAndAqReadsOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalDisposition {
    Reconciled,
    Unresolved,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalEffect {
    pub resource: String,
    pub disposition: ExternalDisposition,
    pub reason: String,
    pub evidence: Vec<ArtifactRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalDeclaration {
    pub scope: ExternalScope,
    pub worktree_attempt_id: Uuid,
    pub known_effects: Vec<ExternalEffect>,
    pub reason: String,
    pub residual_uncertainty: String,
}
fn text(value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty()
        || value.len() > max
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
impl ExternalDeclaration {
    pub fn validate(&self, attempt: Uuid) -> Result<()> {
        if self.worktree_attempt_id != attempt || attempt.is_nil() || self.known_effects.len() > 16
        {
            return Err(invalid());
        }
        text(&self.reason, 2048)?;
        text(&self.residual_uncertainty, 2048)?;
        let mut resources = std::collections::BTreeSet::new();
        for e in &self.known_effects {
            text(&e.resource, 512)?;
            text(&e.reason, 2048)?;
            if !resources.insert(&e.resource) || e.evidence.len() > 8 {
                return Err(invalid());
            }
            for r in &e.evidence {
                r.id.validate().map_err(|_| invalid())?;
                r.version.validate().map_err(|_| invalid())?;
            }
        }
        if encode(self)?.len() > 8192 {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AqLedgerBasis {
    pub profile: ArtifactRef,
    pub digest: Digest,
    pub operation_count: u32,
    pub inspected_bytes: u64,
    pub messaging_disabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombinedSubject {
    pub schema_version: u32,
    pub profile: CombinedProfile,
    pub code: ReviewSubject,
    pub aq_ledger: AqLedgerBasis,
    pub external: ExternalDeclaration,
}
impl CombinedSubject {
    pub fn validate(&self) -> Result<()> {
        self.code.validate().map_err(|_| invalid())?;
        self.external.validate(self.code.binding.attempt_id)?;
        if self.schema_version != 1
            || !self.aq_ledger.messaging_disabled
            || self.aq_ledger.operation_count > 4096
            || self.aq_ledger.inspected_bytes > 33554432
            || self.aq_ledger.profile.digest != self.code.binding.graph_access_profile_digest
        {
            return Err(invalid());
        }
        encode(self)?;
        Ok(())
    }
    pub fn artifact(&self) -> Result<ArtifactRef> {
        self.validate()?;
        reference(
            format!(
                "aq.source-effects-review-subject/{}",
                self.code.result.report_id
            ),
            self,
        )
    }
    pub(super) fn matches_ledger(
        &self,
        result: &crate::profiled_execution::results::RecordedResult,
        ledger: &AqEffectsSnapshot,
    ) -> bool {
        ledger.disposition() == LedgerDisposition::Settled
            && self.aq_ledger.profile == result.spec.graph_access_profile
            && self.aq_ledger.digest == *ledger.digest()
            && self.aq_ledger.operation_count as usize == ledger.operation_count()
            && self.aq_ledger.inspected_bytes == ledger.inspected_bytes()
    }
}
/// Prepared actual owner reads, never deserializable or a finality receipt.
pub struct PreparedCombined {
    code: PreparedReview,
    subject: CombinedSubject,
    principal: Option<crate::profiled_execution::settlement::actor::ActorPin>,
}
impl PreparedCombined {
    pub fn subject(&self) -> &CombinedSubject {
        &self.subject
    }
}
/// Produced only by protected exact decision reads and current eligible Owner.
pub struct VerifiedCombinedDecision {
    subject: CombinedSubject,
    receipt: DecisionReceipt,
}
impl VerifiedCombinedDecision {
    pub(crate) fn subject(&self) -> &CombinedSubject {
        &self.subject
    }
    pub(crate) fn receipt(&self) -> &DecisionReceipt {
        &self.receipt
    }
}
impl Service {
    async fn prepare_combined(
        &self,
        report: Uuid,
        source: Uuid,
        external: ExternalDeclaration,
    ) -> Result<PreparedCombined> {
        let code = self.prepare(report, source).await?;
        external.validate(code.subject.binding.attempt_id)?;
        let mut tx = self.tx().await?;
        let result =
            crate::profiled_execution::results::load_locked(&self.state, &mut tx, report).await?;
        if !result.is_current_report_head {
            return Err(conflict());
        }
        let ledger = effects::inspect_aq_effects_locked(&self.state, &mut tx, &result).await?;
        if ledger.disposition() != LedgerDisposition::Settled {
            return Err(AppError::UpdatesUnavailable(
                "AQ effects require reconciliation before combined review".into(),
            ));
        }
        let subject = CombinedSubject {
            schema_version: 1,
            profile: CombinedProfile::V1,
            code: code.subject.clone(),
            aq_ledger: AqLedgerBasis {
                profile: result.spec.graph_access_profile,
                digest: ledger.digest().clone(),
                operation_count: ledger.operation_count() as u32,
                inspected_bytes: ledger.inspected_bytes(),
                messaging_disabled: true,
            },
            external,
        };
        subject.validate()?;
        tx.commit().await.map_err(db)?;
        Ok(PreparedCombined {
            code,
            subject,
            principal: None,
        })
    }
    pub async fn admit_combined(
        &self,
        actor: Uuid,
        prepared: &PreparedCombined,
        request: &DecisionRequest,
    ) -> Result<()> {
        if prepared.principal.as_ref().is_none_or(|p| p.id() != actor) {
            return Err(forbidden());
        }
        let mut tx = self.tx().await?;
        self.admit_subject_in_tx(
            &mut tx,
            actor,
            &prepared.code,
            request,
            Some(&prepared.subject),
            prepared.principal.as_ref(),
        )
        .await?;
        tx.commit().await.map_err(db)
    }
    pub async fn prepare_combined_as(
        &self,
        actor: &crate::store::User,
        report: Uuid,
        source: Uuid,
        external: ExternalDeclaration,
    ) -> Result<PreparedCombined> {
        let principal =
            crate::profiled_execution::settlement::actor::ActorPin::observe(&self.state, actor)
                .await?;
        let mut prepared = self.prepare_combined(report, source, external).await?;
        prepared.principal = Some(principal);
        Ok(prepared)
    }
    pub async fn resolve_combined_as(
        &self,
        actor: &crate::store::User,
        answer: &ResolveRequest,
        expected_subject: &ArtifactRef,
    ) -> Result<DecisionReceipt> {
        let principal =
            crate::profiled_execution::settlement::actor::ActorPin::observe(&self.state, actor)
                .await?;
        self.resolve_subject(
            actor.id,
            answer,
            Profile::SourceAndEffects,
            Some(expected_subject),
            Some(&principal),
        )
        .await
    }
    pub async fn inspect_combined_as(
        &self,
        actor: &crate::store::User,
        decision: Uuid,
    ) -> Result<serde_json::Value> {
        let principal =
            crate::profiled_execution::settlement::actor::ActorPin::observe(&self.state, actor)
                .await?;
        self.inspect_subject(
            actor.id,
            decision,
            Profile::SourceAndEffects,
            Some(&principal),
        )
        .await
    }
    pub(crate) async fn combined_completion_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        decision: Uuid,
    ) -> Result<VerifiedCombinedDecision> {
        // Immutable identity peek, then actual common authority BEFORE any
        // decision/outbox lock, including callers that did not already load B1.
        let report:Uuid=sqlx::query_scalar("SELECT s.report_id FROM human_decision_requests d JOIN review_subjects s USING(subject_digest) WHERE d.decision_id=$1").bind(decision).fetch_one(&mut **tx).await.map_err(db)?;
        crate::profiled_execution::results::load_locked(&self.state, tx, report).await?;
        let (subject, _event, receipt) = self.completion_subject_in_tx(tx, decision).await?;
        let Subject::SourceAndEffects(subject) = subject else {
            return Err(conflict());
        };
        subject.validate()?;
        let DecisionFinality::HumanAnswer {
            owner_user_id,
            choice: Choice::Approve,
            ..
        } = receipt.finality
        else {
            return Err(conflict());
        };
        if !subject
            .external
            .known_effects
            .iter()
            .all(|e| e.disposition == ExternalDisposition::Reconciled)
        {
            return Err(conflict());
        }
        let actor = crate::profiled_execution::settlement::actor::ActorPin::current_locked(
            &self.state,
            tx,
            owner_user_id,
        )
        .await?;
        let row=sqlx::query("SELECT combined_actor_digest,combined_actor_revision FROM human_decision_finality WHERE decision_id=$1 FOR SHARE").bind(decision).fetch_one(&mut **tx).await.map_err(db)?;
        if row
            .get::<Option<String>, _>("combined_actor_digest")
            .as_deref()
            != Some(actor.identity_digest()?.as_str())
            || row
                .get::<Option<String>, _>("combined_actor_revision")
                .as_deref()
                != Some(actor.revision())
        {
            return Err(forbidden());
        }
        Ok(VerifiedCombinedDecision {
            subject: *subject,
            receipt,
        })
    }
}
