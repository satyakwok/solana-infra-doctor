//! Multi-endpoint Yellowstone gRPC comparison: run the per-endpoint `grpc check`
//! probe for every supplied endpoint concurrently, score each for a workload
//! profile, rank them, and produce a [`GrpcCompareReport`].
//!
//! This reuses the single-endpoint [`run_grpc_check`](super::run_grpc_check)
//! engine unchanged — the same safe, slot-only, redaction-safe probe — and only
//! adds ranking on top. Rendering lives in [`render`] and scoring in [`scoring`].
//!
//! Like `grpc check`, the comparison is safe by default and never prints,
//! serializes, or logs any `x-token`. Tokens are read per endpoint from
//! environment variables named on the command line, paired by position with
//! `--grpc`.

use super::{run_grpc_check, AuthStatus, CheckStatus, GrpcReport};
use crate::{
    cli::{GrpcCheckArgs, GrpcCompareArgs, GrpcCompareProfile},
    error::AppError,
    verdict::Verdict,
};
use serde::Serialize;
use std::cmp::Ordering;

pub mod render;
pub mod scoring;
pub use render::*;
pub use scoring::*;

/// The JSON schema version for the gRPC comparison result. Bump on any breaking
/// change to the serialized [`GrpcCompareReport`] shape.
pub const GRPC_COMPARE_SCHEMA_VERSION: u32 = 1;

/// The full result of comparing multiple Yellowstone gRPC endpoints. This is the
/// serialized shape emitted by `--json`; it never contains a token.
#[derive(Debug, Clone, Serialize)]
pub struct GrpcCompareReport {
    /// Schema version for the comparison result shape.
    pub schema_version: u32,
    /// The workload profile used for scoring.
    pub profile: GrpcCompareProfileSummary,
    /// Per-endpoint results, in the order the endpoints were supplied.
    pub endpoints: Vec<GrpcCompareEndpoint>,
    /// Index (1-based) of the highest-scoring endpoint.
    pub best_endpoint_index: Option<usize>,
    /// Index (1-based) of the lowest-scoring endpoint.
    pub worst_endpoint_index: Option<usize>,
    /// Human-readable recommendation text.
    pub recommendation: String,
}

/// The workload profile a comparison was scored for. Mirrors
/// [`GrpcCompareProfile`](crate::cli::GrpcCompareProfile) in the serialized report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrpcCompareProfileSummary {
    General,
    Latency,
    Indexer,
}

impl GrpcCompareProfileSummary {
    /// The lowercase profile name (`general`, `latency`, `indexer`).
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Latency => "latency",
            Self::Indexer => "indexer",
        }
    }
}

impl From<GrpcCompareProfile> for GrpcCompareProfileSummary {
    fn from(value: GrpcCompareProfile) -> Self {
        match value {
            GrpcCompareProfile::General => Self::General,
            GrpcCompareProfile::Latency => Self::Latency,
            GrpcCompareProfile::Indexer => Self::Indexer,
        }
    }
}

/// One endpoint's result within a [`GrpcCompareReport`].
#[derive(Debug, Clone, Serialize)]
pub struct GrpcCompareEndpoint {
    /// 1-based position in the supplied endpoint list.
    pub index: usize,
    /// The redacted gRPC endpoint URL.
    pub endpoint: String,
    /// The endpoint's single-endpoint readiness verdict.
    pub verdict: Verdict,
    /// Workload score from `0` to `100`.
    pub score: u8,
    /// Whether an `x-token` was supplied for this endpoint (never the token).
    pub token_provided: bool,
    /// The authentication outcome.
    pub authentication: AuthStatus,
    /// Connection establishment latency in milliseconds.
    pub connect_latency_ms: Option<u128>,
    /// Time from subscribe to the first slot update, in milliseconds.
    pub first_event_latency_ms: Option<u128>,
    /// The latest slot observed on the gRPC stream.
    pub latest_slot: Option<u64>,
    /// Slots behind the freshest observed endpoint (`0` = freshest), when comparable.
    pub slot_lag: Option<u64>,
    /// Slot updates observed within the bounded stream window.
    pub updates_observed: u64,
    /// Whether the slot stream reached a passing state.
    pub stream_ok: bool,
    /// Count of unary probes that passed.
    pub unary_passed: usize,
    /// Count of unary probes that failed (skips are not failures).
    pub unary_failed: usize,
    /// Names of the unary methods that failed.
    pub failed_methods: Vec<String>,
    /// Profile-specific advisory notes for this endpoint.
    pub notes: Vec<String>,
}

/// Run the per-endpoint `grpc check` probe for every supplied endpoint and build
/// a ranked [`GrpcCompareReport`] for the requested workload profile.
///
/// Endpoints are probed **concurrently** (`try_join_all`), so the total time is
/// bounded by the slowest endpoint rather than the sum. A local configuration
/// error (e.g. a missing token environment variable) surfaces as the first error
/// and aborts the comparison before ranking.
pub async fn run_grpc_compare(args: GrpcCompareArgs) -> Result<GrpcCompareReport, AppError> {
    if args.grpc.len() < 2 {
        return Err(AppError::GrpcCompareRequiresTwoEndpoints);
    }

    let token_envs = pair_token_envs(args.grpc.len(), &args.x_token_env)?;
    let timeout_ms = args.timeout_ms;
    let duration_ms = args.duration_ms;

    let probes = args
        .grpc
        .into_iter()
        .zip(token_envs)
        .map(|(grpc, x_token_env)| {
            run_grpc_check(GrpcCheckArgs {
                grpc,
                x_token_env,
                rpc: None,
                json: false,
                report: None,
                timeout_ms,
                duration_ms,
            })
        });
    let reports = futures_util::future::try_join_all(probes).await?;

    Ok(build_grpc_compare_report(args.profile, &reports))
}

/// Pair `--x-token-env` values with `--grpc` endpoints by position. `0` tokens
/// means every endpoint is anonymous; `1` token is shared across all endpoints;
/// otherwise the count must equal the endpoint count.
fn pair_token_envs(endpoints: usize, tokens: &[String]) -> Result<Vec<Option<String>>, AppError> {
    match tokens.len() {
        0 => Ok(vec![None; endpoints]),
        1 => Ok(vec![Some(tokens[0].clone()); endpoints]),
        n if n == endpoints => Ok(tokens.iter().cloned().map(Some).collect()),
        n => Err(AppError::GrpcCompareTokenCountMismatch {
            endpoints,
            tokens: n,
        }),
    }
}

/// Build a [`GrpcCompareReport`] from already-computed per-endpoint
/// [`GrpcReport`]s: score and rank each endpoint and assemble the recommendation.
/// Separated from [`run_grpc_compare`] so it can be tested offline.
pub fn build_grpc_compare_report(
    profile: GrpcCompareProfile,
    reports: &[GrpcReport],
) -> GrpcCompareReport {
    // Slot freshness is ranked relative to the freshest endpoint observed in this
    // comparison. gRPC does not expose a genesis hash, so we cannot detect a
    // mixed-network comparison here; callers are expected to compare endpoints on
    // the same Solana network (documented in the README).
    let highest_slot = reports.iter().filter_map(|report| report.latest_slot).max();

    let mut endpoints: Vec<_> = reports
        .iter()
        .enumerate()
        .map(|(position, report)| build_endpoint(position + 1, profile, highest_slot, report))
        .collect();

    for endpoint in &mut endpoints {
        endpoint.score = score_endpoint(profile, endpoint);
        endpoint.notes = profile_notes(profile, endpoint);
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

    GrpcCompareReport {
        schema_version: GRPC_COMPARE_SCHEMA_VERSION,
        profile: profile.into(),
        endpoints,
        best_endpoint_index: Some(best_endpoint_index),
        worst_endpoint_index: Some(worst_endpoint_index),
        recommendation,
    }
}

fn build_endpoint(
    index: usize,
    profile: GrpcCompareProfile,
    highest_slot: Option<u64>,
    report: &GrpcReport,
) -> GrpcCompareEndpoint {
    let unary_passed = report
        .unary
        .iter()
        .filter(|u| u.status == CheckStatus::Pass)
        .count();
    let unary_failed = report
        .unary
        .iter()
        .filter(|u| u.status == CheckStatus::Fail)
        .count();
    let failed_methods = report
        .unary
        .iter()
        .filter(|u| u.status == CheckStatus::Fail)
        .map(|u| u.method.to_string())
        .collect();

    let mut endpoint = GrpcCompareEndpoint {
        index,
        endpoint: report.grpc_endpoint.clone(),
        verdict: report.verdict,
        score: 0,
        token_provided: report.token_provided,
        authentication: report.authentication,
        connect_latency_ms: report.connect_latency_ms,
        first_event_latency_ms: report.stream.first_event_latency_ms,
        latest_slot: report.latest_slot,
        slot_lag: slot_lag(report.latest_slot, highest_slot),
        updates_observed: report.stream.updates_observed,
        stream_ok: report.stream.status == CheckStatus::Pass,
        unary_passed,
        unary_failed,
        failed_methods,
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

/// Ranking order: higher score wins, then better verdict, then faster
/// first-event, then faster connect, then fresher slot.
fn compare_best(left: &&GrpcCompareEndpoint, right: &&GrpcCompareEndpoint) -> Ordering {
    left.score
        .cmp(&right.score)
        .then_with(|| verdict_rank(left.verdict).cmp(&verdict_rank(right.verdict)))
        .then_with(|| {
            reverse_option_u128(left.first_event_latency_ms, right.first_event_latency_ms)
        })
        .then_with(|| reverse_option_u128(left.connect_latency_ms, right.connect_latency_ms))
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

/// Order by the smaller value first (a lower latency / lag is better), so a
/// present-and-smaller value ranks higher than a present-and-larger one, and any
/// present value ranks higher than a missing one.
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
    profile: GrpcCompareProfile,
    endpoints: &[GrpcCompareEndpoint],
    best_endpoint_index: usize,
    worst_endpoint_index: usize,
) -> String {
    let mut lines = vec![
        format!("Best gRPC: #{best_endpoint_index}"),
        format!("Worst gRPC: #{worst_endpoint_index}"),
        format!(
            "gRPC #{best_endpoint_index} is recommended for {} workloads.",
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
        // When the lower-ranked endpoint actually connects faster but is slower to
        // first event or staler, "avoid it for latency" is misleading; describe
        // the real streaming tradeoff instead.
        if faster_connect_but_worse_stream(worst, best) {
            lines.push(format!(
                "gRPC #{} connects faster, but gRPC #{} reaches the first slot update sooner or streams fresher slots. For gRPC streaming, time-to-first-event and slot freshness usually matter more than raw connect latency.",
                worst.index, best.index
            ));
        } else {
            match profile {
                GrpcCompareProfile::General => {
                    lines.push(format!(
                        "Review gRPC #{} before relying on it for streaming workloads.",
                        worst.index
                    ));
                }
                GrpcCompareProfile::Latency => {
                    lines.push(format!(
                        "Avoid gRPC #{} for latency-sensitive streaming (slow connect or first-event).",
                        worst.index
                    ));
                }
                GrpcCompareProfile::Indexer => {
                    lines.push(format!(
                        "Avoid gRPC #{} for freshness-sensitive indexer streaming (stale slots or unstable stream).",
                        worst.index
                    ));
                }
            }
        }
    }

    lines.join("\n")
}

/// True when `worst` connects faster than `best` but is slower to first event or
/// further behind on slot freshness — the connect-versus-stream tradeoff.
fn faster_connect_but_worse_stream(
    worst: &GrpcCompareEndpoint,
    best: &GrpcCompareEndpoint,
) -> bool {
    let faster_connect = matches!(
        (worst.connect_latency_ms, best.connect_latency_ms),
        (Some(worst_connect), Some(best_connect)) if worst_connect < best_connect
    );
    let slower_first_event = matches!(
        (worst.first_event_latency_ms, best.first_event_latency_ms),
        (Some(worst_first), Some(best_first)) if worst_first > best_first
    );
    let staler = matches!(
        (worst.slot_lag, best.slot_lag),
        (Some(worst_lag), Some(best_lag)) if worst_lag > best_lag
    );
    faster_connect && (slower_first_event || staler)
}
