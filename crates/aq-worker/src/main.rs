use clap::Parser;

/// Agent Q worker connector.
#[derive(Parser)]
#[command(version)]
struct Cli {}

fn main() {
    Cli::parse();
    println!("aq-worker {}", env!("CARGO_PKG_VERSION"));
}
