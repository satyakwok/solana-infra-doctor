use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid RPC URL: {reason}")]
    InvalidRpcUrl { reason: String },

    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[source] reqwest::Error),

    #[error("RPC request failed for {method}: {source}")]
    RpcRequest {
        method: &'static str,
        #[source]
        source: reqwest::Error,
    },

    #[error("unexpected RPC response for {method}: {reason}")]
    UnexpectedRpcResponse {
        method: &'static str,
        reason: String,
    },

    #[error("failed to serialize report: {0}")]
    SerializeReport(#[source] serde_json::Error),
}
