//! AQ-10 source inspection and finite eligibility planning, never admission.
//!
//! No route, database write, release, Graph command, capability or scheduler is
//! installed here. The absent applicability/effect-disposition owners remain
//! explicitly unavailable. A plan cannot clear a task claim or mint authority.
pub mod effects;
pub mod eligibility;
