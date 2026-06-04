//! A simple millisecond latency measurement and averaging helper.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A latency measurement in whole milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latency {
    /// Elapsed time in milliseconds.
    pub millis: u128,
}

impl Latency {
    /// Construct a [`Latency`] from a [`Duration`], truncating to whole milliseconds.
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            millis: duration.as_millis(),
        }
    }
}

/// A percentile summary of repeated latency samples, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Number of samples collected.
    pub count: usize,
    /// Fastest sample.
    pub min_ms: u128,
    /// Median (50th percentile).
    pub p50_ms: u128,
    /// 95th percentile — the tail latency a single sample tends to hide.
    pub p95_ms: u128,
    /// Slowest sample.
    pub max_ms: u128,
}

impl LatencyStats {
    /// Compute the summary from latency samples (milliseconds), or `None` when
    /// there are no samples. Percentiles use the nearest-rank method over the
    /// sorted samples.
    pub fn from_samples(samples: &[u128]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let count = sorted.len();
        // Nearest-rank: rank = ceil(p/100 * count), clamped to [1, count].
        let nearest_rank = |percentile: u128| {
            let rank = (percentile * count as u128).div_ceil(100);
            let index = rank.clamp(1, count as u128) as usize - 1;
            sorted[index]
        };
        Some(Self {
            count,
            min_ms: sorted[0],
            p50_ms: nearest_rank(50),
            p95_ms: nearest_rank(95),
            max_ms: sorted[count - 1],
        })
    }
}

/// The mean of the given latencies in milliseconds, or `None` when the iterator
/// is empty.
pub fn average_latency_ms(latencies: impl IntoIterator<Item = Latency>) -> Option<u128> {
    let mut total = 0u128;
    let mut count = 0u128;

    for latency in latencies {
        total = total.saturating_add(latency.millis);
        count = count.saturating_add(1);
    }

    total.checked_div(count)
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn converts_duration_to_milliseconds() {
        assert_eq!(Latency::from_duration(Duration::from_millis(42)).millis, 42);
    }

    #[test]
    fn averages_latencies() {
        let latencies = [
            Latency { millis: 100 },
            Latency { millis: 200 },
            Latency { millis: 300 },
        ];

        assert_eq!(average_latency_ms(latencies), Some(200));
    }

    #[test]
    fn average_is_none_without_samples() {
        assert_eq!(average_latency_ms([]), None);
    }
}
