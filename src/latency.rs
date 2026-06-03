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
