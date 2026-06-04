//! Command-line argument types, shared between the `sol-doctor` binary and the
//! library so an embedding frontend can construct the same inputs.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Top-level CLI: a subcommand plus the global `--color` and `--verbose` flags.
#[derive(Debug, Parser)]
#[command(
    name = "sol-doctor",
    version,
    about = "A Rust CLI for Solana RPC production-readiness diagnostics, comparison, and WebSocket checks."
)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Commands,

    /// When to colorize human output (JSON output is never colored).
    #[arg(long, global = true, value_enum, default_value_t = crate::color::ColorChoice::Auto)]
    pub color: crate::color::ColorChoice,

    /// Show full per-check details in human output (full redacted URLs, hashes,
    /// per-method latencies, and notes). Affects human output only; `--json`
    /// output is unchanged and takes precedence when both are set.
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check whether a Solana RPC endpoint is usable.
    Check(CheckArgs),

    /// Compare multiple Solana RPC endpoints for a workload profile.
    Compare(CompareArgs),

    /// Diagnose Solana WebSocket readiness for realtime workloads.
    Ws(WsArgs),
}

/// Arguments for the `ws` subcommand.
#[derive(Debug, Args, Clone)]
pub struct WsArgs {
    /// Solana RPC HTTP URL used to derive the WebSocket endpoint.
    #[arg(long)]
    pub rpc: String,

    /// Explicit WebSocket URL override (ws:// or wss://).
    #[arg(long)]
    pub ws: Option<String>,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Connection and first-notification timeout in milliseconds.
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,
}

/// Arguments for the `check` subcommand.
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

/// Arguments for the `compare` subcommand.
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

/// The workload profile that drives `compare` scoring and recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompareProfile {
    General,
    Wallet,
    Bot,
    Indexer,
    Ci,
}

impl CompareProfile {
    /// The lowercase profile name (`general`, `wallet`, `bot`, `indexer`, `ci`).
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
