//! Multi-endpoint comparison: run the per-endpoint checks, score each endpoint
//! for a workload profile, rank them, and produce a [`CompareReport`]. Rendering
//! lives in [`render`] and scoring in [`scoring`].

use crate::{
    checks::{CheckReport, CheckStatus, ProgramAccountsReadiness, run_check},
    cli::{CheckArgs, CompareArgs, CompareProfile},
    error::AppError,
    verdict::Verdict,
};
use serde::Serialize;
use std::{cmp::Ordering, collections::BTreeSet};

pub mod render;
pub mod scoring;
pub use render::*;
pub use scoring::*;

const MISMATCH_REASON: &str =
    "Endpoints returned different genesis hashes, indicating different Solana networks.";

/// Schema version for the `compare --json` result shape. Bump on any
/// backward-incompatible change to the serialized fields.
pub const COMPARE_SCHEMA_VERSION: u32 = 1;

/// The full result of comparing multiple RPC endpoints. This is the serialized
/// shape emitted by `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct CompareReport {
    /// Schema version for the result shape (see [`COMPARE_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// The workload profile used for scoring.
    pub profile: CompareProfileSummary,
    /// Per-endpoint results, in the order the endpoints were supplied.
    pub endpoints: Vec<CompareEndpoint>,
    /// Index (1-based) of the highest-scoring endpoint, if ranking is possible.
    pub best_endpoint_index: Option<usize>,
    /// Index (1-based) of the lowest-scoring endpoint, if ranking is possible.
    pub worst_endpoint_index: Option<usize>,
    /// Whether the endpoints are on different Solana networks (ranking disabled).
    pub network_mismatch: bool,
    /// Why a comparison was rejected, when `network_mismatch` is set.
    pub mismatch_reason: Option<String>,
    /// Human-readable recommendation text.
    pub recommendation: String,
}

/// The workload profile a comparison was scored for. Mirrors
/// [`CompareProfile`](crate::cli::CompareProfile) in the serialized report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareProfileSummary {
    General,
    Wallet,
    Bot,
    Indexer,
    Ci,
}

impl CompareProfileSummary {
    /// The lowercase profile name (`general`, `wallet`, `bot`, `indexer`, `ci`).
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Wallet => "wallet",
            Self::Bot => "bot",
            Self::Indexer => "indexer",
            Self::Ci => "ci",
        }
    }
}

impl From<CompareProfile> for CompareProfileSummary {
    fn from(value: CompareProfile) -> Self {
        match value {
            CompareProfile::General => Self::General,
            CompareProfile::Wallet => Self::Wallet,
            CompareProfile::Bot => Self::Bot,
            CompareProfile::Indexer => Self::Indexer,
            CompareProfile::Ci => Self::Ci,
        }
    }
}

/// One endpoint's result within a [`CompareReport`].
#[derive(Debug, Clone, Serialize)]
pub struct CompareEndpoint {
    /// 1-based position in the supplied endpoint list.
    pub index: usize,
    /// The redacted endpoint URL.
    pub url: String,
    /// The endpoint's genesis hash, used for network-mismatch detection.
    pub genesis_hash: Option<String>,
    /// The endpoint's single-endpoint readiness verdict.
    pub verdict: Verdict,
    /// Workload score from `0` to `100`.
    pub score: u8,
    /// The endpoint's observed slot, if available.
    pub slot: Option<u64>,
    /// Slots behind the freshest endpoint (`0` = freshest), when comparable.
    pub slot_lag: Option<u64>,
    /// Mean per-check latency in milliseconds.
    pub average_latency_ms: Option<u128>,
    /// Seconds the finalized block time lags wall clock — a freshness signal
    /// (lower is fresher). Used in scoring; `None` when unavailable.
    pub block_time_lag_secs: Option<i64>,
    /// Median recent prioritization fee (micro-lamports/CU). Chain-wide
    /// fee-market context — surfaced, but not a scoring discriminator.
    pub prioritization_fee_median: Option<u64>,
    /// Whether the RPC serves the SPL Token Program as an executable program.
    pub token_program_ready: bool,
    /// Whether the RPC serves the Token-2022 program as an executable program.
    pub token_2022_ready: bool,
    /// `getProgramAccounts` enablement, when `--data` ran (`None` otherwise).
    /// The `indexer` profile scores this; other profiles ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_accounts: Option<ProgramAccountsReadiness>,
    /// Oldest slot the endpoint can serve (`getFirstAvailableBlock`), when `--data`
    /// ran. `0` means full archival from genesis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_available_slot: Option<u64>,
    /// Archival depth in slots behind the freshest slot, when computable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archival_depth_slots: Option<u64>,
    /// Names of the checks that failed.
    pub failed_checks: Vec<String>,
    /// Whether the latest blockhash validated.
    pub blockhash_valid: bool,
    /// Profile-specific advisory notes for this endpoint.
    pub notes: Vec<String>,
}

/// Run the per-endpoint checks for every supplied URL and build a ranked
/// [`CompareReport`] for the requested workload profile.
///
/// Endpoints are checked **concurrently**: each endpoint's diagnostics run in
/// parallel, so the total time is bounded by the slowest endpoint rather than
/// the sum. `try_join_all` preserves the input order and surfaces the first
/// error, matching the previous sequential behavior.
pub async fn run_compare(args: CompareArgs) -> Result<CompareReport, AppError> {
    if args.rpc.len() < 2 {
        return Err(AppError::CompareRequiresTwoRpcUrls);
    }

    let timeout_ms = args.timeout_ms;
    let data = args.data;
    let data_program = args.data_program;
    let checks = args.rpc.into_iter().map(|rpc| {
        run_check(CheckArgs {
            rpc,
            json: false,
            fail_on_warning: false,
            samples: 1,
            // Data-readiness probes run only when `--data` is set; the `indexer`
            // profile scores them when present.
            data,
            data_program: data_program.clone(),
            timeout_ms,
        })
    });
    let check_reports = futures_util::future::try_join_all(checks).await?;

    Ok(build_compare_report(args.profile, &check_reports))
}

/// Build a [`CompareReport`] from already-computed per-endpoint [`CheckReport`]s:
/// score and rank each endpoint, detect a network mismatch, and assemble the
/// recommendation. Separated from [`run_compare`] so it can be tested offline.
pub fn build_compare_report(profile: CompareProfile, reports: &[CheckReport]) -> CompareReport {
    let network_mismatch = has_genesis_mismatch(reports);
    // Slot lag across different networks is meaningless, so suppress the shared
    // baseline when genesis hashes disagree.
    let highest_slot = if network_mismatch {
        None
    } else {
        reports.iter().filter_map(extract_slot).max()
    };

    let mut endpoints: Vec<_> = reports
        .iter()
        .enumerate()
        .map(|(position, report)| build_endpoint(position + 1, profile, highest_slot, report))
        .collect();

    for endpoint in &mut endpoints {
        endpoint.score = score_endpoint(profile, endpoint);
        endpoint.notes = profile_notes(profile, endpoint);
    }

    if network_mismatch {
        return CompareReport {
            schema_version: COMPARE_SCHEMA_VERSION,
            profile: profile.into(),
            endpoints,
            best_endpoint_index: None,
            worst_endpoint_index: None,
            network_mismatch: true,
            mismatch_reason: Some(MISMATCH_REASON.to_string()),
            recommendation: mismatch_recommendation(),
        };
    }

    let best_endpoint_index = endpoints
        .iter()
        .max_by(compare_best)
        .map(|endpoint| endpoint.index)
        .unwrap_or_default();
    let worst_endpoint_index = endpoints
        .iter()
        .min_by(compare_best)
        .map(|endpoint| endpoint.index)
        .unwrap_or_default();
    let recommendation = build_recommendation(
        profile,
        &endpoints,
        best_endpoint_index,
        worst_endpoint_index,
    );

    CompareReport {
        schema_version: COMPARE_SCHEMA_VERSION,
        profile: profile.into(),
        endpoints,
        best_endpoint_index: Some(best_endpoint_index),
        worst_endpoint_index: Some(worst_endpoint_index),
        network_mismatch: false,
        mismatch_reason: None,
        recommendation,
    }
}

fn has_genesis_mismatch(reports: &[CheckReport]) -> bool {
    let distinct: BTreeSet<&str> = reports.iter().filter_map(genesis_hash).collect();
    distinct.len() >= 2
}

fn genesis_hash(report: &CheckReport) -> Option<&str> {
    report
        .checks
        .iter()
        .find(|check| check.method == "getGenesisHash" && check.status == CheckStatus::Success)
        .map(|check| check.detail.as_str())
}

fn mismatch_recommendation() -> String {
    "Cannot compare endpoints from different Solana networks.\n\
     Slot lag and ranking are disabled because the endpoints report different genesis hashes.\n\
     Re-run compare with endpoints on the same network."
        .to_string()
}

fn build_endpoint(
    index: usize,
    profile: CompareProfile,
    highest_slot: Option<u64>,
    report: &CheckReport,
) -> CompareEndpoint {
    let slot = extract_slot(report);
    let slot_lag = slot_lag(slot, highest_slot);
    let failed_checks = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .map(|check| check.method.to_string())
        .collect();
    let blockhash_valid = report
        .checks
        .iter()
        .any(|check| check.method == "isBlockhashValid" && check.status == CheckStatus::Success);

    let mut endpoint = CompareEndpoint {
        index,
        url: report.rpc_url.clone(),
        genesis_hash: genesis_hash(report).map(str::to_string),
        verdict: report.verdict,
        score: 0,
        slot,
        slot_lag,
        average_latency_ms: report.average_latency_ms,
        block_time_lag_secs: report.block_time_lag_secs,
        prioritization_fee_median: report.prioritization_fee_median,
        token_program_ready: report.token_program_ready,
        token_2022_ready: report.token_2022_ready,
        program_accounts: report.program_accounts,
        oldest_available_slot: report.oldest_available_slot,
        archival_depth_slots: report.archival_depth_slots,
        failed_checks,
        blockhash_valid,
        notes: Vec::new(),
    };
    endpoint.score = score_endpoint(profile, &endpoint);
    endpoint.notes = profile_notes(profile, &endpoint);
    endpoint
}

/// Slots an endpoint is behind the freshest observed slot (`0` = freshest).
pub fn slot_lag(slot: Option<u64>, highest_slot: Option<u64>) -> Option<u64> {
    match (slot, highest_slot) {
        (Some(slot), Some(highest)) => Some(highest.saturating_sub(slot)),
        _ => None,
    }
}

fn extract_slot(report: &CheckReport) -> Option<u64> {
    report
        .checks
        .iter()
        .find(|check| check.method == "getSlot" && check.status == CheckStatus::Success)
        .and_then(|check| check.detail.strip_prefix("slot "))
        .and_then(|slot| slot.parse().ok())
}

fn compare_best(left: &&CompareEndpoint, right: &&CompareEndpoint) -> Ordering {
    left.score
        .cmp(&right.score)
        .then_with(|| verdict_rank(left.verdict).cmp(&verdict_rank(right.verdict)))
        .then_with(|| reverse_option_u128(left.average_latency_ms, right.average_latency_ms))
        .then_with(|| reverse_option_u64(left.slot_lag, right.slot_lag))
}

fn verdict_rank(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::Good => 4,
        Verdict::Warning => 3,
        Verdict::Unknown => 2,
        Verdict::Bad => 1,
    }
}

fn reverse_option_u128(left: Option<u128>, right: Option<u128>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn reverse_option_u64(left: Option<u64>, right: Option<u64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn build_recommendation(
    profile: CompareProfile,
    endpoints: &[CompareEndpoint],
    best_endpoint_index: usize,
    worst_endpoint_index: usize,
) -> String {
    let mut lines = vec![
        format!("Best RPC: RPC #{best_endpoint_index}"),
        format!("Worst RPC: RPC #{worst_endpoint_index}"),
        format!(
            "RPC #{best_endpoint_index} is recommended for {} workloads.",
            profile.label()
        ),
    ];

    let best = endpoints
        .iter()
        .find(|endpoint| endpoint.index == best_endpoint_index);
    let worst = endpoints
        .iter()
        .find(|endpoint| endpoint.index == worst_endpoint_index);

    if let (Some(best), Some(worst)) = (best, worst) {
        // When the lower-ranked endpoint is actually faster but staler, calling
        // it "latency-sensitive risk" is misleading; describe the real tradeoff.
        if !matches!(profile, CompareProfile::Ci) && lower_latency_but_staler(worst, best) {
            lines.push(format!(
                "RPC #{} has lower latency, but RPC #{} is fresher. For {} workloads, slot freshness may matter more than raw HTTP latency.",
                worst.index,
                best.index,
                profile.label()
            ));
        } else {
            match profile {
                CompareProfile::General => {
                    lines.push(format!(
                        "Review RPC #{} before using it for production traffic.",
                        worst.index
                    ));
                }
                CompareProfile::Wallet => {
                    lines.push(format!(
                        "Avoid RPC #{} for wallet transaction flows if blockhash or core checks failed.",
                        worst.index
                    ));
                }
                CompareProfile::Bot => {
                    lines.push(format!(
                        "Avoid RPC #{} for latency-sensitive or slot-sensitive workloads.",
                        worst.index
                    ));
                }
                CompareProfile::Indexer => {
                    lines.push(format!(
                        "Avoid RPC #{} for freshness-sensitive indexer workloads.",
                        worst.index
                    ));
                }
                CompareProfile::Ci => {
                    lines.push(
                        "CI profile is strict: WARNING or BAD endpoints are not recommended for pass gates."
                            .to_string(),
                    );
                }
            }
        }
    }

    lines.join("\n")
}

/// True when `worst` has lower average latency than `best` but is further behind
/// on slot freshness — the latency-versus-freshness tradeoff.
fn lower_latency_but_staler(worst: &CompareEndpoint, best: &CompareEndpoint) -> bool {
    let faster = matches!(
        (worst.average_latency_ms, best.average_latency_ms),
        (Some(worst_latency), Some(best_latency)) if worst_latency < best_latency
    );
    let staler = matches!(
        (worst.slot_lag, best.slot_lag),
        (Some(worst_lag), Some(best_lag)) if worst_lag > best_lag
    );
    faster && staler
}
