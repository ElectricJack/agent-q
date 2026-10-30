use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, bail};
use aq_server::store::Store;
use clap::{Parser, Subcommand};
use sqlx::postgres::PgConnectOptions;

#[derive(Parser)]
#[command(name = "aq-server", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the admin and worker listeners.
    Serve {
        /// Vault directory containing config.toml.
        #[arg(long, env = "AQ_VAULT_DIR")]
        vault: PathBuf,
    },
    /// Probe the local admin listener's /healthz; exit 0 if healthy, 1 if not.
    Healthcheck {
        /// Vault directory; the probe targets its admin_bind port (default 8080).
        #[arg(long, env = "AQ_VAULT_DIR")]
        vault: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let result = match cli.command {
        Command::Serve { vault } => serve(vault).await,
        Command::Healthcheck { vault } => std::process::exit(healthcheck(vault).await),
    };
    if let Err(e) = result {
        eprintln!("aq-server: {e:#}");
        std::process::exit(2);
    }
}

async fn serve(vault: PathBuf) -> anyhow::Result<()> {
    let config = aq_server::config::load(&vault)?;
    // Checked before touching the database so a misconfigured production
    // deployment fails fast.
    aq_server::config::check_deployment(&config, std::env::var("AQ_DEPLOYMENT").ok().as_deref())?;
    let Some(url) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        bail!("DATABASE_URL is not set");
    };
    // Parse errors can echo the URL, so they are reported without detail.
    let opts = PgConnectOptions::from_str(&url)
        .map_err(|_| anyhow::anyhow!("DATABASE_URL is not a valid PostgreSQL URL"))?;
    tracing::info!(
        host = opts.get_host(),
        database = opts.get_database().unwrap_or(""),
        "connecting to database"
    );
    // sqlx connection errors do not include the password.
    let store = Store::connect_with(opts)
        .await
        .context("cannot connect to the database")?;
    aq_server::run(config, store).await
}

/// Container health probe. The image has no curl, so the binary checks itself.
async fn healthcheck(vault: Option<PathBuf>) -> i32 {
    let port = vault
        .and_then(|v| aq_server::config::load(&v).ok())
        .map(|c| c.server.admin_bind.port())
        .unwrap_or(8080);
    let probe = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()?;
        client
            .get(format!("http://127.0.0.1:{port}/healthz"))
            .send()
            .await?
            .error_for_status()
    };
    match probe.await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("aq-server: unhealthy: {e}");
            1
        }
    }
}
