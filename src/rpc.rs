use crate::error::AppError;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone)]
pub struct RpcEndpoint {
    url: Url,
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
        let mut url = self.url.clone();
        if url.password().is_some() {
            let _ = url.set_password(Some("***"));
        }
        if !url.username().is_empty() {
            let _ = url.set_username("***");
        }
        url.set_query(None);
        url.to_string()
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
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params: Vec::new(),
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

#[cfg(test)]
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
    fn redacts_credentials_and_query() {
        let endpoint =
            RpcEndpoint::parse("https://user:pass@example.com/rpc?api-key=secret").unwrap();
        assert_eq!(endpoint.redacted(), "https://***:***@example.com/rpc");
    }
}
