//! Verdict calculation and human summary text for single-endpoint checks.

use super::{CheckCategory, CheckStatus, ErrorKind, RpcCheck};
use crate::verdict::Verdict;

const GOOD_AVERAGE_LATENCY_MS: u128 = 500;
const WARNING_AVERAGE_LATENCY_MS: u128 = 1_500;

/// Compute the overall [`Verdict`] from the individual checks and average latency.
pub fn calculate_verdict(checks: &[RpcCheck], average_latency_ms: Option<u128>) -> Verdict {
    if checks.is_empty() {
        return Verdict::Unknown;
    }

    let failed_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .count();
    let critical_failed = checks
        .iter()
        .any(|check| check.critical && check.status == CheckStatus::Failed);
    // Informational data-capability probes (`--data`) do not, on their own, make
    // an endpoint unusable, so their timeouts must not escalate the verdict to BAD
    // (a slow endpoint can time out both probes; on some platforms a refused
    // connection also surfaces as a timeout).
    let timeout_failures = checks
        .iter()
        .filter(|check| {
            check.category != CheckCategory::Data && check.error_kind == Some(ErrorKind::Timeout)
        })
        .count();
    let invalid_url = checks
        .iter()
        .any(|check| check.error_kind == Some(ErrorKind::InvalidUrl));

    if invalid_url
        || critical_failed
        || timeout_failures >= 2
        || average_latency_ms.is_some_and(|latency| latency > WARNING_AVERAGE_LATENCY_MS)
    {
        return Verdict::Bad;
    }

    // Non-critical (informational) failures — e.g. performance samples, block
    // time, prioritization fees — degrade the endpoint but do not make it
    // unusable, so any number of them caps the verdict at WARNING rather than BAD.
    if failed_count >= 1
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

/// Build the one-line, human-readable summary text for a verdict. Exposed so an
/// embedding frontend can produce the same summary the CLI shows.
pub fn summarize(
    verdict: Verdict,
    checks: &[RpcCheck],
    average_latency_ms: Option<u128>,
    fail_on_warning: bool,
) -> String {
    let failed_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Failed)
        .count();

    match verdict {
        Verdict::Good => "All RPC readiness checks passed".to_string(),
        Verdict::Warning => {
            let base = if failed_count > 0 {
                format!("RPC is reachable, but {failed_count} non-critical check failed")
            } else {
                let latency = average_latency_ms.unwrap_or_default();
                format!("RPC checks passed, but average latency is elevated at {latency} ms")
            };
            if fail_on_warning {
                format!(
                    "{base}; --fail-on-warning is enabled, so CI should treat this as a failure"
                )
            } else {
                base
            }
        }
        Verdict::Bad => {
            if failed_count > 0 {
                format!("{failed_count} RPC readiness checks failed")
            } else {
                let latency = average_latency_ms.unwrap_or_default();
                format!("Average latency is too high at {latency} ms")
            }
        }
        Verdict::Unknown => "Not enough data to produce a reliable verdict".to_string(),
    }
}
