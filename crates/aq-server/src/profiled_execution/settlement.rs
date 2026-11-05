//! Actual finite settlement owners and data-only eligibility evaluation.
//! No router, scheduler or public capability is enabled by module registration.
//! Schema39 and positive owner qualification remain explicit rollout gates.
pub(crate) mod actor;
pub(crate) mod applicability;
pub mod effects;
pub mod eligibility;

mod automatic;
mod completion;
mod default_policy;
mod owner;
pub use automatic::{AutoDisposition, AutoSettlement, settle_after_record};

/// The review requirement an offer must name for this validation policy body:
/// D3's default completion policy is settled by the policy actor, every other
/// policy by the human owner.
pub(crate) fn review_requirement(policy_body: &str) -> aq_admission::outcome::ReviewRequirement {
    if default_policy::parse(policy_body).is_some() {
        aq_admission::outcome::ReviewRequirement::PolicyActor
    } else {
        aq_admission::outcome::ReviewRequirement::HumanOwner
    }
}
pub use completion::{CompletionReceipt, CompletionStatus};
pub use owner::{EvaluateRequest, EvaluationDisposition, EvaluationReceipt, Service};
