//! The WebSocket subscription kinds the `ws` diagnostic can exercise.
//!
//! This is the extension point for new subscriptions: add a variant and fill in
//! its method names, subscribe-request body, notification name, and (if it
//! carries one) a slot extractor. Everything else — connect, reconnect, timing,
//! unsubscribe, rendering — is subscription-agnostic.

use clap::ValueEnum;
use serde::Serialize;
use serde_json::Value;

/// A Solana PubSub subscription kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Subscription {
    /// `slotSubscribe` — notifies on each new slot (the default).
    Slot,
    /// `logsSubscribe` with the broad `"all"` filter — notifies on every
    /// transaction's logs. The diagnostic only waits for the *first*
    /// notification and then unsubscribes, so exposure to the firehose is brief;
    /// some providers may restrict the `"all"` filter.
    Logs,
}

impl Subscription {
    /// The subscribe RPC method name.
    pub fn method(self) -> &'static str {
        match self {
            Self::Slot => "slotSubscribe",
            Self::Logs => "logsSubscribe",
        }
    }

    /// The unsubscribe RPC method name.
    pub fn unsubscribe_method(self) -> &'static str {
        match self {
            Self::Slot => "slotUnsubscribe",
            Self::Logs => "logsUnsubscribe",
        }
    }

    /// The notification method name to match in incoming frames.
    pub fn notification(self) -> &'static str {
        match self {
            Self::Slot => "slotNotification",
            Self::Logs => "logsNotification",
        }
    }

    /// The JSON-RPC subscribe request body (request id `1`).
    pub fn subscribe_request(self) -> String {
        match self {
            Self::Slot => r#"{"jsonrpc":"2.0","id":1,"method":"slotSubscribe"}"#.to_string(),
            Self::Logs => {
                r#"{"jsonrpc":"2.0","id":1,"method":"logsSubscribe","params":["all"]}"#.to_string()
            }
        }
    }

    /// Extract the slot from a notification, when this subscription carries one
    /// (`slotSubscribe` does; `logsSubscribe` does not).
    pub fn extract_slot(self, notification: &Value) -> Option<u64> {
        match self {
            Self::Slot => notification
                .get("params")
                .and_then(|params| params.get("result"))
                .and_then(|result| result.get("slot"))
                .and_then(Value::as_u64),
            Self::Logs => None,
        }
    }
}
