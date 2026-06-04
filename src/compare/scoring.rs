//! Deterministic per-endpoint scoring and profile-specific notes.

use super::CompareEndpoint;
use crate::{cli::CompareProfile, verdict::Verdict};

/// Deterministically score an endpoint from `0` to `100` for a workload profile.
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

    // Block-time freshness of the finalized tip (a healthy chain finalizes
    // ~13-15s behind wall clock). A much larger lag means a stale endpoint.
    // `None` (method unavailable) is neutral, not penalized.
    score += match endpoint.block_time_lag_secs {
        Some(lag) if lag <= 20 => 10,
        Some(lag) if lag <= 45 => 5,
        Some(_) => 0,
        None => 0,
    };

    if endpoint.blockhash_valid {
        score += 15;
    }

    // Token-program readiness — whether the RPC serves the SPL Token and
    // Token-2022 program accounts, which token-touching workloads depend on. This
    // is a bonus weighted by how much the profile cares about tokens; absence is
    // neutral in the base so non-token workloads are not penalized for it.
    let token_weight = match profile {
        CompareProfile::General | CompareProfile::Ci => 0,
        CompareProfile::Wallet | CompareProfile::Bot => 3,
        CompareProfile::Indexer => 5,
    };
    if endpoint.token_program_ready {
        score += token_weight;
    }
    if endpoint.token_2022_ready {
        score += token_weight;
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
            // Indexers are freshness-sensitive: penalize a stale finalized tip.
            if endpoint.block_time_lag_secs.is_some_and(|lag| lag > 45) {
                score -= 10;
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

pub(crate) fn profile_notes(profile: CompareProfile, endpoint: &CompareEndpoint) -> Vec<String> {
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
            if !endpoint.token_program_ready {
                notes.push(
                    "SPL Token Program account is unavailable; wallet SPL token flows are at risk."
                        .to_string(),
                );
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
            if !endpoint.token_program_ready {
                notes.push(
                    "SPL Token Program account is unavailable for token-trading bot workloads."
                        .to_string(),
                );
            }
        }
        CompareProfile::Indexer => {
            if endpoint.slot_lag.is_some_and(|lag| lag > 50) {
                notes.push("Slot lag is high for indexer catch-up and freshness.".to_string());
            }
            if !endpoint.token_program_ready || !endpoint.token_2022_ready {
                notes.push(
                    "SPL Token or Token-2022 program account is unavailable; token account indexing may be incomplete."
                        .to_string(),
                );
            }
            if endpoint.block_time_lag_secs.is_some_and(|lag| lag > 45) {
                notes.push(
                    "Finalized block time is stale relative to wall clock for indexer freshness."
                        .to_string(),
                );
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
