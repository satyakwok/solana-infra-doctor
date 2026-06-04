//! JSON-RPC request/response wire types and Solana RPC result models.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: &'static str,
    pub params: Vec<Value>,
}

impl JsonRpcRequest {
    /// A request for `method` with no parameters.
    pub fn new(id: u64, method: &'static str) -> Self {
        Self::with_params(id, method, Vec::new())
    }

    /// A request for `method` with the given `params`.
    pub fn with_params(id: u64, method: &'static str, params: Vec<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

/// A JSON-RPC 2.0 response envelope with a typed `result`.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC error object (`code` and `message`).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// The result of `getVersion`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionInfo {
    #[serde(rename = "solana-core")]
    pub solana_core: String,
    #[serde(default)]
    pub feature_set: Option<u64>,
}

/// The result of `getLatestBlockhash`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LatestBlockhashResponse {
    pub value: LatestBlockhashValue,
}

/// The `value` payload of a `getLatestBlockhash` response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LatestBlockhashValue {
    pub blockhash: String,
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: u64,
}

/// The result of `isBlockhashValid`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BlockhashValidResponse {
    pub value: bool,
}

/// The result of `getAccountInfo`. `value` is `null` when the account does not
/// exist, so it is modeled as `Option`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountInfoResponse {
    pub value: Option<AccountInfo>,
}

/// An account's fields from `getAccountInfo` (base64 encoding). Only the fields
/// needed for program-readiness are modeled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountInfo {
    /// The account's owner program (a loader, for an executable program).
    pub owner: String,
    /// Whether the account is an executable program.
    pub executable: bool,
    /// The account's data length in bytes, when the RPC reports it.
    #[serde(default)]
    pub space: Option<u64>,
    /// The raw `[data, encoding]` pair, used as a fallback length source when
    /// `space` is absent.
    #[serde(default)]
    pub data: Option<Value>,
}

impl AccountInfo {
    /// The account's data length in bytes: `space` when present, otherwise
    /// derived from the base64 `data` payload.
    pub fn data_len(&self) -> u64 {
        if let Some(space) = self.space {
            return space;
        }
        match &self.data {
            Some(Value::Array(parts)) => parts
                .first()
                .and_then(Value::as_str)
                .map(base64_decoded_len)
                .unwrap_or(0),
            _ => 0,
        }
    }
}

/// Decoded byte length of a base64 string, computed without decoding it.
fn base64_decoded_len(encoded: &str) -> u64 {
    let len = encoded.len() as u64;
    if len == 0 {
        return 0;
    }
    let padding = encoded.bytes().rev().take_while(|&b| b == b'=').count() as u64;
    // `saturating_sub` guards against a malformed payload (e.g. all-padding)
    // where `padding` could exceed the byte estimate and underflow.
    ((len / 4) * 3).saturating_sub(padding)
}

/// One entry from `getRecentPrioritizationFees`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PrioritizationFee {
    /// The slot the fee was observed in.
    pub slot: u64,
    /// The per-compute-unit prioritization fee (micro-lamports), `0` when none.
    #[serde(rename = "prioritizationFee")]
    pub prioritization_fee: u64,
}

/// One entry from `getRecentPerformanceSamples`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PerformanceSample {
    pub slot: u64,
    #[serde(rename = "numSlots")]
    pub num_slots: u64,
    #[serde(rename = "numTransactions")]
    pub num_transactions: u64,
    #[serde(rename = "samplePeriodSecs")]
    pub sample_period_secs: u64,
    #[serde(rename = "numNonVoteTransactions", default)]
    pub num_non_vote_transactions: Option<u64>,
}
