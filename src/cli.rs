use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "sol-doctor",
    version,
    about = "A Rust CLI for Solana RPC production-readiness diagnostics and comparison."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check whether a Solana RPC endpoint is usable.
    Check(CheckArgs),

    /// Compare multiple Solana RPC endpoints for a workload profile.
    Compare(CompareArgs),
}

#[derive(Debug, Args, Clone)]
pub struct CheckArgs {
    /// Solana RPC HTTP URL to diagnose.
    #[arg(long)]
    pub rpc: String,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Keep WARNING exit code 1 and make CI warning behavior explicit in output.
    #[arg(long)]
    pub fail_on_warning: bool,

    /// Per-request timeout in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    pub timeout_ms: u64,
}

#[derive(Debug, Args, Clone)]
pub struct CompareArgs {
    /// Solana RPC HTTP URLs to compare. Provide at least two.
    #[arg(long, required = true)]
    pub rpc: Vec<String>,

    /// Workload profile used for scoring and recommendations.
    #[arg(long, default_value_t = CompareProfile::General)]
    pub profile: CompareProfile,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Write a Markdown report to this path.
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Per-request timeout in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompareProfile {
    General,
    Wallet,
    Bot,
    Indexer,
    Ci,
}

impl CompareProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Wallet => "wallet",
            Self::Bot => "bot",
            Self::Indexer => "indexer",
            Self::Ci => "ci",
        }
    }
}

impl std::fmt::Display for CompareProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}
