use aq_release::*;
use semver::{Version, VersionReq};

const DIGEST: &str = "ghcr.io/electricjack/agent-q-server@sha256:0000000000000000000000000000000000000000000000000000000000000000";
const DIGEST_W: &str = "ghcr.io/electricjack/agent-q-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn json(version: &str, upgrade_from: &str, state: u32, readable: &[u32]) -> String {
    format!(
        r#"{{"manifest_version":1,"version":"{version}","source_revision":"abc123",
"images":{{"server":"{DIGEST}","worker":"{DIGEST_W}"}},
"protocol":{{"min":1,"max":1}},"state_format":{state},"readable_state_formats":{readable:?},
"upgrade_from":"{upgrade_from}","created_at":"2026-10-30T12:00:00Z"}}"#
    )
}

fn m(version: &str, upgrade_from: &str, state: u32, readable: &[u32]) -> ReleaseManifest {
    ReleaseManifest::from_json(json(version, upgrade_from, state, readable).as_bytes()).unwrap()
}

#[test]
fn parses_valid_manifest() {
    let r = m("0.2.0", ">=0.1.0", 1, &[1]);
    assert_eq!(r.version, Version::new(0, 2, 0));
    assert_eq!(r.images.server, DIGEST);
    assert_eq!(r.upgrade_from, VersionReq::parse(">=0.1.0").unwrap());
}

#[test]
fn rejects_tag_reference_without_digest() {
    let j = json("0.2.0", ">=0.1.0", 1, &[1]).replace(DIGEST, "ghcr.io/x/y:latest");
    assert!(matches!(
        ReleaseManifest::from_json(j.as_bytes()),
        Err(ManifestError::InvalidImage { .. })
    ));
}

#[test]
fn rejects_unknown_field() {
    let j = json("0.2.0", ">=0.1.0", 1, &[1])
        .replace("\"state_format\"", "\"extra\":1,\"state_format\"");
    assert!(matches!(
        ReleaseManifest::from_json(j.as_bytes()),
        Err(ManifestError::Parse(_))
    ));
}

#[test]
fn rejects_state_format_not_readable() {
    let j = json("0.2.0", ">=0.1.0", 2, &[1]);
    assert!(matches!(
        ReleaseManifest::from_json(j.as_bytes()),
        Err(ManifestError::StateFormatNotReadable { .. })
    ));
}

#[test]
fn rejects_inverted_protocol_range_and_bad_manifest_version() {
    let j = json("0.2.0", ">=0.1.0", 1, &[1]).replace("\"min\":1,\"max\":1", "\"min\":3,\"max\":2");
    assert!(matches!(
        ReleaseManifest::from_json(j.as_bytes()),
        Err(ManifestError::ProtocolRange { .. })
    ));
    let j = json("0.2.0", ">=0.1.0", 1, &[1])
        .replace("\"manifest_version\":1", "\"manifest_version\":2");
    assert!(matches!(
        ReleaseManifest::from_json(j.as_bytes()),
        Err(ManifestError::UnsupportedManifestVersion(2))
    ));
}

#[test]
fn first_install_allowed() {
    let t = m("0.2.0", ">=0.1.0", 1, &[1]);
    let plan = check_upgrade(None, None, &t).unwrap();
    assert!(!plan.needs_restore_on_rollback);
    assert!(check_upgrade(None, Some(1), &t).is_ok());
    assert!(matches!(
        check_upgrade(None, Some(5), &t),
        Err(Refusal::UnreadableState { found: 5, .. })
    ));
}

#[test]
fn same_version_refused() {
    let c = m("0.2.0", ">=0.1.0", 1, &[1]);
    assert_eq!(
        check_upgrade(Some(&c), Some(1), &c).unwrap_err(),
        Refusal::SameVersion
    );
}

#[test]
fn outside_upgrade_from_refused() {
    let c = m("0.1.0", ">=0.1.0", 1, &[1]);
    let t = m("0.3.0", ">=0.2.0", 1, &[1]);
    let e = check_upgrade(Some(&c), Some(1), &t).unwrap_err();
    assert!(matches!(e, Refusal::NotInUpgradePath { .. }));
    assert!(e.to_string().contains("install 0.2.x first") || e.to_string().contains(">=0.2.0"));
}

#[test]
fn unreadable_state_refused() {
    let c = m("0.1.0", ">=0.1.0", 2, &[1, 2]);
    let t = m("0.2.0", ">=0.1.0", 1, &[1]);
    assert_eq!(
        check_upgrade(Some(&c), Some(2), &t).unwrap_err(),
        Refusal::UnreadableState {
            found: 2,
            readable: vec![1]
        }
    );
}

#[test]
fn downgrade_refused() {
    let c = m("0.3.0", ">=0.1.0", 1, &[1]);
    let t = m("0.2.0", ">=0.1.0", 1, &[1]);
    assert!(matches!(
        check_upgrade(Some(&c), Some(1), &t).unwrap_err(),
        Refusal::Downgrade { .. }
    ));
}

#[test]
fn restore_needed_when_state_format_changes() {
    let c = m("0.1.0", ">=0.1.0", 1, &[1]);
    let t = m("0.2.0", ">=0.1.0", 2, &[1, 2]);
    assert!(
        check_upgrade(Some(&c), Some(1), &t)
            .unwrap()
            .needs_restore_on_rollback
    );
    let t2 = m("0.2.0", ">=0.1.0", 1, &[1]);
    assert!(
        !check_upgrade(Some(&c), Some(1), &t2)
            .unwrap()
            .needs_restore_on_rollback
    );
}

#[test]
fn canonical_json_roundtrips_byte_identical() {
    let r = m("0.2.0", ">=0.1.0", 1, &[1]);
    let bytes = r.to_canonical_json();
    let again = ReleaseManifest::from_json(&bytes).unwrap();
    assert_eq!(again, r);
    assert_eq!(again.to_canonical_json(), bytes);
    // sorted keys, no whitespace
    let s = String::from_utf8(bytes).unwrap();
    assert!(!s.contains(' ') && !s.contains('\n'));
    assert!(s.starts_with(r#"{"created_at":"#));
    assert!(s.find("\"images\"").unwrap() < s.find("\"version\"").unwrap());
}
