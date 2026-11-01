//! Actual local physical lifecycle after actual remote phase issuance. No remote
//! Started/Stopped acknowledgment or task settlement is claimed by this probe.
use super::*;
fn pin(binary: &str, hash: &str, id: &str) -> Value {
    let path =
        std::fs::canonicalize(std::env::var_os(binary).expect("explicit host binary")).unwrap();
    let expected: Digest =
        serde_json::from_value(json!(std::env::var(hash).expect("explicit host digest"))).unwrap();
    assert_eq!(Digest::of(&std::fs::read(&path).unwrap()), expected);
    json!({"path":path,"artifact":{"id":id,"version":"1","digest":expected}})
}
pub(super) fn configuration() -> Value {
    json!({
        "helper":pin("AQ_NATIVE_HOST_BINARY","AQ_NATIVE_HOST_SHA256","aq.synthetic-host-helper"),
        "tmux":pin("AQ_NATIVE_TMUX_BINARY","AQ_NATIVE_TMUX_SHA256","aq.synthetic-system-tmux34"),
        "program":pin("AQ_NATIVE_SYNTHETIC_ELF","AQ_NATIVE_SYNTHETIC_SHA256","aq.synthetic-native-harness"),
        "renderer":{"id":"aq.synthetic-baseline-ascii","version":"1",
            "digest":Digest::of(b"synthetic baseline only; no Snapshot qualification")}
    })
}
pub(super) fn verify(root: &std::path::Path, result: &Value) {
    let started: aq_admission::started::Request =
        serde_json::from_value(result["host_started"].clone()).unwrap();
    started.validate().unwrap();
    let delivery: aq_admission::preparation_v2::ExecutionDelivery =
        serde_json::from_value(result["execution_delivery"].clone()).unwrap();
    assert_eq!(started.binding, delivery.receipt.grant.binding);
    assert_eq!(started.grant_id, delivery.receipt.grant.grant_id);
    assert_ne!(started.guardian_id, started.observation.guardian.nonce);
    let snapshot: Value =
        serde_json::from_slice(&std::fs::read(root.join("journal/journal.json")).unwrap()).unwrap();
    let history =
        &snapshot["state"]["attempts"][started.binding.attempt_id.to_string()]["native_host"];
    assert_eq!(history["running"]["request"], result["host_started"]);
    assert_eq!(history["stopped"], result["host_stopped"]);
    assert_eq!(
        history["stopped"]["recorded_revision"],
        result["host_revision"]
    );
    assert!(history["baseline"].is_object());
    assert!(history["started_receipt"].is_null());
    assert!(history["uncertain"].is_null());
    assert_eq!(
        history["stopped"]["observation"]["kernel_wait_echild"],
        true
    );
    assert!(history["stopped"]["observation"]["root_status"].is_i64());
    assert_eq!(
        history["stopped"]["observation"]["guardian"],
        history["running"]["observation"]["guardian"]
    );
    assert!(
        history["stopped"]["recorded_revision"].as_u64().unwrap()
            > history["running"]["recorded_revision"].as_u64().unwrap()
    );
    let directory = root.join("host").join(started.launch_id.to_string());
    assert_eq!(
        std::fs::read(directory.join("fixture-marker")).unwrap(),
        std::fs::read(directory.join("context-0.bin")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("fixture-marker.cwd")).unwrap(),
        result["workspace_path"].as_str().unwrap()
    );
    eprintln!(
        "actual host: Running then durable ECHILD; execution_revision={}, stopped_revision={}",
        result["execution_revision"], result["host_revision"]
    );
}
