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
    assert_eq!(c.expected_auth_mode, "cloudflare_access");
    assert_eq!(c.maintenance_drain_secs, 5);
}

#[test]
fn maintenance_drain_may_be_zero() {
    let c = UpdaterConfig::parse(&with("maintenance_drain_secs = 0")).unwrap();
    assert_eq!(c.maintenance_drain_secs, 0);
    let c = UpdaterConfig::parse(&with("maintenance_drain_secs = 30")).unwrap();
    assert_eq!(c.maintenance_drain_secs, 30);
    assert!(UpdaterConfig::parse(&with("maintenance_drain_secs = -1")).is_err());
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
        r#"expected_auth_mode = """#,
        r#"release_repo = """#,
        r#"release_repo = "ghcr.io/x/y:stable""#,
        r#"release_repo = "ghcr.io/x/y@sha256:abc""#,
        r#"release_repo = "agent-q-release""#,
        r#"release_repo = "ghcr.io/""#,
        r#"release_repo = "host:5000/""#,
    ] {
        assert!(UpdaterConfig::parse(&with(bad)).is_err(), "{bad} accepted");
    }
    assert!(UpdaterConfig::parse(&with("readiness_deadline_secs = 2")).is_ok());
    let dev = UpdaterConfig::parse(&with(r#"expected_auth_mode = "dev""#)).unwrap();
    assert_eq!(dev.expected_auth_mode, "dev");
    for good in [
        "127.0.0.1:5000/agent-q-release",
        "ghcr.io/electricjack/agent-q-release",
    ] {
        let c = with(&format!("release_repo = \"{good}\""));
        assert!(UpdaterConfig::parse(&c).is_ok(), "{good} refused");
    }
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

#[test]
fn rejects_a_state_dir_that_cannot_go_into_release_env() {
    // The spool path is written unquoted into release.env.
    for bad in [
        "/var/lib/agent q",
        "/var/lib/$HOME",
        "/var/lib/a#b",
        "/var/lib/a\\nB=1",
    ] {
        let cfg = BASE.replace("/var/lib/agent-q/updater", bad);
        let e = UpdaterConfig::parse(&cfg).unwrap_err();
        assert!(e.to_string().contains("state_dir"), "{bad:?}: {e}");
    }
    let ok = BASE.replace("/var/lib/agent-q/updater", "/tmp/aq-suite.Ab3_x/state");
    UpdaterConfig::parse(&ok).unwrap();
}
