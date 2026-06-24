//! The crate's error type. All fallible engine functions return [`AppError`];
//! the library never exits the process.

use thiserror::Error;

/// An error produced by the diagnostic engine. Error messages are redaction-safe
/// (any embedded URL is redacted before it reaches a message).
#[derive(Debug, Error)]
pub enum AppError {
    /// The RPC URL could not be parsed or used an unsupported scheme.
    #[error("invalid RPC URL: {reason} (accepted schemes: http://, https://, ws://, wss://)")]
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

    /// `grpc compare` was invoked with fewer than two gRPC endpoints.
    #[error("grpc compare requires at least 2 gRPC endpoints")]
    GrpcCompareRequiresTwoEndpoints,

    /// The number of `--x-token-env` values did not pair with `--grpc`: it must
    /// be `0` (all anonymous), `1` (shared), or one per endpoint.
    #[error(
        "--x-token-env count ({tokens}) must be 0, 1, or equal to the number of --grpc endpoints ({endpoints})"
    )]
    GrpcCompareTokenCountMismatch { endpoints: usize, tokens: usize },

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

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn invalid_rpc_url_includes_scheme_hint() {
        let err = AppError::InvalidRpcUrl {
            reason: "bad url".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("http://"));
        assert!(msg.contains("https://"));
        assert!(msg.contains("ws://"));
        assert!(msg.contains("wss://"));
    }
}
