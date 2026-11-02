//! Actual finite settlement owners and data-only eligibility evaluation.
//! No router, scheduler or public capability is enabled by module registration.
//! Schema39 and positive owner qualification remain explicit rollout gates.
pub mod effects;
pub mod eligibility;

mod completion;
mod owner;
pub use completion::{CompletionReceipt, CompletionStatus};
pub use owner::{EvaluateRequest, EvaluationDisposition, EvaluationReceipt, Service};
