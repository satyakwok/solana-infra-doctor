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

    /// Yellowstone gRPC diagnostics (use `grpc check`).
    Grpc(GrpcArgs),
}

/// Arguments for the `grpc` command group.
#[derive(Debug, Args)]
pub struct GrpcArgs {
    /// The gRPC subcommand to run.
    #[command(subcommand)]
    pub command: GrpcCommand,
}

/// The `grpc` subcommands.
#[derive(Debug, Subcommand)]
pub enum GrpcCommand {
    /// Check whether a Yellowstone gRPC endpoint is ready for a backend workload.
    Check(GrpcCheckArgs),

    /// Compare multiple Yellowstone gRPC endpoints for a workload profile.
    Compare(GrpcCompareArgs),
}

/// Arguments for `grpc compare`.
#[derive(Debug, Args, Clone)]
pub struct GrpcCompareArgs {
    /// Yellowstone gRPC endpoint URLs to compare. Provide at least two.
    #[arg(long, required = true)]
    pub grpc: Vec<String>,

    /// Environment variable names holding each endpoint's `x-token`, paired by
    /// position with `--grpc`. Provide none (all anonymous), one (shared by every
    /// endpoint), or exactly one per `--grpc`. The token is never accepted
    /// directly on the command line and is never printed.
    #[arg(long = "x-token-env")]
    pub x_token_env: Vec<String>,

    /// Workload profile used for scoring and recommendations.
    #[arg(long, default_value_t = GrpcCompareProfile::General)]
    pub profile: GrpcCompareProfile,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Write a Markdown report to this path.
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Connection and per-request timeout in milliseconds.
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,

    /// Bounded slot-stream observation window, in milliseconds.
    #[arg(long = "duration", default_value_t = 5_000)]
    pub duration_ms: u64,
}

/// The workload profile that drives `grpc compare` scoring and recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GrpcCompareProfile {
    /// Balanced scoring across connect, first-event, and slot freshness.
    General,
    /// Latency-sensitive (bots/MEV): connect and time-to-first-event weigh most.
    Latency,
    /// Freshness-sensitive (indexers): slot freshness and stream stability weigh most.
    Indexer,
}

impl GrpcCompareProfile {
    /// The lowercase profile name (`general`, `latency`, `indexer`).
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Latency => "latency",
            Self::Indexer => "indexer",
        }
    }
}

impl std::fmt::Display for GrpcCompareProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Arguments for `grpc check`.
#[derive(Debug, Args, Clone)]
pub struct GrpcCheckArgs {
    /// Yellowstone gRPC endpoint URL (http or https).
    #[arg(long)]
    pub grpc: String,

    /// Read the `x-token` from this environment variable. The token is never
    /// accepted directly on the command line and is never printed.
    #[arg(long)]
    pub x_token_env: Option<String>,

    /// Optional HTTP RPC endpoint for a slot-freshness cross-check.
    #[arg(long)]
    pub rpc: Option<String>,

    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Write a Markdown report to this path.
    #[arg(long)]
    pub report: Option<PathBuf>,

    /// Connection and per-request timeout in milliseconds.
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,

    /// Bounded slot-stream observation window, in milliseconds.
    #[arg(long = "duration", default_value_t = 5_000)]
    pub duration_ms: u64,
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

    /// Which PubSub subscription to test.
    #[arg(long, value_enum, default_value_t = crate::ws::subscription::Subscription::Slot)]
    pub subscription: crate::ws::subscription::Subscription,

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

    /// Probe latency this many times and report p50/p95 percentiles. The default
    /// of 1 is a single sample; higher values reveal tail latency.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub samples: u32,

    /// Run data-readiness checks: `getProgramAccounts` enablement and archival
    /// history depth. Off by default; these add a couple of extra requests.
    #[arg(long)]
    pub data: bool,

    /// Program to probe for `getProgramAccounts` readiness (default: a small,
    /// non-excluded program). Pass your own program to test its gPA availability;
    /// only used with `--data`.
    #[arg(long)]
    pub data_program: Option<String>,

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

    /// Run data-readiness checks (`getProgramAccounts` enablement + archival depth)
    /// for every endpoint. Off by default; the `indexer` profile scores them when
    /// enabled. Adds two requests per endpoint.
    #[arg(long)]
    pub data: bool,

    /// Program to probe for `getProgramAccounts` readiness when `--data` is set
    /// (default: a small, non-excluded program). Applies to every endpoint.
    #[arg(long)]
    pub data_program: Option<String>,

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
