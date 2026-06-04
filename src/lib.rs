//! Solana Infra Doctor — the reusable diagnostic engine.
//!
//! This crate provides the Solana RPC production-readiness diagnostics that the
//! `sol-doctor` binary exposes on the command line. The binary is a thin
//! frontend; all diagnostic logic lives here so it can be embedded in other
//! interfaces (for example a CI wrapper) without the CLI.
//!
//! The engine is intentionally lightweight: raw HTTP JSON-RPC via `reqwest` and
//! WebSocket diagnostics via `tokio-tungstenite`, with no Solana SDK dependency.
//!
//! ## Layout
//!
//! - [`checks`] — single-endpoint HTTP JSON-RPC diagnostics and verdicts.
//! - [`compare`] — multi-endpoint comparison, scoring, and report rendering.
//! - [`ws`] — WebSocket (`slotSubscribe`) readiness diagnostics.
//! - [`rpc`] — the JSON-RPC client, endpoint parsing, and wire models.
//! - [`redact`] — credential/API-key redaction for safe output.
//! - [`report`], [`latency`], [`verdict`], [`error`] — output and shared models.
//! - [`output`] — shared human-terminal presentation helpers (status vocabulary,
//!   unit formatting, tables); not used by JSON or Markdown output.
//! - [`color`] — TTY-aware ANSI styling for human output.
//! - [`cli`] — argument types shared with the binary frontend.
//!
//! ## Primary entrypoints
//!
//! ```no_run
//! # async fn demo() -> Result<(), solana_infra_doctor::error::AppError> {
//! use solana_infra_doctor::{cli::CheckArgs, run_check};
//!
//! let report = run_check(CheckArgs {
//!     rpc: "https://api.mainnet-beta.solana.com".to_string(),
//!     json: false,
//!     fail_on_warning: false,
//!     samples: 1,
//!     timeout_ms: 5_000,
//! })
//! .await?;
//! println!("{}", report.verdict);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

pub mod checks;
pub mod cli;
pub mod color;
pub mod compare;
pub mod error;
pub mod latency;
pub mod output;
pub mod redact;
pub mod report;
pub mod rpc;
pub mod verdict;
pub mod ws;

// Curated top-level entrypoints for embedding the engine in another frontend.
pub use checks::run_check;
pub use compare::run_compare;
pub use redact::{redact_text, redact_url};
pub use ws::run_ws;
