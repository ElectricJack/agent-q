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
