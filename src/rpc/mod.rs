//! The HTTP JSON-RPC client, endpoint parsing/validation, and wire models. URLs
//! are treated as secret-bearing: they are redacted before appearing in any
//! output, `Debug`, or error.

use crate::error::AppError;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;
use url::Url;

pub mod models;
pub mod resilience;
pub use models::*;

use resilience::{is_transient, Resilience};

/// A validated `http`/`https` RPC endpoint. Its `Debug` is redacted so credentials
/// in the URL never leak through logging or panics.
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
    /// Parse and validate an RPC URL, rejecting non-`http(s)` schemes and
    /// hostless URLs.
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

    /// The underlying parsed URL.
    pub fn as_url(&self) -> &Url {
        &self.url
    }

    /// The URL with any credentials or likely API key redacted, safe to display.
    pub fn redacted(&self) -> String {
        crate::redact::redact_url(&self.url)
    }
}

/// A `reqwest`-backed JSON-RPC client bound to a single [`RpcEndpoint`], with a
/// per-endpoint rate limiter and transient-error retry (see [`resilience`]).
#[derive(Debug)]
pub struct RpcClient {
    endpoint: RpcEndpoint,
    client: Client,
    resilience: Resilience,
}

impl RpcClient {
    /// Build a client for `endpoint` with the given per-request `timeout`.
    pub fn new(endpoint: RpcEndpoint, timeout: Duration) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(AppError::HttpClient)?;

        Ok(Self {
            endpoint,
            client,
            resilience: Resilience::new(),
        })
    }

    /// Send a JSON-RPC `request` and deserialize the typed response body, pacing
    /// the call through the rate limiter and retrying transient failures with
    /// exponential backoff.
    pub async fn call<T>(
        &self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<T>, reqwest::Error>
    where
        T: DeserializeOwned,
    {
        let mut attempt = 0u32;
        loop {
            self.resilience.acquire().await;
            match self.send_once::<T>(request).await {
                Ok(response) => return Ok(response),
                Err(error) => match self.resilience.retry_delay(attempt) {
                    Some(delay) if is_transient(&error) => {
                        // Never log the error itself — it carries the URL.
                        tracing::debug!(retry = attempt + 1, "transient RPC error; retrying");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                    _ => return Err(error),
                },
            }
        }
    }

    async fn send_once<T>(
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
