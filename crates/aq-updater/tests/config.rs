use aq_updater::config::UpdaterConfig;

const BASE: &str = r#"
state_dir = "/var/lib/agent-q/updater"
backup_dir = "/var/lib/agent-q/backups"
[compose]
project_dir = "/opt/agent-q"
files = ["compose.yaml"]
project_name = "agent-q"
[verifier]
kind = "cosign_keyless"
cosign_path = "/usr/local/bin/cosign"
identity_regexp = "^https://github\\.com/x$"
oidc_issuer = "https://token.actions.githubusercontent.com"
"#;

fn with(extra: &str) -> String {
    format!("{extra}\n{BASE}")
}

#[test]
fn defaults_apply() {
    let c = UpdaterConfig::parse(BASE).unwrap();
    assert_eq!(c.release_repo, "ghcr.io/electricjack/agent-q-release");
    assert_eq!(c.channel, "stable");
    assert_eq!(c.readiness_deadline_secs, 120);
    assert_eq!(c.poll_interval_secs, 3600);
    assert_eq!(c.min_free_bytes, 2_147_483_648);
    assert_eq!(c.backups_keep, 5);
}

#[test]
fn config_rejects_unknown_key() {
    let e = UpdaterConfig::parse(&with("bogus = 1")).unwrap_err();
    assert!(e.to_string().contains("bogus"), "{e}");
}

#[test]
fn rejects_relative_empty_and_bad_regexp() {
    let rel = BASE.replace("/var/lib/agent-q/updater", "rel/dir");
    assert!(UpdaterConfig::parse(&rel).is_err());
    let empty = BASE.replace(r#"files = ["compose.yaml"]"#, "files = []");
    assert!(UpdaterConfig::parse(&empty).is_err());
    let re = BASE.replace(r#""^https://github\\.com/x$""#, r#""(""#);
    let e = UpdaterConfig::parse(&re).unwrap_err();
    assert!(e.to_string().contains("identity_regexp"), "{e}");
}

#[test]
fn rejects_out_of_range_numbers_and_names() {
    for bad in [
        "poll_interval_secs = 0",
        "readiness_deadline_secs = 1",
        "backups_keep = 0",
        r#"channel = """#,
        r#"release_repo = """#,
        r#"release_repo = "ghcr.io/x/y:latest""#,
        r#"release_repo = "ghcr.io/x/y@sha256:abc""#,
    ] {
        assert!(UpdaterConfig::parse(&with(bad)).is_err(), "{bad} accepted");
    }
    assert!(UpdaterConfig::parse(&with("readiness_deadline_secs = 2")).is_ok());
}

#[cfg(not(feature = "test-verifier"))]
#[test]
fn test_key_verifier_kind_rejected_without_feature() {
    let cfg = BASE
        .replace("cosign_keyless", "test_key")
        .replace("cosign_path", "public_key_hex");
    let e = UpdaterConfig::parse(&cfg).unwrap_err();
    assert!(e.to_string().contains("verifier.kind"), "{e}");
}

#[cfg(feature = "test-verifier")]
#[test]
fn test_key_verifier_kind_accepted_with_feature() {
    use aq_updater::config::VerifierConfig;
    let cfg = r#"
state_dir = "/s"
backup_dir = "/b"
[compose]
project_dir = "/p"
files = ["c.yaml"]
project_name = "x"
[verifier]
kind = "test_key"
public_key_hex = "00"
"#;
    assert!(matches!(
        UpdaterConfig::parse(cfg).unwrap().verifier,
        VerifierConfig::TestKey { .. }
    ));
}
