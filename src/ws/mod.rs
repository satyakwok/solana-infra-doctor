//! WebSocket readiness diagnostics: connect, `slotSubscribe`, measure
//! time-to-first-slot-notification, unsubscribe, and close.

use crate::{
    cli::WsArgs,
    color::Palette,
    error::AppError,
    output::{
        style::{self, Status},
        table::{self, Cell},
    },
    redact,
    rpc::RpcEndpoint,
    verdict::Verdict,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::{timeout, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub mod analysis;
pub use analysis::{classify, derive_ws_url};

const SLOT_SUBSCRIBE_METHOD: &str = "slotSubscribe";
const SLOT_SUBSCRIBE_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"slotSubscribe"}"#;
const UNSUBSCRIBE_ACK_MS: u64 = 1_000;

/// The result of a WebSocket readiness diagnostic. This is the serialized shape
/// emitted by `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct WsReport {
    /// Overall readiness verdict (drives the process exit code).
    pub verdict: Verdict,
    /// The redacted RPC URL the WebSocket URL was derived from.
    pub rpc_url: String,
    /// The redacted `ws`/`wss` URL that was tested.
    pub ws_url: String,
    /// Whether the WebSocket connection was established.
    pub connected: bool,
    /// Time to establish the connection, in milliseconds.
    pub connect_latency_ms: Option<u128>,
    /// The subscription method used (`slotSubscribe`).
    pub subscription_method: &'static str,
    /// Whether the subscription was confirmed.
    pub subscribed: bool,
    /// Time from subscribe to the first notification, in milliseconds.
    pub time_to_first_notification_ms: Option<u128>,
    /// The slot reported by the first notification, if any.
    pub first_slot: Option<u64>,
    /// Whether the unsubscribe was sent successfully.
    pub unsubscribed: bool,
    /// Whether the connection closed cleanly.
    pub closed_cleanly: bool,
    /// One-line, human-readable summary of the verdict.
    pub summary: String,
    /// Advisory notes about degraded behavior.
    pub notes: Vec<String>,
}

impl WsReport {
    fn new(rpc_url: String, ws_url: String) -> Self {
        Self {
            verdict: Verdict::Unknown,
            rpc_url,
            ws_url,
            connected: false,
            connect_latency_ms: None,
            subscription_method: SLOT_SUBSCRIBE_METHOD,
            subscribed: false,
            time_to_first_notification_ms: None,
            first_slot: None,
            unsubscribed: false,
            closed_cleanly: false,
            summary: String::new(),
            notes: Vec::new(),
        }
    }

    fn finish(mut self) -> Self {
        let (verdict, summary, notes) = classify(&self);
        self.verdict = verdict;
        self.summary = summary;
        self.notes = notes;
        self
    }
}

/// Diagnose WebSocket readiness: derive the `ws`/`wss` URL, connect, subscribe
/// with `slotSubscribe`, measure time-to-first-notification, unsubscribe, close,
/// and return a redaction-safe [`WsReport`].
pub async fn run_ws(args: WsArgs) -> Result<WsReport, AppError> {
    let endpoint = match RpcEndpoint::parse(&args.rpc) {
        Ok(endpoint) => endpoint,
        Err(AppError::InvalidRpcUrl { reason }) => {
            let mut report = WsReport::new("<invalid>".to_string(), "<invalid>".to_string());
            report.summary = format!("invalid RPC URL: {}", redact::redact_text(&reason));
            report.verdict = Verdict::Bad;
            return Ok(report);
        }
        Err(error) => return Err(error),
    };

    let rpc_redacted = endpoint.redacted();
    let ws_url = match derive_ws_url(endpoint.as_url(), args.ws.as_deref()) {
        Ok(url) => url,
        Err(AppError::InvalidRpcUrl { reason }) => {
            let mut report = WsReport::new(rpc_redacted, "<invalid>".to_string());
            report.summary = format!("invalid WebSocket URL: {}", redact::redact_text(&reason));
            report.verdict = Verdict::Bad;
            return Ok(report);
        }
        Err(error) => return Err(error),
    };

    let ws_redacted = redact::redact_url(&ws_url);
    let mut report = WsReport::new(rpc_redacted, ws_redacted);
    let duration = Duration::from_millis(args.timeout_ms);

    let started = Instant::now();
    let mut stream = match timeout(duration, connect_async(ws_url.as_str())).await {
        Ok(Ok((stream, _response))) => {
            report.connected = true;
            report.connect_latency_ms = Some(started.elapsed().as_millis());
            stream
        }
        Ok(Err(_)) | Err(_) => return Ok(report.finish()),
    };

    let subscribe_started = Instant::now();
    let deadline = subscribe_started + duration;
    let mut sub_id = None;
    let mut got_notification = false;

    if stream
        .send(Message::text(SLOT_SUBSCRIBE_REQUEST))
        .await
        .is_ok()
    {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match timeout(remaining, stream.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        if value.get("method").and_then(Value::as_str) == Some("slotNotification") {
                            report.first_slot = value
                                .get("params")
                                .and_then(|params| params.get("result"))
                                .and_then(|result| result.get("slot"))
                                .and_then(Value::as_u64);
                            report.time_to_first_notification_ms =
                                Some(subscribe_started.elapsed().as_millis());
                            got_notification = true;
                            break;
                        }
                        // A confirmation `{"result":<subId>,"id":1}` confirms the
                        // subscription and gives the id used to unsubscribe.
                        if value.get("id").and_then(Value::as_u64) == Some(1)
                            && value.get("result").is_some()
                        {
                            report.subscribed = true;
                            sub_id = value.get("result").and_then(Value::as_u64);
                        }
                    }
                    Err(_) => break, // malformed frame
                },
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break, // server closed
                Ok(Some(Ok(_))) => {}                                // ping/pong/binary
                Ok(Some(Err(_))) => break,                           // protocol error
                Err(_) => break,                                     // timed out
            }
        }
    }

    if got_notification {
        report.subscribed = true;
        report.unsubscribed = unsubscribe(&mut stream, sub_id).await;
        report.closed_cleanly = stream.close(None).await.is_ok();
    } else {
        let _ = stream.close(None).await;
    }

    Ok(report.finish())
}

async fn unsubscribe(
    stream: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    sub_id: Option<u64>,
) -> bool {
    let Some(id) = sub_id else {
        return false;
    };
    let request =
        format!(r#"{{"jsonrpc":"2.0","id":2,"method":"slotUnsubscribe","params":[{id}]}}"#);
    if stream.send(Message::text(request)).await.is_err() {
        return false;
    }
    // Best-effort: wait briefly for the acknowledgement, but treat a sent
    // unsubscribe as success even if the ack does not arrive in time.
    let _ = timeout(Duration::from_millis(UNSUBSCRIBE_ACK_MS), stream.next()).await;
    true
}

/// Render the human-readable `ws` report.
///
/// The default view is a compact step table (connect, subscribe, first
/// notification, unsubscribe, close). `verbose` shows the full redacted RPC URL
/// and any diagnostic notes. `verbose` affects human output only.
pub fn render_human(report: &WsReport, palette: Palette, verbose: bool) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor · WebSocket Readiness"));
    output.push_str("\n\n");

    // Target: RPC as a safe hostname by default (full redacted URL in verbose);
    // the WebSocket URL is already redacted and short, so it is always shown.
    output.push_str(&palette.heading("Target"));
    output.push('\n');
    let rpc_value = if verbose {
        report.rpc_url.clone()
    } else {
        style::endpoint_label(&report.rpc_url)
    };
    output.push_str(&table::render(
        &[
            vec![
                Cell::styled("RPC", palette.label("RPC")),
                Cell::plain(rpc_value),
            ],
            vec![
                Cell::styled("WebSocket", palette.label("WebSocket")),
                Cell::plain(report.ws_url.clone()),
            ],
        ],
        3,
    ));
    output.push('\n');

    // Result.
    output.push_str(&palette.heading("Result"));
    output.push('\n');
    output.push_str(&table::render(
        &[vec![
            Cell::styled(report.verdict.to_string(), palette.verdict(report.verdict)),
            Cell::plain(report.summary.clone()),
        ]],
        3,
    ));
    output.push('\n');

    // Checks: one row per WebSocket step.
    output.push_str(&palette.heading("Checks"));
    output.push('\n');
    let subscribe_detail = if report.subscribed {
        format!("{} · id 1", report.subscription_method)
    } else {
        report.subscription_method.to_string()
    };
    let first_detail = report
        .time_to_first_notification_ms
        .map(|ms| match report.first_slot {
            Some(slot) => format!("{} · slot {slot}", style::millis(ms)),
            None => style::millis(ms),
        })
        .unwrap_or_default();
    let mut rows = vec![vec![
        Cell::styled("Check", palette.label("Check")),
        Cell::styled("Status", palette.label("Status")),
        Cell::styled("Detail", palette.label("Detail")),
    ]];
    rows.push(step_row(
        palette,
        "Connect",
        report.connected,
        report
            .connect_latency_ms
            .map(style::millis)
            .unwrap_or_default(),
    ));
    rows.push(step_row(
        palette,
        "Subscribe",
        report.subscribed,
        subscribe_detail,
    ));
    rows.push(step_row(
        palette,
        "First notification",
        report.first_slot.is_some(),
        first_detail,
    ));
    rows.push(step_row(
        palette,
        "Unsubscribe",
        report.unsubscribed,
        String::new(),
    ));
    rows.push(step_row(
        palette,
        "Close",
        report.closed_cleanly,
        String::new(),
    ));
    output.push_str(&table::render(&rows, 4));

    // Notes matter for degraded endpoints, so show them whenever present.
    if !report.notes.is_empty() {
        output.push('\n');
        output.push_str(&palette.heading("Notes"));
        output.push('\n');
        for note in &report.notes {
            output.push_str(&format!("- {note}\n"));
        }
    }

    if !verbose {
        output.push('\n');
        output.push_str(&palette.dim("Tip: run with --verbose to see full details."));
        output.push('\n');
    }

    output
}

fn step_row(palette: Palette, check: &str, ok: bool, detail: String) -> Vec<Cell> {
    let status = if ok { Status::Pass } else { Status::Fail };
    vec![
        Cell::plain(check.to_string()),
        Cell::styled(status.label(), status.paint(palette)),
        Cell::plain(detail),
    ]
}

pub fn render_json(report: &WsReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}
