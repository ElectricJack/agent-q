use aq_server::config::{Environment, load};
use std::path::Path;

fn vault_with(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), toml).unwrap();
    dir
}

const VALID: &str = r#"
format_version = 1
environment = "development"

[server]
admin_bind = "0.0.0.0:8080"
worker_bind = "0.0.0.0:8081"
public_origin = "http://127.0.0.1:8080"

[auth]
mode = "dev"
dev_email = "owner@localhost"

[bootstrap]
enabled = false
"#;

#[test]
fn loads_example_vault() {
    let cfg = load(Path::new("../../vault.example")).unwrap();
    assert_eq!(cfg.environment, Environment::Development);
    assert_eq!(cfg.enrollment.code_ttl_secs, 600);
    assert_eq!(cfg.workers.heartbeat_interval_secs, 15);
    assert_eq!(cfg.workers.offline_after_secs, 60);
}

#[test]
fn loads_minimal_valid_config_with_defaults() {
    let dir = vault_with(VALID);
    let cfg = load(dir.path()).unwrap();
    assert_eq!(cfg.enrollment.code_ttl_secs, 600);
}

#[test]
fn rejects_dev_auth_in_production() {
    let dir = vault_with(
        &VALID
            .replace("development", "production")
            .replace("http://", "https://"),
    );
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("auth.mode"), "{err}");
    assert!(err.contains("cannot be enabled in production"), "{err}");
}

#[test]
fn rejects_non_https_origin_in_production() {
    let toml = r#"
format_version = 1
environment = "production"
[server]
admin_bind = "0.0.0.0:8080"
worker_bind = "0.0.0.0:8081"
public_origin = "http://aq.example.com"
[auth]
mode = "cloudflare_access"
team_domain = "https://team.cloudflareaccess.com"
audience = "abc"
[bootstrap]
enabled = false
"#;
    let dir = vault_with(toml);
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("server.public_origin"), "{err}");
}

#[test]
fn rejects_non_https_team_domain() {
    let toml = r#"
format_version = 1
environment = "development"
[server]
admin_bind = "0.0.0.0:8080"
worker_bind = "0.0.0.0:8081"
public_origin = "http://127.0.0.1:8080"
[auth]
mode = "cloudflare_access"
team_domain = "team.cloudflareaccess.com"
audience = "abc"
[bootstrap]
enabled = false
"#;
    let dir = vault_with(toml);
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("auth.team_domain"), "{err}");
}

#[test]
fn rejects_unknown_key() {
    let dir = vault_with(&VALID.replace("[auth]", "admni_bind = \"0.0.0.0:1\"\n\n[auth]"));
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("admni_bind"), "{err}");
    assert!(
        err.contains(&dir.path().join("config.toml").display().to_string()),
        "{err}"
    );
}

#[test]
fn rejects_missing_required_key() {
    let dir = vault_with(&VALID.replace("public_origin = \"http://127.0.0.1:8080\"\n", ""));
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("public_origin"), "{err}");
}

#[test]
fn rejects_future_format_version() {
    let dir = vault_with(&VALID.replace("format_version = 1", "format_version = 2"));
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("unsupported format_version 2"), "{err}");
    assert!(err.contains("format_version"), "{err}");
}

#[test]
fn rejects_offline_not_greater_than_heartbeat() {
    let dir = vault_with(&format!(
        "{VALID}\n[workers]\nheartbeat_interval_secs = 30\noffline_after_secs = 30\n"
    ));
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("workers.offline_after_secs"), "{err}");
}

#[test]
fn missing_file_names_path() {
    let dir = tempfile::tempdir().unwrap();
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(
        err.contains(&dir.path().join("config.toml").display().to_string()),
        "{err}"
    );
}

#[test]
fn bootstrap_enabled_requires_email() {
    let dir = vault_with(&VALID.replace("enabled = false", "enabled = true"));
    let err = load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("bootstrap.owner_email"), "{err}");
}

#[test]
fn rejects_zero_durations() {
    for (section, key) in [
        ("enrollment", "code_ttl_secs"),
        ("workers", "heartbeat_interval_secs"),
        ("workers", "offline_after_secs"),
    ] {
        let dir = vault_with(&format!("{VALID}\n[{section}]\n{key} = 0\n"));
        let err = load(dir.path()).unwrap_err().to_string();
        assert!(err.contains(key) && err.contains("at least 1"), "{err}");
    }
}
