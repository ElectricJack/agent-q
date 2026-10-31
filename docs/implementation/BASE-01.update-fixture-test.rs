// Supplemental checker; copy into the archived baseline's
// crates/aq-updater/tests/base01_fixtures.rs and set BASE01_FIXTURE to the JSON.
use aq_release::{ReleaseManifest, UpdateRequest, UpdateStatus, UpdateState};
use aq_updater::journal::{Journal, JournalError};
use serde_json::Value;

#[test]
fn retained_update_formats_parse_and_reject_incompatible_journal() {
    let f: Value = serde_json::from_slice(&std::fs::read(std::env::var("BASE01_FIXTURE").unwrap()).unwrap()).unwrap();
    let u = &f["updates"];
    let manifest = ReleaseManifest::from_json(&serde_json::to_vec(&u["manifest"]).unwrap()).unwrap();
    assert_eq!(manifest.state_format, 1);
    serde_json::from_value::<UpdateRequest>(u["request"].clone()).unwrap();
    let status: UpdateStatus = serde_json::from_value(u["status"].clone()).unwrap();
    assert!(matches!(status.state, UpdateState::Idle));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.json");
    std::fs::write(&path, serde_json::to_vec(&u["journal"]).unwrap()).unwrap();
    assert_eq!(Journal::read(dir.path()).unwrap().active.unwrap(), manifest);
    let mut future = u["journal"].clone(); future["journal_format"] = 2.into();
    std::fs::write(&path, serde_json::to_vec(&future).unwrap()).unwrap();
    assert!(matches!(Journal::read(dir.path()), Err(JournalError::UnsupportedFormat { found: 2, .. })));
    let mut newer = u["status"].clone(); newer["future_field"] = true.into();
    newer["state"] = serde_json::json!({"kind":"future_state"});
    assert!(matches!(serde_json::from_value::<UpdateStatus>(newer).unwrap().state, UpdateState::Unknown));
}
