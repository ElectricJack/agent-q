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
    Healthcheck,
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
        Command::Healthcheck => std::process::exit(healthcheck().await),
    };
    if let Err(e) = result {
        eprintln!("aq-server: {e:#}");
        std::process::exit(2);
    }
}

async fn serve(vault: PathBuf) -> anyhow::Result<()> {
    let config = aq_server::config::load(&vault)?;
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
    let store = Store::connect(&url)
        .await
        .context("cannot connect to the database")?;
    aq_server::run(config, store).await
}

/// Container health probe. The image has no curl, so the binary checks itself.
async fn healthcheck() -> i32 {
    let probe = async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()?;
        client
            .get("http://127.0.0.1:8080/healthz")
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
