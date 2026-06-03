use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latency {
    pub millis: u128,
}

impl Latency {
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            millis: duration.as_millis(),
        }
    }
}

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
