//! The crate's error type. All fallible engine functions return [`AppError`];
//! the library never exits the process.

use thiserror::Error;

/// An error produced by the diagnostic engine. Error messages are redaction-safe
/// (any embedded URL is redacted before it reaches a message).
#[derive(Debug, Error)]
pub enum AppError {
    /// The RPC URL could not be parsed or used an unsupported scheme.
    #[error("invalid RPC URL: {reason}")]
    InvalidRpcUrl { reason: String },

    /// The HTTP client could not be constructed.
    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[source] reqwest::Error),

    /// An RPC request failed at the transport level.
    #[error("RPC request failed for {method}: {source}")]
    RpcRequest {
        method: &'static str,
        #[source]
        source: reqwest::Error,
    },

    /// The RPC responded, but the body did not match the expected shape.
    #[error("unexpected RPC response for {method}: {reason}")]
    UnexpectedRpcResponse {
        method: &'static str,
        reason: String,
    },

    /// A report could not be serialized to JSON.
    #[error("failed to serialize report: {0}")]
    SerializeReport(#[source] serde_json::Error),

    /// `compare` was invoked with fewer than two RPC URLs.
    #[error("compare requires at least 2 RPC URLs")]
    CompareRequiresTwoRpcUrls,

    /// The Markdown report could not be written to disk.
    #[error("failed to write Markdown report to {path}: {source}")]
    WriteMarkdownReport {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// `--x-token-env` named an environment variable that is unset or empty. The
    /// message names the variable only; the token value is never read into it.
    #[error("x-token environment variable '{var}' is not set or is empty")]
    MissingTokenEnv { var: String },

    /// The resolved x-token is not valid gRPC metadata (must be ASCII). The
    /// value itself is never included in the message.
    #[error("x-token value is not valid gRPC metadata (must be ASCII)")]
    InvalidTokenValue,
}
