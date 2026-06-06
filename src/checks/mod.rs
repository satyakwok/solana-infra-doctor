//! Single-endpoint HTTP JSON-RPC readiness diagnostics: run the core, blockhash,
//! and performance checks against one RPC endpoint and produce a [`CheckReport`].

use crate::{
    cli::CheckArgs,
    error::AppError,
    latency::{average_latency_ms, Latency, LatencyStats},
    rpc::{
        AccountInfoResponse, BlockhashValidResponse, JsonRpcRequest, JsonRpcResponse,
        LatestBlockhashResponse, PerformanceSample, PrioritizationFee, RpcClient, RpcEndpoint,
        VersionInfo,
    },
    verdict::Verdict,
};
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};

pub mod verdict;
pub use verdict::calculate_verdict;
use verdict::summarize;

/// Schema version for the `check --json` result shape. Bump on any
/// backward-incompatible change to the serialized fields.
pub const CHECK_SCHEMA_VERSION: u32 = 1;

/// The full result of diagnosing a single RPC endpoint. This is the serialized
/// shape emitted by `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    /// Schema version for the result shape (see [`CHECK_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Overall readiness verdict (drives the process exit code).
    pub verdict: Verdict,
    /// The redacted RPC URL that was diagnosed.
    pub rpc_url: String,
    /// One-line, human-readable summary of the verdict.
    pub summary: String,
    /// Mean latency across the individual checks, if any succeeded.
    pub average_latency_ms: Option<u128>,
    /// Percentile latency summary from repeat sampling (`--samples`), if requested.
    pub latency_samples: Option<LatencyStats>,
    /// Seconds the finalized chain tip's block time lags wall-clock time — a
    /// freshness signal (lower is fresher). `None` if `getBlockTime` failed.
    pub block_time_lag_secs: Option<i64>,
    /// Median recent prioritization fee (micro-lamports/CU). Chain-wide context,
    /// not an endpoint-quality signal. `None` if `getRecentPrioritizationFees`
    /// failed.
    pub prioritization_fee_median: Option<u64>,
    /// Whether the SPL Token Program account is served as an executable program
    /// (token transaction workloads depend on it).
    pub token_program_ready: bool,
    /// Whether the Token-2022 program account is served as an executable program.
    pub token_2022_ready: bool,
    /// `getProgramAccounts` enablement, when `--data` was set (`None` otherwise).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_accounts: Option<ProgramAccountsReadiness>,
    /// The oldest slot the endpoint can serve (`getFirstAvailableBlock`), when
    /// `--data` was set. `0` means history from genesis (full archival).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_available_slot: Option<u64>,
    /// Archival depth in slots behind the current slot, when computable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archival_depth_slots: Option<u64>,
    /// Whether `--fail-on-warning` was set (surfaced for CI context).
    pub fail_on_warning: bool,
    /// The individual per-method check results.
    pub checks: Vec<RpcCheck>,
}

/// The result of a single JSON-RPC method check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcCheck {
    /// Which diagnostic category this check belongs to.
    pub category: CheckCategory,
    /// The JSON-RPC method that was called.
    pub method: &'static str,
    /// Whether the check succeeded or failed.
    pub status: CheckStatus,
    /// Round-trip latency for the call, if it completed.
    pub latency_ms: Option<u128>,
    /// Human-readable detail or error message (redacted).
    pub detail: String,
    /// Classified error kind when the check failed.
    pub error_kind: Option<ErrorKind>,
    /// Whether a failure of this check is critical to readiness.
    pub critical: bool,
}

impl RpcCheck {
    fn success(
        category: CheckCategory,
        method: &'static str,
        latency: Latency,
        detail: String,
    ) -> Self {
        Self {
            category,
            method,
            status: CheckStatus::Success,
            latency_ms: Some(latency.millis),
            detail,
            error_kind: None,
            critical: category.is_critical(),
        }
    }

    fn failed(
        category: CheckCategory,
        method: &'static str,
        latency: Option<Latency>,
        detail: String,
        error_kind: ErrorKind,
    ) -> Self {
        Self {
            category,
            method,
            status: CheckStatus::Failed,
            latency_ms: latency.map(|value| value.millis),
            detail,
            error_kind: Some(error_kind),
            critical: category.is_critical(),
        }
    }
}

/// The diagnostic category a check belongs to. `Core` and `Blockhash` checks are
/// critical to readiness; `Performance`, `Token`, and `Data` checks are
/// informational.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckCategory {
    Core,
    Blockhash,
    Performance,
    Token,
    /// Data-capability checks (`getProgramAccounts` enablement, archival depth),
    /// run only with `--data`. Informational: they report capability facts and do
    /// not, on their own, make a general endpoint unusable.
    Data,
}

impl CheckCategory {
    /// The human-readable category name (`Core`, `Blockhash`, `Performance`,
    /// `Token`, `Data`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Blockhash => "Blockhash",
            Self::Performance => "Performance",
            Self::Token => "Token",
            Self::Data => "Data",
        }
    }

    fn is_critical(self) -> bool {
        matches!(self, Self::Core | Self::Blockhash)
    }
}

/// Whether the endpoint serves `getProgramAccounts` — the readiness most indexer
/// and data-pipeline workloads depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramAccountsReadiness {
    /// The method is enabled (the bounded probe returned a result set).
    Ready,
    /// The method responded but is unavailable for the probed program (disabled,
    /// or the program is excluded from the account secondary indexes).
    Gated,
    /// The probe could not complete (timeout or transport failure).
    Degraded,
}

/// Whether an individual check succeeded or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Success,
    Failed,
}

/// A classified failure cause, used to drive verdicts and machine-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    InvalidUrl,
    Timeout,
    HttpError,
    RpcError,
    MalformedResponse,
    UnknownError,
}

impl ErrorKind {
    /// The stable snake_case identifier used in JSON output and reports.
    pub fn label(self) -> &'static str {
        match self {
            Self::InvalidUrl => "invalid_url",
            Self::Timeout => "timeout",
            Self::HttpError => "http_error",
            Self::RpcError => "rpc_error",
            Self::MalformedResponse => "malformed_response",
            Self::UnknownError => "unknown_error",
        }
    }
}

/// Diagnose a single RPC endpoint: run every readiness check, measure latency,
/// compute the verdict, and return a redaction-safe [`CheckReport`].
pub async fn run_check(args: CheckArgs) -> Result<CheckReport, AppError> {
    let endpoint = match RpcEndpoint::parse(&args.rpc) {
        Ok(endpoint) => endpoint,
        Err(AppError::InvalidRpcUrl { reason }) => {
            let reason = crate::redact::redact_text(&reason);
            return Ok(CheckReport {
                schema_version: CHECK_SCHEMA_VERSION,
                verdict: Verdict::Bad,
                rpc_url: "<invalid>".to_string(),
                summary: format!("invalid RPC URL: {reason}"),
                average_latency_ms: None,
                latency_samples: None,
                block_time_lag_secs: None,
                prioritization_fee_median: None,
                token_program_ready: false,
                token_2022_ready: false,
                program_accounts: None,
                oldest_available_slot: None,
                archival_depth_slots: None,
                fail_on_warning: args.fail_on_warning,
                checks: vec![RpcCheck::failed(
                    CheckCategory::Core,
                    "url_validation",
                    None,
                    reason,
                    ErrorKind::InvalidUrl,
                )],
            });
        }
        Err(error) => return Err(error),
    };
    let redacted_rpc_url = endpoint.redacted();
    let client = RpcClient::new(endpoint, Duration::from_millis(args.timeout_ms))?;

    let mut checks = vec![
        check_health(&client).await,
        check_version(&client).await,
        check_genesis_hash(&client).await,
        check_slot(&client).await,
    ];

    let latest_blockhash = check_latest_blockhash(&client).await;
    let blockhash = latest_blockhash
        .status
        .eq(&CheckStatus::Success)
        .then(|| latest_blockhash.detail.clone());
    checks.push(latest_blockhash);
    checks.push(check_blockhash_valid(&client, blockhash.as_deref()).await);
    checks.push(check_performance_samples(&client).await);

    let (block_time_check, block_time_lag_secs) = check_block_time(&client).await;
    checks.push(block_time_check);
    let (fee_check, prioritization_fee_median) = check_prioritization_fees(&client).await;
    checks.push(fee_check);

    let (token_program_check, token_program_ready) = check_token_program(&client).await;
    checks.push(token_program_check);
    let (token_2022_check, token_2022_ready) = check_token_2022(&client).await;
    checks.push(token_2022_check);

    // Optional data-readiness checks (`--data`): getProgramAccounts enablement and
    // archival history depth. These are capability probes, not request-latency
    // measurements, so they are excluded from the latency average below.
    let (program_accounts, oldest_available_slot, archival_depth_slots) = if args.data {
        let program = args.data_program.as_deref().unwrap_or(DEFAULT_DATA_PROGRAM);
        let (gpa_check, readiness) = check_program_accounts(&client, program).await;
        checks.push(gpa_check);
        let current_slot = current_slot_from_checks(&checks);
        let (archival_check, oldest, depth) = check_archival_depth(&client, current_slot).await;
        checks.push(archival_check);
        (Some(readiness), oldest, depth)
    } else {
        (None, None, None)
    };

    // Average latency reflects the request-latency methods only; data-capability
    // probes (which can be slow without meaning the endpoint is slow) never
    // pollute the latency that drives the verdict.
    let average_latency_ms = average_latency_ms(
        checks
            .iter()
            .filter(|check| check.category != CheckCategory::Data)
            .filter_map(|check| check.latency_ms.map(|millis| Latency { millis })),
    );

    // Optional repeat-sampling latency probe (`--samples`). The verdict is still
    // driven by the per-check results; this only enriches the latency picture.
    let latency_samples = if args.samples > 1 {
        probe_latency(&client, args.samples).await
    } else {
        None
    };

    let verdict = calculate_verdict(&checks, average_latency_ms);
    let summary = summarize(verdict, &checks, average_latency_ms, args.fail_on_warning);

    Ok(CheckReport {
        schema_version: CHECK_SCHEMA_VERSION,
        verdict,
        rpc_url: redacted_rpc_url,
        summary,
        average_latency_ms,
        latency_samples,
        block_time_lag_secs,
        prioritization_fee_median,
        token_program_ready,
        token_2022_ready,
        program_accounts,
        oldest_available_slot,
        archival_depth_slots,
        fail_on_warning: args.fail_on_warning,
        checks,
    })
}

/// Probe round-trip latency by sending `samples` lightweight `getHealth`
/// requests and summarizing the measured latencies. Failed probes are skipped;
/// returns `None` if none succeeded. Presentation/diagnostic only — it does not
/// affect the verdict.
async fn probe_latency(client: &RpcClient, samples: u32) -> Option<LatencyStats> {
    let mut measured = Vec::with_capacity(samples as usize);
    for _ in 0..samples {
        // `getHealth` is the lightest method; id 1 matches the getHealth check
        // (a JSON-RPC server echoes the request id).
        if let Ok((_response, latency)) =
            call_rpc::<String>(client, 1, "getHealth", Vec::new()).await
        {
            measured.push(latency.millis);
        }
    }
    LatencyStats::from_samples(&measured)
}

async fn check_health(client: &RpcClient) -> RpcCheck {
    match call_rpc::<String>(client, 1, "getHealth", Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(value) if value == "ok" => RpcCheck::success(
                CheckCategory::Core,
                "getHealth",
                latency,
                "health is ok".to_string(),
            ),
            Some(value) => RpcCheck::failed(
                CheckCategory::Core,
                "getHealth",
                Some(latency),
                format!("unexpected health response: {value}"),
                ErrorKind::MalformedResponse,
            ),
            None => {
                failed_from_response(CheckCategory::Core, "getHealth", Some(latency), &response)
            }
        },
        Err(error) => failed_from_error(CheckCategory::Core, "getHealth", error),
    }
}

async fn check_version(client: &RpcClient) -> RpcCheck {
    match call_rpc::<VersionInfo>(client, 2, "getVersion", Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(version) => RpcCheck::success(
                CheckCategory::Core,
                "getVersion",
                latency,
                format!("solana-core {}", version.solana_core),
            ),
            None => {
                failed_from_response(CheckCategory::Core, "getVersion", Some(latency), &response)
            }
        },
        Err(error) => failed_from_error(CheckCategory::Core, "getVersion", error),
    }
}

async fn check_genesis_hash(client: &RpcClient) -> RpcCheck {
    match call_rpc::<String>(client, 3, "getGenesisHash", Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(hash) if !hash.trim().is_empty() => {
                RpcCheck::success(CheckCategory::Core, "getGenesisHash", latency, hash)
            }
            Some(_) => RpcCheck::failed(
                CheckCategory::Core,
                "getGenesisHash",
                Some(latency),
                "empty genesis hash".to_string(),
                ErrorKind::MalformedResponse,
            ),
            None => failed_from_response(
                CheckCategory::Core,
                "getGenesisHash",
                Some(latency),
                &response,
            ),
        },
        Err(error) => failed_from_error(CheckCategory::Core, "getGenesisHash", error),
    }
}

async fn check_slot(client: &RpcClient) -> RpcCheck {
    match call_rpc::<u64>(client, 4, "getSlot", Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(slot) => RpcCheck::success(
                CheckCategory::Core,
                "getSlot",
                latency,
                format!("slot {slot}"),
            ),
            None => failed_from_response(CheckCategory::Core, "getSlot", Some(latency), &response),
        },
        Err(error) => failed_from_error(CheckCategory::Core, "getSlot", error),
    }
}

async fn check_latest_blockhash(client: &RpcClient) -> RpcCheck {
    match call_rpc::<LatestBlockhashResponse>(client, 5, "getLatestBlockhash", Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(blockhash) if !blockhash.value.blockhash.trim().is_empty() => RpcCheck::success(
                CheckCategory::Blockhash,
                "getLatestBlockhash",
                latency,
                blockhash.value.blockhash,
            ),
            Some(_) => RpcCheck::failed(
                CheckCategory::Blockhash,
                "getLatestBlockhash",
                Some(latency),
                "empty latest blockhash".to_string(),
                ErrorKind::MalformedResponse,
            ),
            None => failed_from_response(
                CheckCategory::Blockhash,
                "getLatestBlockhash",
                Some(latency),
                &response,
            ),
        },
        Err(error) => failed_from_error(CheckCategory::Blockhash, "getLatestBlockhash", error),
    }
}

async fn check_blockhash_valid(client: &RpcClient, blockhash: Option<&str>) -> RpcCheck {
    let Some(blockhash) = blockhash else {
        return RpcCheck::failed(
            CheckCategory::Blockhash,
            "isBlockhashValid",
            None,
            "latest blockhash unavailable".to_string(),
            ErrorKind::MalformedResponse,
        );
    };

    match call_rpc::<BlockhashValidResponse>(
        client,
        6,
        "isBlockhashValid",
        vec![Value::String(blockhash.to_string())],
    )
    .await
    {
        Ok((response, latency)) => match response.result {
            Some(validity) if validity.value => RpcCheck::success(
                CheckCategory::Blockhash,
                "isBlockhashValid",
                latency,
                "latest blockhash is valid".to_string(),
            ),
            Some(_) => RpcCheck::failed(
                CheckCategory::Blockhash,
                "isBlockhashValid",
                Some(latency),
                "latest blockhash is not valid".to_string(),
                ErrorKind::RpcError,
            ),
            None => failed_from_response(
                CheckCategory::Blockhash,
                "isBlockhashValid",
                Some(latency),
                &response,
            ),
        },
        Err(error) => failed_from_error(CheckCategory::Blockhash, "isBlockhashValid", error),
    }
}

async fn check_performance_samples(client: &RpcClient) -> RpcCheck {
    match call_rpc::<Vec<PerformanceSample>>(client, 7, "getRecentPerformanceSamples", Vec::new())
        .await
    {
        Ok((response, latency)) => match response.result {
            Some(samples) if !samples.is_empty() => {
                let sample = &samples[0];
                RpcCheck::success(
                    CheckCategory::Performance,
                    "getRecentPerformanceSamples",
                    latency,
                    format!(
                        "{} transactions across {} slots in {}s",
                        sample.num_transactions, sample.num_slots, sample.sample_period_secs
                    ),
                )
            }
            Some(_) => RpcCheck::failed(
                CheckCategory::Performance,
                "getRecentPerformanceSamples",
                Some(latency),
                "no recent performance samples returned".to_string(),
                ErrorKind::MalformedResponse,
            ),
            None => failed_from_response(
                CheckCategory::Performance,
                "getRecentPerformanceSamples",
                Some(latency),
                &response,
            ),
        },
        Err(error) => failed_from_error(
            CheckCategory::Performance,
            "getRecentPerformanceSamples",
            error,
        ),
    }
}

/// Current wall-clock time as a Unix timestamp (seconds).
fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// `getBlockTime` on the latest **finalized** slot, yielding how far the
/// finalized chain tip's block time lags wall-clock time (a freshness signal).
///
/// We use `getBlockTime` rather than `getBlock`: on public RPC, `getBlock`
/// frequently returns "Block not available" for recent slots, whereas
/// `getBlockTime` is reliable for finalized slots and returns exactly the
/// timestamp we need.
async fn check_block_time(client: &RpcClient) -> (RpcCheck, Option<i64>) {
    const METHOD: &str = "getBlockTime";
    let slot = match call_rpc::<u64>(
        client,
        8,
        "getSlot",
        vec![serde_json::json!({"commitment": "finalized"})],
    )
    .await
    {
        Ok((response, latency)) => match response.result {
            Some(slot) => slot,
            None => {
                return (
                    failed_from_response(
                        CheckCategory::Performance,
                        METHOD,
                        Some(latency),
                        &response,
                    ),
                    None,
                )
            }
        },
        Err(error) => {
            return (
                failed_from_error(CheckCategory::Performance, METHOD, error),
                None,
            )
        }
    };

    match call_rpc::<i64>(client, 9, METHOD, vec![serde_json::json!(slot)]).await {
        Ok((response, latency)) => match response.result {
            Some(block_time) => {
                let lag = (unix_now_secs() - block_time).max(0);
                (
                    RpcCheck::success(
                        CheckCategory::Performance,
                        METHOD,
                        latency,
                        format!("finalized block time {lag}s behind wall clock"),
                    ),
                    Some(lag),
                )
            }
            None => (
                failed_from_response(CheckCategory::Performance, METHOD, Some(latency), &response),
                None,
            ),
        },
        Err(error) => (
            failed_from_error(CheckCategory::Performance, METHOD, error),
            None,
        ),
    }
}

/// `getRecentPrioritizationFees`, summarized as the **median** recent
/// per-compute-unit fee. This is chain-wide fee-market context (it does not
/// discriminate between endpoints on the same network), so it is surfaced for
/// information but does not affect the comparison score.
async fn check_prioritization_fees(client: &RpcClient) -> (RpcCheck, Option<u64>) {
    const METHOD: &str = "getRecentPrioritizationFees";
    match call_rpc::<Vec<PrioritizationFee>>(client, 10, METHOD, vec![serde_json::json!([])]).await
    {
        Ok((response, latency)) => match response.result {
            Some(fees) if !fees.is_empty() => {
                let mut values: Vec<u64> = fees.iter().map(|fee| fee.prioritization_fee).collect();
                values.sort_unstable();
                let median = values[values.len() / 2];
                let max = values[values.len() - 1];
                (
                    RpcCheck::success(
                        CheckCategory::Performance,
                        METHOD,
                        latency,
                        format!("median priority fee {median} micro-lamports/CU (max {max})"),
                    ),
                    Some(median),
                )
            }
            Some(_) => (
                RpcCheck::failed(
                    CheckCategory::Performance,
                    METHOD,
                    Some(latency),
                    "no recent prioritization fees returned".to_string(),
                    ErrorKind::MalformedResponse,
                ),
                None,
            ),
            None => (
                failed_from_response(CheckCategory::Performance, METHOD, Some(latency), &response),
                None,
            ),
        },
        Err(error) => (
            failed_from_error(CheckCategory::Performance, METHOD, error),
            None,
        ),
    }
}

/// The canonical SPL Token Program account address.
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// The canonical Token-2022 (Token Extensions) program account address.
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// `getAccountInfo` on the SPL Token Program account: confirms the RPC serves the
/// program as an executable account, which token transaction workloads need.
async fn check_token_program(client: &RpcClient) -> (RpcCheck, bool) {
    check_program_account(client, 11, "Token Program", TOKEN_PROGRAM_ID).await
}

/// `getAccountInfo` on the Token-2022 (Token Extensions) program account.
async fn check_token_2022(client: &RpcClient) -> (RpcCheck, bool) {
    check_program_account(client, 12, "Token-2022", TOKEN_2022_PROGRAM_ID).await
}

/// Shared `getAccountInfo` program-readiness probe: an account is "ready" when it
/// exists, is `executable`, and reports a non-zero data length — i.e. the RPC
/// returns the deployed program rather than a missing or non-program account.
///
/// This is an informational (non-critical) signal: an RPC that cannot serve the
/// token programs is degraded for token-touching workloads, but it does not make
/// the endpoint unusable for non-token traffic, so failures cap the verdict at
/// `WARNING` rather than `BAD`.
async fn check_program_account(
    client: &RpcClient,
    id: u64,
    label: &'static str,
    program_id: &str,
) -> (RpcCheck, bool) {
    const METHOD: &str = "getAccountInfo";
    let params = vec![
        Value::String(program_id.to_string()),
        serde_json::json!({ "encoding": "base64" }),
    ];

    match call_rpc::<AccountInfoResponse>(client, id, METHOD, params).await {
        Ok((response, latency)) => match response.result {
            Some(account_info) => match account_info.value {
                Some(account) => {
                    let data_len = account.data_len();
                    if account.executable && data_len > 0 {
                        (
                            RpcCheck::success(
                                CheckCategory::Token,
                                METHOD,
                                latency,
                                format!(
                                    "{label} ready: executable {data_len}-byte program owned by {}",
                                    account.owner
                                ),
                            ),
                            true,
                        )
                    } else {
                        (
                            RpcCheck::failed(
                                CheckCategory::Token,
                                METHOD,
                                Some(latency),
                                format!(
                                    "{label} account is not an executable program (executable={}, {data_len} bytes)",
                                    account.executable
                                ),
                                ErrorKind::MalformedResponse,
                            ),
                            false,
                        )
                    }
                }
                None => (
                    RpcCheck::failed(
                        CheckCategory::Token,
                        METHOD,
                        Some(latency),
                        format!("{label} account not found"),
                        ErrorKind::MalformedResponse,
                    ),
                    false,
                ),
            },
            None => (
                failed_from_response(CheckCategory::Token, METHOD, Some(latency), &response),
                false,
            ),
        },
        Err(error) => (
            failed_from_error(CheckCategory::Token, METHOD, error),
            false,
        ),
    }
}

/// Default `getProgramAccounts` probe target: the ComputeBudget native program.
/// It owns no accounts (so the bounded probe returns instantly) and is not one of
/// the large programs validators exclude from account secondary indexes, so it
/// cleanly reflects whether the method itself is enabled.
const DEFAULT_DATA_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";

/// Approximate slots per day on Solana (~2.5 slots/sec) for a rough archival-depth
/// estimate in days. Presentation-only; the exact slot count is also reported.
const SLOTS_PER_DAY: u64 = 216_000;

/// The current slot parsed from a successful `getSlot` check (`"slot N"`), used to
/// compute archival depth without an extra request.
fn current_slot_from_checks(checks: &[RpcCheck]) -> Option<u64> {
    checks
        .iter()
        .find(|check| check.method == "getSlot" && check.status == CheckStatus::Success)
        .and_then(|check| check.detail.strip_prefix("slot "))
        .and_then(|slot| slot.parse().ok())
}

/// Probe `getProgramAccounts` enablement with a bounded request: a `dataSize: 1`
/// filter matches no real account (so it returns an empty set) and `dataSlice`
/// length `0` drops account data — proving the method is enabled and accepts
/// filters without enumerating a large account set. This is a capability fact, not
/// a readiness pass/fail: a `Gated` result does not fail the general verdict (only
/// the `compare` indexer profile penalizes it). A transport failure is an
/// informational failure.
async fn check_program_accounts(
    client: &RpcClient,
    program: &str,
) -> (RpcCheck, ProgramAccountsReadiness) {
    const METHOD: &str = "getProgramAccounts";
    let params = vec![
        Value::String(program.to_string()),
        serde_json::json!({
            "encoding": "base64",
            "dataSlice": { "offset": 0, "length": 0 },
            "filters": [ { "dataSize": 1 } ],
        }),
    ];

    match call_rpc::<Vec<Value>>(client, 13, METHOD, params).await {
        Ok((response, latency)) => match response.result {
            Some(accounts) => (
                RpcCheck::success(
                    CheckCategory::Data,
                    METHOD,
                    latency,
                    format!(
                        "getProgramAccounts enabled (probed on {program}, {} matched)",
                        accounts.len()
                    ),
                ),
                ProgramAccountsReadiness::Ready,
            ),
            // An RPC-level error means the server answered "no" for this program
            // (method disabled, or program excluded from secondary indexes). That
            // is a capability fact — record it without failing the general verdict.
            None => {
                let detail = match &response.error {
                    Some(error) => format!(
                        "getProgramAccounts unavailable on {program} (RPC error {})",
                        error.code
                    ),
                    None => format!("getProgramAccounts returned no result on {program}"),
                };
                (
                    RpcCheck::success(CheckCategory::Data, METHOD, latency, detail),
                    ProgramAccountsReadiness::Gated,
                )
            }
        },
        Err(error) => (
            failed_from_error(CheckCategory::Data, METHOD, error),
            ProgramAccountsReadiness::Degraded,
        ),
    }
}

/// Probe archival history depth via `getFirstAvailableBlock`: the oldest slot the
/// endpoint can still serve. `0` means history from genesis (full archival). The
/// depth behind `current_slot` is reported in slots and a rough day estimate.
async fn check_archival_depth(
    client: &RpcClient,
    current_slot: Option<u64>,
) -> (RpcCheck, Option<u64>, Option<u64>) {
    const METHOD: &str = "getFirstAvailableBlock";
    match call_rpc::<u64>(client, 14, METHOD, Vec::new()).await {
        Ok((response, latency)) => match response.result {
            Some(oldest) => {
                let depth = current_slot.map(|current| current.saturating_sub(oldest));
                let detail = if oldest == 0 {
                    "history from genesis (full archival)".to_string()
                } else {
                    match depth {
                        Some(slots) => format!(
                            "history from slot {oldest} (~{slots} slots / ~{} days behind tip)",
                            slots / SLOTS_PER_DAY
                        ),
                        None => format!("history from slot {oldest}"),
                    }
                };
                (
                    RpcCheck::success(CheckCategory::Data, METHOD, latency, detail),
                    Some(oldest),
                    depth,
                )
            }
            None => (
                failed_from_response(CheckCategory::Data, METHOD, Some(latency), &response),
                None,
                None,
            ),
        },
        Err(error) => (
            failed_from_error(CheckCategory::Data, METHOD, error),
            None,
            None,
        ),
    }
}

async fn call_rpc<T>(
    client: &RpcClient,
    id: u64,
    method: &'static str,
    params: Vec<Value>,
) -> Result<(JsonRpcResponse<T>, Latency), AppError>
where
    T: serde::de::DeserializeOwned,
{
    let request = if params.is_empty() {
        JsonRpcRequest::new(id, method)
    } else {
        JsonRpcRequest::with_params(id, method, params)
    };
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

fn failed_from_response<T>(
    category: CheckCategory,
    method: &'static str,
    latency: Option<Latency>,
    response: &JsonRpcResponse<T>,
) -> RpcCheck {
    if let Some(error) = &response.error {
        RpcCheck::failed(
            category,
            method,
            latency,
            format!("RPC error {}: {}", error.code, error.message),
            ErrorKind::RpcError,
        )
    } else {
        RpcCheck::failed(
            category,
            method,
            latency,
            "missing result".to_string(),
            ErrorKind::MalformedResponse,
        )
    }
}

fn failed_from_error(category: CheckCategory, method: &'static str, error: AppError) -> RpcCheck {
    let error_kind = classify_error(&error);
    // reqwest error Display embeds the request URL (with query string), so the
    // message must be redacted before it is stored or shown anywhere.
    let detail = crate::redact::redact_text(&error.to_string());
    RpcCheck::failed(category, method, None, detail, error_kind)
}

fn classify_error(error: &AppError) -> ErrorKind {
    match error {
        AppError::InvalidRpcUrl { .. } => ErrorKind::InvalidUrl,
        AppError::RpcRequest { source, .. } if source.is_timeout() => ErrorKind::Timeout,
        AppError::RpcRequest { source, .. } if source.is_status() => ErrorKind::HttpError,
        AppError::RpcRequest { source, .. } if source.is_decode() => ErrorKind::MalformedResponse,
        AppError::UnexpectedRpcResponse { .. } => ErrorKind::MalformedResponse,
        AppError::RpcRequest { .. }
        | AppError::HttpClient(_)
        | AppError::SerializeReport(_)
        | AppError::CompareRequiresTwoRpcUrls
        | AppError::GrpcCompareRequiresTwoEndpoints
        | AppError::GrpcCompareTokenCountMismatch { .. }
        | AppError::WriteMarkdownReport { .. }
        | AppError::MissingTokenEnv { .. }
        | AppError::InvalidTokenValue => ErrorKind::UnknownError,
    }
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::{self, JoinHandle},
    };

    struct MockRpcServer {
        url: String,
        handle: JoinHandle<()>,
    }

    impl MockRpcServer {
        fn start(responses: Vec<&'static str>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let handle = thread::spawn(move || {
                for response in responses {
                    let (mut stream, _) = listener.accept().unwrap();
                    let _body = read_http_body(&mut stream);
                    write_http_response(&mut stream, response);
                }
            });

            Self { url, handle }
        }

        fn join(self) {
            self.handle.join().unwrap();
        }
    }

    fn read_http_body(stream: &mut TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let bytes_read = stream.read(&mut chunk).unwrap();
            assert!(bytes_read > 0, "connection closed before headers completed");
            buffer.extend_from_slice(&chunk[..bytes_read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_default();

        while buffer.len() < header_end + content_length {
            let bytes_read = stream.read(&mut chunk).unwrap();
            assert!(bytes_read > 0, "connection closed before body completed");
            buffer.extend_from_slice(&chunk[..bytes_read]);
        }

        String::from_utf8(buffer[header_end..header_end + content_length].to_vec()).unwrap()
    }

    fn write_http_response(stream: &mut TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }

    fn health_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#
    }

    fn version_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":2,"result":{"solana-core":"4.0.0","feature-set":123}}"#
    }

    fn genesis_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":3,"result":"5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"}"#
    }

    fn slot_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":4,"result":424013263}"#
    }

    fn latest_blockhash_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":5,"result":{"value":{"blockhash":"7xKXtgQvExample111111111111111111111111111","lastValidBlockHeight":123456}}}"#
    }

    fn blockhash_valid_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":6,"result":{"value":true}}"#
    }

    fn performance_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":7,"result":[{"slot":10,"numSlots":64,"numTransactions":124000,"samplePeriodSecs":60,"numNonVoteTransactions":90000}]}"#
    }

    // The getBlockTime check makes two calls (getSlot finalized, then
    // getBlockTime); getRecentPrioritizationFees is one.
    fn finalized_slot_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":8,"result":100}"#
    }
    fn block_time_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":9,"result":1700000000}"#
    }
    fn fees_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":10,"result":[{"slot":1,"prioritizationFee":0},{"slot":2,"prioritizationFee":150}]}"#
    }
    // Token Program / Token-2022 readiness: getAccountInfo returns an executable
    // program account (space 36, owned by a BPF loader).
    fn token_program_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":11,"result":{"context":{"slot":100},"value":{"owner":"BPFLoaderUpgradeab1e11111111111111111111111","executable":true,"space":36,"lamports":1,"data":["","base64"]}}}"#
    }
    fn token_2022_ok() -> &'static str {
        r#"{"jsonrpc":"2.0","id":12,"result":{"context":{"slot":100},"value":{"owner":"BPFLoaderUpgradeab1e11111111111111111111111","executable":true,"space":36,"lamports":1,"data":["","base64"]}}}"#
    }

    fn success(category: CheckCategory, method: &'static str, latency_ms: u128) -> RpcCheck {
        RpcCheck {
            category,
            method,
            status: CheckStatus::Success,
            latency_ms: Some(latency_ms),
            detail: "ok".to_string(),
            error_kind: None,
            critical: category.is_critical(),
        }
    }

    fn failed(category: CheckCategory, method: &'static str, error_kind: ErrorKind) -> RpcCheck {
        RpcCheck {
            category,
            method,
            status: CheckStatus::Failed,
            latency_ms: None,
            detail: "request failed".to_string(),
            error_kind: Some(error_kind),
            critical: category.is_critical(),
        }
    }

    fn args_for(url: String) -> CheckArgs {
        CheckArgs {
            rpc: url,
            json: false,
            fail_on_warning: false,
            samples: 1,
            data: false,
            data_program: None,
            timeout_ms: 1_000,
        }
    }

    #[tokio::test]
    async fn run_check_returns_good_for_mocked_healthy_rpc() {
        let server = MockRpcServer::start(vec![
            health_ok(),
            version_ok(),
            genesis_ok(),
            slot_ok(),
            latest_blockhash_ok(),
            blockhash_valid_ok(),
            performance_ok(),
            finalized_slot_ok(),
            block_time_ok(),
            fees_ok(),
            token_program_ok(),
            token_2022_ok(),
        ]);

        let expected_url = format!("{}/", server.url);
        let report = run_check(args_for(server.url.clone())).await.unwrap();
        server.join();

        assert_eq!(report.verdict, Verdict::Good);
        assert_eq!(report.rpc_url, expected_url);
        assert_eq!(report.summary, "All RPC readiness checks passed");
        assert_eq!(report.checks.len(), 11);
        assert!(report.average_latency_ms.is_some());
        assert!(report.token_program_ready);
        assert!(report.token_2022_ready);
        assert!(report
            .checks
            .iter()
            .any(|check| check.category == CheckCategory::Token
                && check.detail.contains("Token Program ready")));
        assert!(report
            .checks
            .iter()
            .all(|check| check.status == CheckStatus::Success));
    }

    #[tokio::test]
    async fn run_check_returns_bad_when_critical_blockhash_check_fails() {
        let server = MockRpcServer::start(vec![
            health_ok(),
            version_ok(),
            genesis_ok(),
            slot_ok(),
            r#"{"jsonrpc":"2.0","id":5,"result":{"value":{"blockhash":"","lastValidBlockHeight":123456}}}"#,
            performance_ok(),
            finalized_slot_ok(),
            block_time_ok(),
            fees_ok(),
            token_program_ok(),
            token_2022_ok(),
        ]);

        let report = run_check(args_for(server.url.clone())).await.unwrap();
        server.join();

        assert_eq!(report.verdict, Verdict::Bad);
        assert!(report
            .checks
            .iter()
            .any(|check| check.method == "getLatestBlockhash"
                && check.error_kind == Some(ErrorKind::MalformedResponse)));
        assert!(report
            .checks
            .iter()
            .any(|check| check.method == "isBlockhashValid"
                && check.detail == "latest blockhash unavailable"));
    }

    #[tokio::test]
    async fn run_check_classifies_rpc_error_response() {
        let server = MockRpcServer::start(vec![
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"node unhealthy"}}"#,
            version_ok(),
            genesis_ok(),
            slot_ok(),
            latest_blockhash_ok(),
            blockhash_valid_ok(),
            performance_ok(),
            finalized_slot_ok(),
            block_time_ok(),
            fees_ok(),
            token_program_ok(),
            token_2022_ok(),
        ]);

        let report = run_check(args_for(server.url.clone())).await.unwrap();
        server.join();

        let health = report
            .checks
            .iter()
            .find(|check| check.method == "getHealth")
            .unwrap();
        assert_eq!(report.verdict, Verdict::Bad);
        assert_eq!(health.error_kind, Some(ErrorKind::RpcError));
        assert_eq!(health.detail, "RPC error -32005: node unhealthy");
    }

    #[tokio::test]
    async fn run_check_classifies_malformed_json_rpc_metadata() {
        let server = MockRpcServer::start(vec![
            r#"{"jsonrpc":"1.0","id":1,"result":"ok"}"#,
            version_ok(),
            genesis_ok(),
            slot_ok(),
            latest_blockhash_ok(),
            blockhash_valid_ok(),
            performance_ok(),
            finalized_slot_ok(),
            block_time_ok(),
            fees_ok(),
            token_program_ok(),
            token_2022_ok(),
        ]);

        let report = run_check(args_for(server.url.clone())).await.unwrap();
        server.join();

        let health = report
            .checks
            .iter()
            .find(|check| check.method == "getHealth")
            .unwrap();
        assert_eq!(report.verdict, Verdict::Bad);
        assert_eq!(health.error_kind, Some(ErrorKind::MalformedResponse));
        assert!(health.detail.contains("expected JSON-RPC 2.0"));
    }

    #[test]
    fn labels_categories_and_error_kinds() {
        assert_eq!(CheckCategory::Core.label(), "Core");
        assert_eq!(CheckCategory::Blockhash.label(), "Blockhash");
        assert_eq!(CheckCategory::Performance.label(), "Performance");

        assert_eq!(ErrorKind::InvalidUrl.label(), "invalid_url");
        assert_eq!(ErrorKind::Timeout.label(), "timeout");
        assert_eq!(ErrorKind::HttpError.label(), "http_error");
        assert_eq!(ErrorKind::RpcError.label(), "rpc_error");
        assert_eq!(ErrorKind::MalformedResponse.label(), "malformed_response");
        assert_eq!(ErrorKind::UnknownError.label(), "unknown_error");
    }

    #[test]
    fn summarizes_warning_with_fail_on_warning_policy() {
        let checks = vec![failed(
            CheckCategory::Performance,
            "getRecentPerformanceSamples",
            ErrorKind::RpcError,
        )];

        let summary = summarize(Verdict::Warning, &checks, Some(100), true);

        assert_eq!(
            summary,
            "RPC is reachable, but 1 non-critical check failed; --fail-on-warning is enabled, so CI should treat this as a failure"
        );
    }

    #[test]
    fn verdict_good_when_all_new_checks_pass_quickly() {
        let checks = vec![
            success(CheckCategory::Core, "getHealth", 100),
            success(CheckCategory::Core, "getVersion", 120),
            success(CheckCategory::Core, "getGenesisHash", 110),
            success(CheckCategory::Core, "getSlot", 90),
            success(CheckCategory::Blockhash, "getLatestBlockhash", 100),
            success(CheckCategory::Blockhash, "isBlockhashValid", 100),
            success(
                CheckCategory::Performance,
                "getRecentPerformanceSamples",
                100,
            ),
        ];
        assert_eq!(calculate_verdict(&checks, Some(103)), Verdict::Good);
    }

    #[test]
    fn verdict_warning_for_one_non_critical_failed_check() {
        let checks = vec![
            success(CheckCategory::Core, "getHealth", 100),
            success(CheckCategory::Blockhash, "getLatestBlockhash", 110),
            success(CheckCategory::Blockhash, "isBlockhashValid", 90),
            failed(
                CheckCategory::Performance,
                "getRecentPerformanceSamples",
                ErrorKind::RpcError,
            ),
        ];
        assert_eq!(calculate_verdict(&checks, Some(100)), Verdict::Warning);
    }

    #[test]
    fn verdict_bad_for_critical_blockhash_failure() {
        let checks = vec![
            success(CheckCategory::Core, "getHealth", 100),
            failed(
                CheckCategory::Blockhash,
                "isBlockhashValid",
                ErrorKind::RpcError,
            ),
        ];
        assert_eq!(calculate_verdict(&checks, Some(100)), Verdict::Bad);
    }

    #[test]
    fn verdict_bad_for_invalid_url() {
        let checks = vec![failed(
            CheckCategory::Core,
            "url_validation",
            ErrorKind::InvalidUrl,
        )];
        assert_eq!(calculate_verdict(&checks, None), Verdict::Bad);
    }

    #[test]
    fn verdict_warning_for_elevated_latency() {
        let checks = vec![success(CheckCategory::Core, "getHealth", 700)];
        assert_eq!(calculate_verdict(&checks, Some(700)), Verdict::Warning);
    }

    #[test]
    fn verdict_bad_for_repeated_timeouts() {
        let checks = vec![
            failed(CheckCategory::Performance, "a", ErrorKind::Timeout),
            failed(CheckCategory::Performance, "b", ErrorKind::Timeout),
        ];
        assert_eq!(calculate_verdict(&checks, None), Verdict::Bad);
    }

    #[test]
    fn verdict_bad_for_high_latency() {
        let checks = vec![success(CheckCategory::Core, "getHealth", 2_000)];
        assert_eq!(calculate_verdict(&checks, Some(2_000)), Verdict::Bad);
    }

    #[test]
    fn verdict_unknown_without_checks() {
        assert_eq!(calculate_verdict(&[], None), Verdict::Unknown);
    }
}
