//! `/etc/agent-q/updater.toml`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid config: {0}")]
    Parse(String),
    #[error("invalid config: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComposeConfig {
    pub project_dir: PathBuf,
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub env_files: Vec<PathBuf>,
    pub project_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VerifierConfig {
    CosignKeyless {
        cosign_path: PathBuf,
        identity_regexp: String,
        oidc_issuer: String,
    },
    #[cfg(feature = "test-verifier")]
    TestKey { public_key_hex: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdaterConfig {
    pub state_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub compose: ComposeConfig,
    #[serde(default = "d_repo")]
    pub release_repo: String,
    #[serde(default = "d_channel")]
    pub channel: String,
    pub verifier: VerifierConfig,
    #[serde(default = "d_ready")]
    pub readiness_deadline_secs: u64,
    #[serde(default = "d_poll")]
    pub poll_interval_secs: u64,
    #[serde(default = "d_free")]
    pub min_free_bytes: u64,
    #[serde(default = "d_keep")]
    pub backups_keep: usize,
}

fn d_repo() -> String {
    "ghcr.io/electricjack/agent-q-release".into()
}
fn d_channel() -> String {
    "stable".into()
}
fn d_ready() -> u64 {
    120
}
fn d_poll() -> u64 {
    3600
}
fn d_free() -> u64 {
    2_147_483_648
}
fn d_keep() -> usize {
    5
}

impl UpdaterConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: UpdaterConfig = toml::from_str(text).map_err(|e| {
            let msg = e.to_string();
            // Without the test-verifier feature the `test_key` variant does
            // not exist; serde's "unknown variant" message is not obvious.
            if msg.contains("unknown variant") && msg.contains("test_key") {
                ConfigError::Parse(
                    "verifier.kind \"test_key\" requires the test-verifier build feature".into(),
                )
            } else if msg.contains("unknown variant") {
                ConfigError::Parse(format!("verifier.kind: {msg}"))
            } else {
                ConfigError::Parse(msg)
            }
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let bad = |m: String| Err(ConfigError::Invalid(m));
        for (name, p) in [
            ("state_dir", &self.state_dir),
            ("backup_dir", &self.backup_dir),
            ("compose.project_dir", &self.compose.project_dir),
        ] {
            if !p.is_absolute() {
                return bad(format!("{name} must be an absolute path"));
            }
        }
        if self.compose.files.is_empty() {
            return bad("compose.files must not be empty".into());
        }
        if self.readiness_deadline_secs < 1 {
            return bad("readiness_deadline_secs must be >= 1".into());
        }
        if self.poll_interval_secs < 1 {
            return bad("poll_interval_secs must be >= 1".into());
        }
        #[allow(irrefutable_let_patterns)]
        if let VerifierConfig::CosignKeyless {
            identity_regexp, ..
        } = &self.verifier
            && let Err(e) = regex::Regex::new(identity_regexp)
        {
            return bad(format!("verifier.identity_regexp does not compile: {e}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let e = UpdaterConfig::parse(&format!("bogus = 1\n{BASE}")).unwrap_err();
        assert!(e.to_string().contains("bogus"), "{e}");
    }

    #[test]
    fn rejects_relative_empty_and_bad_values() {
        let rel = BASE.replace("/var/lib/agent-q/updater", "rel/dir");
        assert!(UpdaterConfig::parse(&rel).is_err());
        let empty = BASE.replace(r#"files = ["compose.yaml"]"#, "files = []");
        assert!(UpdaterConfig::parse(&empty).is_err());
        let re = BASE.replace(r#""^https://github\\.com/x$""#, r#""(""#);
        let e = UpdaterConfig::parse(&re).unwrap_err();
        assert!(e.to_string().contains("identity_regexp"), "{e}");
        let zero = format!("poll_interval_secs = 0\n{BASE}");
        assert!(UpdaterConfig::parse(&zero).is_err());
        let zero = format!("readiness_deadline_secs = 0\n{BASE}");
        assert!(UpdaterConfig::parse(&zero).is_err());
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
}
