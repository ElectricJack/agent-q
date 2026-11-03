use aq_protocol::SUPPORTED_WORKER_PROTOCOLS;
use aq_server::STATE_FORMAT_VERSION;

fn dockerfile_arg(name: &str) -> u32 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Dockerfile");
    let dockerfile = std::fs::read_to_string(path).expect("read server Dockerfile");
    let prefix = format!("ARG {name}=");
    let defaults: Vec<_> = dockerfile
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix))
        .collect();
    assert_eq!(defaults.len(), 1, "expected one default for {name}");
    defaults[0].parse().expect("numeric Dockerfile default")
}

#[test]
fn dockerfile_state_format_matches_source() {
    assert_eq!(
        dockerfile_arg("AQ_STATE_FORMAT"),
        STATE_FORMAT_VERSION,
        "bump Dockerfile's AQ_STATE_FORMAT default with STATE_FORMAT_VERSION"
    );
}

#[test]
fn dockerfile_protocol_max_matches_source() {
    assert_eq!(
        dockerfile_arg("AQ_PROTOCOL_MAX"),
        SUPPORTED_WORKER_PROTOCOLS.1,
        "bump Dockerfile's AQ_PROTOCOL_MAX default with SUPPORTED_WORKER_PROTOCOLS"
    );
}
