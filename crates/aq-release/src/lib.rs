//! Release manifest, compatibility rules and spool file formats shared by
//! the server and the host updater. Pure data and logic: no I/O, no runtime.

pub mod manifest;
pub mod spool;

pub use manifest::{
    ImageRefs, ManifestError, ProtocolRange, Refusal, ReleaseManifest, UpgradePlan, check_upgrade,
};
pub use spool::{
    HistoryEntry, Outcome, ReadinessReport, ReleaseSummary, Step, UpdateRequest, UpdateState,
    UpdateStatus,
};
