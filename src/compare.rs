use crate::{
    checks::{run_check, CheckReport, CheckStatus},
    cli::{CheckArgs, CompareArgs, CompareProfile},
    error::AppError,
    verdict::Verdict,
};
use serde::Serialize;
use std::{cmp::Ordering, collections::BTreeSet, fs};

const MISMATCH_REASON: &str =
    "Endpoints returned different genesis hashes, indicating different Solana networks.";

#[derive(Debug, Clone, Serialize)]
pub struct CompareReport {
    pub profile: CompareProfileSummary,
    pub endpoints: Vec<CompareEndpoint>,
    pub best_endpoint_index: Option<usize>,
    pub worst_endpoint_index: Option<usize>,
    pub network_mismatch: bool,
    pub mismatch_reason: Option<String>,
    pub recommendation: String,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct CompareEndpoint {
    pub index: usize,
    pub url: String,
    pub genesis_hash: Option<String>,
    pub verdict: Verdict,
    pub score: u8,
    pub slot: Option<u64>,
    pub slot_lag: Option<u64>,
    pub average_latency_ms: Option<u128>,
    pub failed_checks: Vec<String>,
    pub blockhash_valid: bool,
    pub notes: Vec<String>,
}

pub async fn run_compare(args: CompareArgs) -> Result<CompareReport, AppError> {
    if args.rpc.len() < 2 {
        return Err(AppError::CompareRequiresTwoRpcUrls);
    }

    let mut check_reports = Vec::with_capacity(args.rpc.len());
    for rpc in args.rpc {
        check_reports.push(
            run_check(CheckArgs {
                rpc,
                json: false,
                fail_on_warning: false,
                timeout_ms: args.timeout_ms,
            })
            .await?,
        );
    }

    Ok(build_compare_report(args.profile, &check_reports))
}

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
        failed_checks,
        blockhash_valid,
        notes: Vec::new(),
    };
    endpoint.score = score_endpoint(profile, &endpoint);
    endpoint.notes = profile_notes(profile, &endpoint);
    endpoint
}

pub fn slot_lag(slot: Option<u64>, highest_slot: Option<u64>) -> Option<u64> {
    match (slot, highest_slot) {
        (Some(slot), Some(highest)) => Some(highest.saturating_sub(slot)),
        _ => None,
    }
}

pub fn score_endpoint(profile: CompareProfile, endpoint: &CompareEndpoint) -> u8 {
    let mut score = match endpoint.verdict {
        Verdict::Good => 40i32,
        Verdict::Warning => 20,
        Verdict::Unknown => 5,
        Verdict::Bad => 0,
    };

    score += match endpoint.average_latency_ms {
        Some(latency) if latency <= 300 => 20,
        Some(latency) if latency <= 700 => 12,
        Some(latency) if latency <= 1_500 => 5,
        Some(_) | None => 0,
    };

    score += match endpoint.slot_lag {
        Some(lag) if lag <= 5 => 15,
        Some(lag) if lag <= 50 => 8,
        Some(_) | None => 0,
    };

    if endpoint.blockhash_valid {
        score += 15;
    }

    score -= 5 * i32::try_from(endpoint.failed_checks.len()).unwrap_or(i32::MAX);

    match profile {
        CompareProfile::General => {}
        CompareProfile::Wallet => {
            if endpoint.blockhash_valid {
                score += 5;
            }
            if has_failed_core_check(endpoint) {
                score -= 10;
            }
        }
        CompareProfile::Bot => {
            if endpoint
                .average_latency_ms
                .is_some_and(|latency| latency > 700)
            {
                score -= 10;
            }
            if endpoint.slot_lag.is_some_and(|lag| lag > 50) {
                score -= 10;
            }
        }
        CompareProfile::Indexer => {
            if endpoint.slot_lag.is_some_and(|lag| lag > 50) {
                score -= 15;
            }
            if endpoint
                .failed_checks
                .iter()
                .any(|method| method == "getRecentPerformanceSamples")
            {
                score -= 10;
            }
        }
        CompareProfile::Ci => {
            if !matches!(endpoint.verdict, Verdict::Good) {
                score -= 15;
            }
        }
    }

    score.clamp(0, 100) as u8
}

fn has_failed_core_check(endpoint: &CompareEndpoint) -> bool {
    endpoint.failed_checks.iter().any(|method| {
        matches!(
            method.as_str(),
            "getHealth" | "getVersion" | "getGenesisHash" | "getSlot"
        )
    })
}

fn profile_notes(profile: CompareProfile, endpoint: &CompareEndpoint) -> Vec<String> {
    let mut notes = Vec::new();

    match profile {
        CompareProfile::General => {}
        CompareProfile::Wallet => {
            if !endpoint.blockhash_valid {
                notes.push(
                    "Latest blockhash is invalid or missing; wallet transaction preparation is at risk."
                        .to_string(),
                );
            }
            if has_failed_core_check(endpoint) {
                notes.push("One or more core RPC methods failed for wallet workloads.".to_string());
            }
        }
        CompareProfile::Bot => {
            if endpoint
                .average_latency_ms
                .is_some_and(|latency| latency > 700)
            {
                notes.push(
                    "Average latency is high for latency-sensitive bot workloads.".to_string(),
                );
            }
            if endpoint.slot_lag.is_some_and(|lag| lag > 50) {
                notes.push("Slot lag is high for slot-sensitive bot workloads.".to_string());
            }
        }
        CompareProfile::Indexer => {
            if endpoint.slot_lag.is_some_and(|lag| lag > 50) {
                notes.push("Slot lag is high for indexer catch-up and freshness.".to_string());
            }
            if endpoint
                .failed_checks
                .iter()
                .any(|method| method == "getRecentPerformanceSamples")
            {
                notes.push(
                    "Recent performance samples are unavailable for indexer diagnostics."
                        .to_string(),
                );
            }
        }
        CompareProfile::Ci => {
            if matches!(endpoint.verdict, Verdict::Warning | Verdict::Bad) {
                notes.push(
                    "This endpoint is not recommended for CI pass gates because its verdict is not GOOD."
                        .to_string(),
                );
            }
        }
    }

    notes
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

pub fn render_human(report: &CompareReport) -> String {
    let mut output = String::new();
    output.push_str("Solana Infra Doctor — RPC Compare\n\n");
    output.push_str(&format!("Profile: {}\n\n", report.profile.label()));

    if report.network_mismatch {
        output.push_str("Cannot compare endpoints from different Solana networks.\n");
        output.push_str(
            "Endpoints returned different genesis hashes; ranking and slot lag are disabled.\n\n",
        );
    }

    for endpoint in &report.endpoints {
        output.push_str(&format!("RPC #{}\n", endpoint.index));
        output.push_str(&format!("URL: {}\n", endpoint.url));
        output.push_str(&format!(
            "Genesis: {}\n",
            format_genesis(&endpoint.genesis_hash)
        ));
        output.push_str(&format!("Verdict: {}\n", endpoint.verdict));
        output.push_str(&format!("Score: {}/100\n", endpoint.score));
        output.push_str(&format!("Slot: {}\n", format_slot(endpoint.slot)));
        output.push_str(&format!(
            "Slot lag: {}\n",
            format_slot_lag(endpoint.slot_lag)
        ));
        output.push_str(&format!(
            "Average latency: {}\n",
            format_latency(endpoint.average_latency_ms)
        ));
        output.push_str(&format!(
            "Failed checks: {}\n",
            format_failed_checks(&endpoint.failed_checks)
        ));
        output.push_str(&format!(
            "Blockhash valid: {}\n",
            if endpoint.blockhash_valid {
                "yes"
            } else {
                "no"
            }
        ));
        if !endpoint.notes.is_empty() {
            output.push_str("Notes:\n");
            for note in &endpoint.notes {
                output.push_str(&format!("- {note}\n"));
            }
        }
        output.push('\n');
    }

    output.push_str("Recommendation:\n");
    output.push_str(&report.recommendation);
    output.push('\n');
    output
}

pub fn render_json(report: &CompareReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

pub fn render_markdown(report: &CompareReport) -> String {
    let mut output = String::new();
    output.push_str("# Solana Infra Doctor RPC Compare Report\n\n");
    output.push_str(&format!("Profile: `{}`\n\n", report.profile.label()));

    if report.network_mismatch {
        output.push_str("## Network Mismatch\n\n");
        output.push_str(
            "Cannot compare endpoints from different Solana networks. Endpoints returned different genesis hashes, so ranking and slot lag are disabled.\n\n",
        );
    }

    output.push_str("## Summary\n\n");
    output.push_str(&format!(
        "- Best RPC: {}\n- Worst RPC: {}\n\n",
        format_rank(report.best_endpoint_index),
        format_rank(report.worst_endpoint_index)
    ));

    output.push_str("## Comparison\n\n");
    output.push_str("| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |\n");
    output.push_str("| --- | --- | --- | ---: | --- | --- | --- | --- | --- |\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!(
            "| RPC #{} | `{}` | `{}` | {}/100 | {} | {} | {} | {} | {} |\n",
            endpoint.index,
            endpoint.url,
            endpoint.verdict,
            endpoint.score,
            format_slot(endpoint.slot),
            format_slot_lag(endpoint.slot_lag),
            format_latency(endpoint.average_latency_ms),
            format_failed_checks(&endpoint.failed_checks),
            if endpoint.blockhash_valid {
                "yes"
            } else {
                "no"
            }
        ));
    }

    output.push_str("\n## Per-Endpoint Details\n\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!("### RPC #{}\n\n", endpoint.index));
        output.push_str(&format!("- URL: `{}`\n", endpoint.url));
        output.push_str(&format!(
            "- Genesis: `{}`\n",
            format_genesis(&endpoint.genesis_hash)
        ));
        output.push_str(&format!("- Verdict: `{}`\n", endpoint.verdict));
        output.push_str(&format!("- Score: {}/100\n", endpoint.score));
        output.push_str(&format!("- Slot: {}\n", format_slot(endpoint.slot)));
        output.push_str(&format!(
            "- Slot lag: {}\n",
            format_slot_lag(endpoint.slot_lag)
        ));
        output.push_str(&format!(
            "- Average latency: {}\n",
            format_latency(endpoint.average_latency_ms)
        ));
        output.push_str(&format!(
            "- Failed checks: {}\n",
            format_failed_checks(&endpoint.failed_checks)
        ));
        if endpoint.notes.is_empty() {
            output.push_str("- Notes: none\n\n");
        } else {
            output.push_str("- Notes:\n");
            for note in &endpoint.notes {
                output.push_str(&format!("  - {note}\n"));
            }
            output.push('\n');
        }
    }

    output.push_str("## Recommendation\n\n");
    output.push_str(&report.recommendation.replace('\n', "\n\n"));
    output.push_str("\n\n## Limitations\n\n");
    output.push_str(
        "- HTTP JSON-RPC diagnostics only; WebSocket diagnostics are not included yet.\n",
    );
    output.push_str("- Checks run sequentially for deterministic v0.1 behavior.\n");
    output.push_str("- Scores are deterministic heuristics, not a provider guarantee.\n\n");
    output.push_str("## Disclaimer\n\n");
    output.push_str(
        "Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.\n",
    );
    output
}

pub fn write_markdown_report(
    report: &CompareReport,
    path: &std::path::Path,
) -> Result<(), AppError> {
    fs::write(path, render_markdown(report)).map_err(|source| AppError::WriteMarkdownReport {
        path: path.display().to_string(),
        source,
    })
}

fn format_genesis(genesis_hash: &Option<String>) -> String {
    genesis_hash
        .as_deref()
        .map_or_else(|| "n/a".to_string(), str::to_string)
}

fn format_rank(index: Option<usize>) -> String {
    index.map_or_else(
        || "n/a (different networks)".to_string(),
        |index| format!("RPC #{index}"),
    )
}

fn format_slot(slot: Option<u64>) -> String {
    slot.map_or_else(|| "n/a".to_string(), |slot| slot.to_string())
}

fn format_slot_lag(slot_lag: Option<u64>) -> String {
    match slot_lag {
        Some(0) => "baseline".to_string(),
        Some(lag) => format!("{lag} slots behind"),
        None => "n/a".to_string(),
    }
}

fn format_latency(latency: Option<u128>) -> String {
    latency.map_or_else(|| "n/a".to_string(), |latency| format!("{latency}ms"))
}

fn format_failed_checks(failed_checks: &[String]) -> String {
    if failed_checks.is_empty() {
        "none".to_string()
    } else {
        failed_checks.join(", ")
    }
}
