//! Deterministic per-endpoint scoring and profile-specific notes for gRPC
//! comparison. The score weighs the signals that matter for a Yellowstone
//! streaming consumer: verdict, connect latency, time-to-first-event, slot
//! freshness, and stream stability.

use super::GrpcCompareEndpoint;
use crate::{cli::GrpcCompareProfile, verdict::Verdict};

/// Deterministically score a gRPC endpoint from `0` to `100` for a workload
/// profile. A perfect general endpoint (GOOD, fast connect, fast first event,
/// fresh slot, stable stream) reaches `100`; failures and profile-specific
/// penalties subtract from there.
pub fn score_endpoint(profile: GrpcCompareProfile, endpoint: &GrpcCompareEndpoint) -> u8 {
    let mut score = match endpoint.verdict {
        Verdict::Good => 40i32,
        Verdict::Warning => 20,
        Verdict::Unknown => 5,
        Verdict::Bad => 0,
    };

    // Connection establishment latency (lower is better).
    score += match endpoint.connect_latency_ms {
        Some(ms) if ms <= 100 => 15,
        Some(ms) if ms <= 300 => 10,
        Some(ms) if ms <= 800 => 5,
        Some(_) | None => 0,
    };

    // Time-to-first-slot-update — the key streaming-startup signal.
    score += match endpoint.first_event_latency_ms {
        Some(ms) if ms <= 500 => 20,
        Some(ms) if ms <= 1_500 => 12,
        Some(ms) if ms <= 3_000 => 5,
        Some(_) | None => 0,
    };

    // Slot freshness relative to the freshest endpoint in this comparison.
    score += match endpoint.slot_lag {
        Some(lag) if lag <= 5 => 15,
        Some(lag) if lag <= 50 => 8,
        Some(_) | None => 0,
    };

    // Stream stability: reaching a passing slot stream is worth the remaining
    // headroom toward 100.
    if endpoint.stream_ok {
        score += 10;
    }

    // Each failed unary method is a real capability gap (skips are not failures).
    score -= 3 * i32::try_from(endpoint.failed_methods.len()).unwrap_or(i32::MAX);

    match profile {
        GrpcCompareProfile::General => {}
        GrpcCompareProfile::Latency => {
            if endpoint.connect_latency_ms.is_some_and(|ms| ms > 300) {
                score -= 8;
            }
            // A slow or absent first event is disqualifying for latency-sensitive
            // streaming consumers (bots/MEV).
            if endpoint.first_event_latency_ms.is_none_or(|ms| ms > 1_500) {
                score -= 12;
            }
        }
        GrpcCompareProfile::Indexer => {
            if endpoint.slot_lag.is_none_or(|lag| lag > 50) {
                score -= 12;
            }
            if !endpoint.stream_ok {
                score -= 10;
            }
            // A stream that opened but delivered almost nothing is weak for an
            // indexer that needs a steady slot feed.
            if endpoint.updates_observed < 2 {
                score -= 5;
            }
        }
    }

    score.clamp(0, 100) as u8
}

pub(crate) fn profile_notes(
    profile: GrpcCompareProfile,
    endpoint: &GrpcCompareEndpoint,
) -> Vec<String> {
    let mut notes = Vec::new();

    // A shared, profile-independent note: a missing token against an endpoint
    // that rejected authentication is the most common, most actionable issue.
    if !endpoint.token_provided
        && matches!(
            endpoint.authentication,
            crate::grpc::AuthStatus::Unauthenticated | crate::grpc::AuthStatus::PermissionDenied
        )
    {
        notes.push(
            "Endpoint rejected the request without a token; supply an x-token via --x-token-env."
                .to_string(),
        );
    }

    match profile {
        GrpcCompareProfile::General => {}
        GrpcCompareProfile::Latency => {
            if endpoint.connect_latency_ms.is_some_and(|ms| ms > 300) {
                notes.push(
                    "Connect latency is high for latency-sensitive streaming workloads."
                        .to_string(),
                );
            }
            if endpoint.first_event_latency_ms.is_none_or(|ms| ms > 1_500) {
                notes.push(
                    "Time-to-first-slot-update is slow or missing for latency-sensitive workloads."
                        .to_string(),
                );
            }
        }
        GrpcCompareProfile::Indexer => {
            if endpoint.slot_lag.is_none_or(|lag| lag > 50) {
                notes.push(
                    "Slot freshness is poor or unknown for indexer catch-up workloads.".to_string(),
                );
            }
            if !endpoint.stream_ok {
                notes.push(
                    "Slot stream did not reach a healthy state for indexer streaming.".to_string(),
                );
            } else if endpoint.updates_observed < 2 {
                notes.push(
                    "Few slot updates were observed within the window; the feed may be slow."
                        .to_string(),
                );
            }
        }
    }

    notes
}
