use clap::Parser;
use std::path::PathBuf;

/// Agent Q host updater. Subcommands are added in a later task.
#[derive(Parser)]
#[command(name = "aq-updater", version)]
struct Cli {
    /// Path to the updater config.
    #[arg(long, default_value = "/etc/agent-q/updater.toml")]
    config: PathBuf,
}

fn main() {
    let _ = Cli::parse();
    println!("aq-updater: no subcommands implemented yet (see --help)");
}
