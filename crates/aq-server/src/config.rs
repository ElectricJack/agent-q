//! Vault `config.toml` loading and validation.

use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const SUPPORTED_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub format_version: u32,
    pub environment: Environment,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub bootstrap: BootstrapConfig,
    #[serde(default)]
    pub enrollment: EnrollmentConfig,
    #[serde(default)]
    pub workers: WorkersConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Production,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub admin_bind: SocketAddr,
    pub worker_bind: SocketAddr,
    pub public_origin: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthConfig {
    Dev {
        dev_email: String,
    },
    CloudflareAccess {
        team_domain: String,
        audience: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapConfig {
    pub owner_email: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct EnrollmentConfig {
    pub code_ttl_secs: u64,
}

impl Default for EnrollmentConfig {
    fn default() -> Self {
        Self { code_ttl_secs: 600 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WorkersConfig {
    pub heartbeat_interval_secs: u64,
    pub offline_after_secs: u64,
}

impl Default for WorkersConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 15,
            offline_after_secs: 60,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot parse {}: {message}", path.display())]
    Parse { path: PathBuf, message: String },
    #[error(
        "AQ_DEPLOYMENT=production but the vault is not a production configuration: \
         environment is {environment:?} and auth.mode is {auth_mode:?}; \
         set environment = \"production\" and auth.mode = \"cloudflare_access\""
    )]
    NotProduction {
        environment: &'static str,
        auth_mode: &'static str,
    },
    #[error("invalid config {}: {key}: {reason}", path.display())]
    Invalid {
        path: PathBuf,
        key: String,
        reason: String,
    },
}

/// Reads and validates `vault_dir/config.toml`.
pub fn load(vault_dir: &Path) -> Result<Config, ConfigError> {
    let path = vault_dir.join("config.toml");
    let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.clone(),
        source,
    })?;
    let config: Config = toml::from_str(&text).map_err(|e| ConfigError::Parse {
        path: path.clone(),
        message: e.to_string(),
    })?;
    validate(&config).map_err(|(key, reason)| ConfigError::Invalid { path, key, reason })?;
    Ok(config)
}

fn validate(c: &Config) -> Result<(), (String, String)> {
    let invalid = |key: &str, reason: String| Err((key.to_string(), reason));
    if c.format_version != SUPPORTED_FORMAT_VERSION {
        return invalid(
            "format_version",
            format!(
                "unsupported format_version {}; this server reads {SUPPORTED_FORMAT_VERSION}",
                c.format_version
            ),
        );
    }
    let production = c.environment == Environment::Production;
    if production && matches!(c.auth, AuthConfig::Dev { .. }) {
        return invalid(
            "auth.mode",
            "dev authentication cannot be enabled in production".into(),
        );
    }
    if production && !c.server.public_origin.starts_with("https://") {
        return invalid(
            "server.public_origin",
            "must start with https:// in production".into(),
        );
    }
    if let AuthConfig::CloudflareAccess { team_domain, .. } = &c.auth
        && !team_domain.starts_with("https://")
    {
        return invalid("auth.team_domain", "must start with https://".into());
    }
    for (key, v) in [
        ("enrollment.code_ttl_secs", c.enrollment.code_ttl_secs),
        (
            "workers.heartbeat_interval_secs",
            c.workers.heartbeat_interval_secs,
        ),
        ("workers.offline_after_secs", c.workers.offline_after_secs),
    ] {
        if v < 1 {
            return invalid(key, "must be at least 1".into());
        }
    }
    if c.workers.offline_after_secs <= c.workers.heartbeat_interval_secs {
        return invalid(
            "workers.offline_after_secs",
            "must be greater than workers.heartbeat_interval_secs".into(),
        );
    }
    if c.bootstrap.enabled && c.bootstrap.owner_email.is_none() {
        return invalid(
            "bootstrap.owner_email",
            "required when bootstrap.enabled is true".into(),
        );
    }
    Ok(())
}

/// Refuses a vault that is not a production configuration when the process is
/// told it runs in production (`AQ_DEPLOYMENT=production`, set by compose.yaml).
/// This stops a development vault (dev auth) from being mounted behind a tunnel.
pub fn check_deployment(config: &Config, deployment: Option<&str>) -> Result<(), ConfigError> {
    if deployment != Some("production") {
        return Ok(());
    }
    let dev_auth = matches!(config.auth, AuthConfig::Dev { .. });
    if config.environment == Environment::Production && !dev_auth {
        return Ok(());
    }
    Err(ConfigError::NotProduction {
        environment: match config.environment {
            Environment::Development => "development",
            Environment::Production => "production",
        },
        auth_mode: if dev_auth { "dev" } else { "cloudflare_access" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(env: Environment, dev: bool) -> Config {
        Config {
            format_version: 1,
            environment: env,
            server: ServerConfig {
                admin_bind: "127.0.0.1:8080".parse().unwrap(),
                worker_bind: "127.0.0.1:8081".parse().unwrap(),
                public_origin: "https://x".into(),
            },
            auth: if dev {
                AuthConfig::Dev {
                    dev_email: "a@b".into(),
                }
            } else {
                AuthConfig::CloudflareAccess {
                    team_domain: "https://t".into(),
                    audience: "a".into(),
                }
            },
            bootstrap: BootstrapConfig {
                owner_email: None,
                enabled: false,
            },
            enrollment: Default::default(),
            workers: Default::default(),
        }
    }

    #[test]
    fn production_deployment_rejects_dev_vault() {
        let e = check_deployment(&cfg(Environment::Development, true), Some("production"))
            .unwrap_err()
            .to_string();
        assert!(e.contains("development") && e.contains("dev"), "{e}");
    }

    #[test]
    fn production_deployment_rejects_dev_auth_in_production_env() {
        assert!(check_deployment(&cfg(Environment::Production, true), Some("production")).is_err());
    }

    #[test]
    fn production_deployment_rejects_development_env_with_access_auth() {
        assert!(
            check_deployment(&cfg(Environment::Development, false), Some("production")).is_err()
        );
    }

    #[test]
    fn production_deployment_accepts_production_vault() {
        check_deployment(&cfg(Environment::Production, false), Some("production")).unwrap();
    }

    #[test]
    fn other_deployments_accept_dev_vault() {
        for d in [None, Some("development"), Some("")] {
            check_deployment(&cfg(Environment::Development, true), d).unwrap();
        }
    }
}
