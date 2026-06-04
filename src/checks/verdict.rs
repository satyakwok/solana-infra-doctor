//! Verdict calculation and human summary text for single-endpoint checks.

use super::{CheckStatus, ErrorKind, RpcCheck};
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
    let timeout_failures = checks
        .iter()
        .filter(|check| check.error_kind == Some(ErrorKind::Timeout))
        .count();
    let invalid_url = checks
        .iter()
        .any(|check| check.error_kind == Some(ErrorKind::InvalidUrl));

    if invalid_url
        || critical_failed
        || failed_count >= 2
        || timeout_failures >= 2
        || average_latency_ms.is_some_and(|latency| latency > WARNING_AVERAGE_LATENCY_MS)
    {
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

pub(crate) fn summarize(
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

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;
    use crate::checks::CheckCategory;

    fn check(status: CheckStatus, critical: bool, error_kind: Option<ErrorKind>) -> RpcCheck {
        RpcCheck {
            category: CheckCategory::Core,
            method: "m",
            status,
            latency_ms: Some(10),
            detail: String::new(),
            error_kind,
            critical,
        }
    }

    #[test]
    fn summarize_covers_every_verdict_branch() {
        let pass = vec![check(CheckStatus::Success, true, None)];
        assert_eq!(
            summarize(Verdict::Good, &pass, Some(50), false),
            "All RPC readiness checks passed"
        );
        assert_eq!(
            summarize(Verdict::Unknown, &[], None, false),
            "Not enough data to produce a reliable verdict"
        );

        let one_fail = vec![check(CheckStatus::Failed, false, Some(ErrorKind::RpcError))];
        assert!(summarize(Verdict::Warning, &one_fail, Some(50), false)
            .contains("non-critical check failed"));

        let elevated = summarize(Verdict::Warning, &pass, Some(800), true);
        assert!(elevated.contains("elevated at 800 ms"));
        assert!(elevated.contains("--fail-on-warning is enabled"));

        let two_fail = vec![
            check(CheckStatus::Failed, true, None),
            check(CheckStatus::Failed, false, None),
        ];
        assert_eq!(
            summarize(Verdict::Bad, &two_fail, Some(50), false),
            "2 RPC readiness checks failed"
        );
        assert_eq!(
            summarize(Verdict::Bad, &pass, Some(2_000), false),
            "Average latency is too high at 2000 ms"
        );
    }

    #[test]
    fn calculate_verdict_covers_thresholds() {
        let pass = vec![check(CheckStatus::Success, true, None)];
        assert_eq!(calculate_verdict(&[], None), Verdict::Unknown);
        assert_eq!(calculate_verdict(&pass, Some(50)), Verdict::Good);
        assert_eq!(calculate_verdict(&pass, None), Verdict::Unknown);
        assert_eq!(calculate_verdict(&pass, Some(800)), Verdict::Warning);
        assert_eq!(calculate_verdict(&pass, Some(2_000)), Verdict::Bad);

        let critical = vec![check(CheckStatus::Failed, true, None)];
        assert_eq!(calculate_verdict(&critical, Some(50)), Verdict::Bad);

        let invalid = vec![check(
            CheckStatus::Failed,
            false,
            Some(ErrorKind::InvalidUrl),
        )];
        assert_eq!(calculate_verdict(&invalid, Some(50)), Verdict::Bad);

        let one_non_critical = vec![
            check(CheckStatus::Success, true, None),
            check(CheckStatus::Failed, false, Some(ErrorKind::RpcError)),
        ];
        assert_eq!(
            calculate_verdict(&one_non_critical, Some(50)),
            Verdict::Warning
        );

        let timeouts = vec![
            check(CheckStatus::Failed, false, Some(ErrorKind::Timeout)),
            check(CheckStatus::Failed, false, Some(ErrorKind::Timeout)),
        ];
        assert_eq!(calculate_verdict(&timeouts, Some(50)), Verdict::Bad);
    }
}
