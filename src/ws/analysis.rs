//! Pure WebSocket analysis: verdict classification and `ws`/`wss` URL derivation.

use super::WsReport;
use crate::{error::AppError, verdict::Verdict};
use url::Url;

const GOOD_NOTIFY_MS: u128 = 2_000;

pub fn classify(report: &WsReport) -> (Verdict, String, Vec<String>) {
    let (verdict, summary, mut notes) = classify_outcome(report);
    // Surface reconnects regardless of the final verdict — a connection that
    // needed retries is worth noting even when it eventually succeeded.
    if report.reconnect_attempts > 0 {
        // Neutral wording: accurate whether the retries eventually succeeded
        // (GOOD/WARNING) or were exhausted (BAD).
        notes.insert(
            0,
            format!(
                "Connection was retried {} time(s) during the check.",
                report.reconnect_attempts
            ),
        );
    }
    (verdict, summary, notes)
}

fn classify_outcome(report: &WsReport) -> (Verdict, String, Vec<String>) {
    if !report.connected {
        return (
            Verdict::Bad,
            "WebSocket connection failed".to_string(),
            Vec::new(),
        );
    }
    if !report.subscribed {
        return (
            Verdict::Bad,
            format!("{} did not succeed", report.subscription_method),
            Vec::new(),
        );
    }
    match report.time_to_first_notification_ms {
        None => (
            Verdict::Bad,
            "No notification received before timeout".to_string(),
            Vec::new(),
        ),
        Some(ms) if ms <= GOOD_NOTIFY_MS && report.unsubscribed && report.closed_cleanly => (
            Verdict::Good,
            "WebSocket readiness checks passed".to_string(),
            Vec::new(),
        ),
        Some(ms) => {
            let mut notes = Vec::new();
            if ms > GOOD_NOTIFY_MS {
                notes.push(format!("First notification was slow at {ms} ms."));
            }
            if !report.unsubscribed {
                notes.push("Unsubscribe did not complete cleanly.".to_string());
            }
            if !report.closed_cleanly {
                notes.push("Connection did not close cleanly.".to_string());
            }
            (
                Verdict::Warning,
                "WebSocket is reachable, but realtime behavior is degraded".to_string(),
                notes,
            )
        }
    }
}

/// Derive a `ws`/`wss` URL from the HTTP RPC URL, or validate an explicit override.
pub fn derive_ws_url(rpc: &Url, override_ws: Option<&str>) -> Result<Url, AppError> {
    if let Some(raw) = override_ws {
        let parsed = Url::parse(raw).map_err(|error| AppError::InvalidRpcUrl {
            reason: error.to_string(),
        })?;
        return match parsed.scheme() {
            "ws" | "wss" => Ok(parsed),
            other => Err(AppError::InvalidRpcUrl {
                reason: format!("unsupported WebSocket scheme '{other}', expected ws or wss"),
            }),
        };
    }

    let mut ws = rpc.clone();
    let scheme = match rpc.scheme() {
        "https" => "wss",
        "http" => "ws",
        other => {
            return Err(AppError::InvalidRpcUrl {
                reason: format!("cannot derive WebSocket URL from scheme '{other}'"),
            })
        }
    };
    ws.set_scheme(scheme)
        .map_err(|()| AppError::InvalidRpcUrl {
            reason: "cannot derive WebSocket URL".to_string(),
        })?;
    Ok(ws)
}
