use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "sol-doctor",
    version,
    about = "A lightweight Rust CLI for diagnosing Solana RPC and infrastructure health."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check whether a Solana RPC endpoint is usable.
    Check(CheckArgs),
}

#[derive(Debug, Args, Clone)]
pub struct CheckArgs {
    /// Solana RPC HTTP URL to diagnose.
    #[arg(long)]
    pub rpc: String,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Per-request timeout in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    pub timeout_ms: u64,
}
