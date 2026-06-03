use crate::{
    cli::CheckArgs,
    error::AppError,
    latency::{average_latency_ms, Latency},
    rpc::{JsonRpcRequest, JsonRpcResponse, RpcClient, RpcEndpoint, VersionInfo},
    verdict::Verdict,
};
use serde::Serialize;
use std::time::{Duration, Instant};

const GOOD_AVERAGE_LATENCY_MS: u128 = 500;
const WARNING_AVERAGE_LATENCY_MS: u128 = 1_500;

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub verdict: Verdict,
    pub rpc_url: String,
    pub summary: String,
    pub average_latency_ms: Option<u128>,
    pub checks: Vec<RpcCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcCheck {
    pub method: &'static str,
    pub status: CheckStatus,
    pub latency_ms: Option<u128>,
    pub detail: String,
}

impl RpcCheck {
    fn success(method: &'static str, latency: Latency, detail: String) -> Self {
        Self {
            method,
            status: CheckStatus::Success,
            latency_ms: Some(latency.millis),
            detail,
        }
    }

    fn failed(method: &'static str, latency: Option<Latency>, detail: String) -> Self {
        Self {
            method,
            status: CheckStatus::Failed,
            latency_ms: latency.map(|value| value.millis),
            detail,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Success,
    Failed,
}

pub async fn run_check(args: CheckArgs) -> Result<CheckReport, AppError> {
    let endpoint = match RpcEndpoint::parse(&args.rpc) {
        Ok(endpoint) => endpoint,
        Err(AppError::InvalidRpcUrl { reason }) => {
            return Ok(CheckReport {
                verdict: Verdict::Bad,
                rpc_url: "<invalid>".to_string(),
                summary: format!("invalid RPC URL: {reason}"),
                average_latency_ms: None,
                checks: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };
    let redacted_rpc_url = endpoint.redacted();
    let client = RpcClient::new(endpoint, Duration::from_millis(args.timeout_ms))?;

    let checks = vec![
        check_health(&client).await,
        check_version(&client).await,
        check_genesis_hash(&client).await,
        check_slot(&client).await,
    ];

    let average_latency_ms = average_latency_ms(
        checks
            .iter()
            .filter_map(|check| check.latency_ms.map(|millis| Latency { millis })),
    );
    let verdict = calculate_verdict(&checks, average_latency_ms);
    let summary = summarize(verdict, &checks, average_latency_ms);

    Ok(CheckReport {
        verdict,
        rpc_url: redacted_rpc_url,
        summary,
        average_latency_ms,
        checks,
    })
}

async fn check_health(client: &RpcClient) -> RpcCheck {
    match call_rpc::<String>(client, 1, "getHealth").await {
        Ok((response, latency)) => match response.result {
            Some(value) if value == "ok" => {
                RpcCheck::success("getHealth", latency, "health is ok".to_string())
            }
            Some(value) => RpcCheck::failed(
                "getHealth",
                Some(latency),
                format!("unexpected health response: {value}"),
            ),
            None => RpcCheck::failed(
                "getHealth",
                Some(latency),
                response_error_detail(&response, "missing result"),
            ),
        },
        Err(error) => RpcCheck::failed("getHealth", None, error.to_string()),
    }
}

async fn check_version(client: &RpcClient) -> RpcCheck {
    match call_rpc::<VersionInfo>(client, 2, "getVersion").await {
        Ok((response, latency)) => match response.result {
            Some(version) => RpcCheck::success(
                "getVersion",
                latency,
                format!("solana-core {}", version.solana_core),
            ),
            None => RpcCheck::failed(
                "getVersion",
                Some(latency),
                response_error_detail(&response, "missing result"),
            ),
        },
        Err(error) => RpcCheck::failed("getVersion", None, error.to_string()),
    }
}

async fn check_genesis_hash(client: &RpcClient) -> RpcCheck {
    match call_rpc::<String>(client, 3, "getGenesisHash").await {
        Ok((response, latency)) => match response.result {
            Some(hash) if !hash.trim().is_empty() => {
                RpcCheck::success("getGenesisHash", latency, hash)
            }
            Some(_) => RpcCheck::failed(
                "getGenesisHash",
                Some(latency),
                "empty genesis hash".to_string(),
            ),
            None => RpcCheck::failed(
                "getGenesisHash",
                Some(latency),
                response_error_detail(&response, "missing result"),
            ),
        },
        Err(error) => RpcCheck::failed("getGenesisHash", None, error.to_string()),
    }
}

async fn check_slot(client: &RpcClient) -> RpcCheck {
    match call_rpc::<u64>(client, 4, "getSlot").await {
        Ok((response, latency)) => match response.result {
            Some(slot) => RpcCheck::success("getSlot", latency, format!("slot {slot}")),
            None => RpcCheck::failed(
                "getSlot",
                Some(latency),
                response_error_detail(&response, "missing result"),
            ),
        },
        Err(error) => RpcCheck::failed("getSlot", None, error.to_string()),
    }
}

async fn call_rpc<T>(
    client: &RpcClient,
    id: u64,
    method: &'static str,
) -> Result<(JsonRpcResponse<T>, Latency), AppError>
where
    T: serde::de::DeserializeOwned,
{
    let request = JsonRpcRequest::new(id, method);
    let started = Instant::now();
    let response = client
        .call::<T>(&request)
        .await
        .map_err(|source| AppError::RpcRequest { method, source })?;
    if response.jsonrpc != "2.0" {
        return Err(AppError::UnexpectedRpcResponse {
            method,
            reason: format!("expected JSON-RPC 2.0, got {}", response.jsonrpc),
        });
    }
    if response.id != id {
        return Err(AppError::UnexpectedRpcResponse {
            method,
            reason: format!("expected response id {id}, got {}", response.id),
        });
    }
    let latency = Latency::from_duration(started.elapsed());

    Ok((response, latency))
}

fn response_error_detail<T>(response: &JsonRpcResponse<T>, fallback: &str) -> String {
    response.error.as_ref().map_or_else(
        || fallback.to_string(),
        |error| format!("RPC error {}: {}", error.code, error.message),
    )
}

pub fn calculate_verdict(checks: &[RpcCheck], average_latency_ms: Option<u128>) -> Verdict {
    if checks.is_empty() {
        return Verdict::Unknown;
    }

    let failed_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .count();
    let timeout_like_failures = checks
        .iter()
        .filter(|check| {
            check.status == CheckStatus::Failed
                && check.detail.to_ascii_lowercase().contains("timeout")
        })
        .count();

    if failed_count >= 2 || timeout_like_failures >= 2 {
        return Verdict::Bad;
    }

    if average_latency_ms.is_some_and(|latency| latency > WARNING_AVERAGE_LATENCY_MS) {
        return Verdict::Bad;
    }

    if failed_count == 1
        || average_latency_ms.is_some_and(|latency| {
            latency > GOOD_AVERAGE_LATENCY_MS && latency <= WARNING_AVERAGE_LATENCY_MS
        })
    {
        return Verdict::Warning;
    }

    if failed_count == 0 && average_latency_ms.is_some() {
        Verdict::Good
    } else {
        Verdict::Unknown
    }
}

fn summarize(verdict: Verdict, checks: &[RpcCheck], average_latency_ms: Option<u128>) -> String {
    let failed_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .count();

    match verdict {
        Verdict::Good => "all required RPC checks succeeded".to_string(),
        Verdict::Warning => {
            if failed_count > 0 {
                format!("RPC is reachable, but {failed_count} check failed")
            } else {
                let latency = average_latency_ms.unwrap_or_default();
                format!("RPC checks succeeded, but average latency is elevated at {latency}ms")
            }
        }
        Verdict::Bad => {
            if failed_count > 0 {
                format!("{failed_count} required RPC checks failed")
            } else {
                let latency = average_latency_ms.unwrap_or_default();
                format!("average latency is too high at {latency}ms")
            }
        }
        Verdict::Unknown => "not enough data to produce a reliable verdict".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success(method: &'static str, latency_ms: u128) -> RpcCheck {
        RpcCheck {
            method,
            status: CheckStatus::Success,
            latency_ms: Some(latency_ms),
            detail: "ok".to_string(),
        }
    }

    fn failed(method: &'static str) -> RpcCheck {
        RpcCheck {
            method,
            status: CheckStatus::Failed,
            latency_ms: None,
            detail: "request failed".to_string(),
        }
    }

    #[test]
    fn verdict_good_when_all_checks_pass_quickly() {
        let checks = vec![
            success("getHealth", 100),
            success("getVersion", 120),
            success("getGenesisHash", 110),
            success("getSlot", 90),
        ];
        assert_eq!(calculate_verdict(&checks, Some(105)), Verdict::Good);
    }

    #[test]
    fn verdict_warning_for_one_failed_check() {
        let checks = vec![
            success("getHealth", 100),
            failed("getVersion"),
            success("getGenesisHash", 110),
            success("getSlot", 90),
        ];
        assert_eq!(calculate_verdict(&checks, Some(100)), Verdict::Warning);
    }

    #[test]
    fn verdict_warning_for_elevated_latency() {
        let checks = vec![success("getHealth", 700)];
        assert_eq!(calculate_verdict(&checks, Some(700)), Verdict::Warning);
    }

    #[test]
    fn verdict_bad_for_repeated_failures() {
        let checks = vec![failed("getHealth"), failed("getVersion")];
        assert_eq!(calculate_verdict(&checks, None), Verdict::Bad);
    }

    #[test]
    fn verdict_bad_for_high_latency() {
        let checks = vec![success("getHealth", 2_000)];
        assert_eq!(calculate_verdict(&checks, Some(2_000)), Verdict::Bad);
    }

    #[test]
    fn verdict_unknown_without_checks() {
        assert_eq!(calculate_verdict(&[], None), Verdict::Unknown);
    }
}
