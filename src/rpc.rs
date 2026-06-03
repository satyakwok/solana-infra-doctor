use crate::error::AppError;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use url::Url;

#[derive(Clone)]
pub struct RpcEndpoint {
    url: Url,
}

// Custom Debug so the raw URL (which may carry credentials or API keys) never
// leaks through `{:?}`, tracing, or panic messages.
impl std::fmt::Debug for RpcEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RpcEndpoint")
            .field("url", &self.redacted())
            .finish()
    }
}

impl RpcEndpoint {
    pub fn parse(input: &str) -> Result<Self, AppError> {
        let url = Url::parse(input).map_err(|error| AppError::InvalidRpcUrl {
            reason: error.to_string(),
        })?;

        match url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(AppError::InvalidRpcUrl {
                    reason: format!("unsupported scheme '{scheme}', expected http or https"),
                });
            }
        }

        if url.host_str().is_none() {
            return Err(AppError::InvalidRpcUrl {
                reason: "missing host".to_string(),
            });
        }

        Ok(Self { url })
    }

    pub fn as_url(&self) -> &Url {
        &self.url
    }

    pub fn redacted(&self) -> String {
        crate::redact::redact_url(&self.url)
    }
}

#[derive(Debug, Clone)]
pub struct RpcClient {
    endpoint: RpcEndpoint,
    client: Client,
}

impl RpcClient {
    pub fn new(endpoint: RpcEndpoint, timeout: Duration) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(AppError::HttpClient)?;

        Ok(Self { endpoint, client })
    }

    pub async fn call<T>(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<T>, reqwest::Error>
    where
        T: DeserializeOwned,
    {
        self.client
            .post(self.endpoint.as_url().clone())
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<JsonRpcResponse<T>>()
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: &'static str,
    pub params: Vec<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &'static str) -> Self {
        Self::with_params(id, method, Vec::new())
    }

    pub fn with_params(id: u64, method: &'static str, params: Vec<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionInfo {
    #[serde(rename = "solana-core")]
    pub solana_core: String,
    #[serde(default)]
    pub feature_set: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LatestBlockhashResponse {
    pub value: LatestBlockhashValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LatestBlockhashValue {
    pub blockhash: String,
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BlockhashValidResponse {
    pub value: bool,
}

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

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn validates_http_rpc_url() {
        let endpoint = RpcEndpoint::parse("https://api.mainnet-beta.solana.com").unwrap();
        assert_eq!(endpoint.as_url().scheme(), "https");
    }

    #[test]
    fn rejects_non_http_rpc_url() {
        let error = RpcEndpoint::parse("ftp://example.com").unwrap_err();
        assert!(error.to_string().contains("unsupported scheme"));
    }

    #[test]
    fn rejects_invalid_rpc_url() {
        let error = RpcEndpoint::parse("not a url").unwrap_err();
        assert!(error.to_string().contains("invalid RPC URL"));
    }

    #[test]
    fn builds_json_rpc_request() {
        let request = JsonRpcRequest::new(7, "getHealth");
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, 7);
        assert_eq!(request.method, "getHealth");
        assert!(request.params.is_empty());
    }

    #[test]
    fn builds_json_rpc_request_with_params() {
        let request = JsonRpcRequest::with_params(8, "isBlockhashValid", vec!["abc".into()]);
        assert_eq!(request.method, "isBlockhashValid");
        assert_eq!(request.params[0], "abc");
    }

    #[test]
    fn parses_latest_blockhash_response() {
        let json = r#"{
            "value": {
                "blockhash": "ExampleBlockhash111111111111111111111111111111",
                "lastValidBlockHeight": 123456
            }
        }"#;
        let parsed: LatestBlockhashResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.value.blockhash,
            "ExampleBlockhash111111111111111111111111111111"
        );
        assert_eq!(parsed.value.last_valid_block_height, 123456);
    }

    #[test]
    fn parses_blockhash_valid_response() {
        let parsed: BlockhashValidResponse = serde_json::from_str(r#"{"value":true}"#).unwrap();
        assert!(parsed.value);
    }

    #[test]
    fn parses_performance_sample() {
        let json = r#"{
            "slot": 10,
            "numSlots": 64,
            "numTransactions": 1200,
            "samplePeriodSecs": 60,
            "numNonVoteTransactions": 300
        }"#;
        let parsed: PerformanceSample = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.slot, 10);
        assert_eq!(parsed.num_slots, 64);
        assert_eq!(parsed.num_transactions, 1200);
        assert_eq!(parsed.sample_period_secs, 60);
        assert_eq!(parsed.num_non_vote_transactions, Some(300));
    }

    #[test]
    fn redacts_credentials_and_query() {
        let endpoint =
            RpcEndpoint::parse("https://user:pass@example.com/rpc?api-key=secret").unwrap();
        assert_eq!(endpoint.redacted(), "https://***:***@example.com/rpc");
    }
}
