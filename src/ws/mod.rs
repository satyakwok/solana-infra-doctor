//! WebSocket readiness diagnostics: connect, subscribe (`slotSubscribe` by
//! default, see [`subscription`]), measure time-to-first-notification,
//! unsubscribe, and close — with exponential-backoff reconnect on a dropped or
//! failed connection.

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
use reliakit_backoff::Backoff;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::{timeout, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub mod analysis;
pub mod subscription;
pub use analysis::{classify, derive_ws_url};
pub use subscription::Subscription;

const UNSUBSCRIBE_ACK_MS: u64 = 1_000;
/// How many times to reconnect after a failed or dropped attempt before giving up.
const MAX_RECONNECTS: u32 = 3;

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
    /// The subscribe method used (e.g. `slotSubscribe`, `logsSubscribe`).
    pub subscription_method: &'static str,
    /// Whether the subscription was confirmed.
    pub subscribed: bool,
    /// Time from subscribe to the first notification, in milliseconds.
    pub time_to_first_notification_ms: Option<u128>,
    /// The slot reported by the first notification, if the subscription carries
    /// one (`slotSubscribe`); `null` for log subscriptions.
    pub first_slot: Option<u64>,
    /// Whether the unsubscribe was sent successfully.
    pub unsubscribed: bool,
    /// Whether the connection closed cleanly.
    pub closed_cleanly: bool,
    /// How many times the connection was retried after a failed/dropped attempt.
    pub reconnect_attempts: u32,
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
            subscription_method: Subscription::Slot.method(),
            subscribed: false,
            time_to_first_notification_ms: None,
            first_slot: None,
            unsubscribed: false,
            closed_cleanly: false,
            reconnect_attempts: 0,
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

/// Diagnose WebSocket readiness: derive the `ws`/`wss` URL, connect, subscribe,
/// measure time-to-first-notification, unsubscribe, and close. A failed or
/// dropped attempt is retried with exponential backoff before giving up. Returns
/// a redaction-safe [`WsReport`].
pub async fn run_ws(args: WsArgs) -> Result<WsReport, AppError> {
    let subscription = args.subscription;
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
    report.subscription_method = subscription.method();
    let duration = Duration::from_millis(args.timeout_ms);

    // Reconnect with exponential backoff. Only connection-level failures (could
    // not connect, or the socket dropped before the first notification) are
    // retried — a connected-but-quiet endpoint is not, since reconnecting would
    // not make it faster.
    let backoff = Backoff::exponential(Duration::from_millis(250), 2)
        .with_max_delay(Duration::from_secs(2))
        .with_max_retries(MAX_RECONNECTS);

    let mut attempt = 0u32;
    loop {
        let outcome = attempt_session(ws_url.as_str(), duration, subscription).await;
        outcome.apply_to(&mut report);
        if outcome.got_notification || !outcome.reconnectable {
            break;
        }
        match backoff.delay(attempt) {
            Some(delay) => {
                report.reconnect_attempts += 1;
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            None => break,
        }
    }

    Ok(report.finish())
}

/// The result of a single connect/subscribe/wait attempt.
#[derive(Debug, Default)]
struct Attempt {
    connected: bool,
    connect_latency_ms: Option<u128>,
    subscribed: bool,
    time_to_first_notification_ms: Option<u128>,
    first_slot: Option<u64>,
    got_notification: bool,
    unsubscribed: bool,
    closed_cleanly: bool,
    /// Whether this failure was connection-level and therefore worth retrying.
    reconnectable: bool,
}

impl Attempt {
    /// Fold this attempt into the report. `connected`/`subscribed` accumulate
    /// (whether we *ever* got that far), while the notification timing, slot, and
    /// teardown reflect the attempt that actually received a notification.
    fn apply_to(&self, report: &mut WsReport) {
        report.connected |= self.connected;
        if self.connect_latency_ms.is_some() {
            report.connect_latency_ms = self.connect_latency_ms;
        }
        report.subscribed |= self.subscribed;
        if self.got_notification {
            report.time_to_first_notification_ms = self.time_to_first_notification_ms;
            report.first_slot = self.first_slot;
            report.unsubscribed = self.unsubscribed;
            report.closed_cleanly = self.closed_cleanly;
        }
    }
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// One connect → subscribe → wait-for-first-notification cycle. Sets
/// `reconnectable` when the failure is connection-level (could not connect, send
/// failed, or the socket dropped) and therefore worth a reconnect.
async fn attempt_session(ws_url: &str, duration: Duration, subscription: Subscription) -> Attempt {
    let mut attempt = Attempt::default();

    let started = Instant::now();
    let mut stream = match timeout(duration, connect_async(ws_url)).await {
        Ok(Ok((stream, _response))) => {
            attempt.connected = true;
            attempt.connect_latency_ms = Some(started.elapsed().as_millis());
            stream
        }
        Ok(Err(_)) | Err(_) => {
            attempt.reconnectable = true;
            return attempt;
        }
    };

    let subscribe_started = Instant::now();
    let deadline = subscribe_started + duration;
    if stream
        .send(Message::text(subscription.subscribe_request()))
        .await
        .is_err()
    {
        attempt.reconnectable = true;
        let _ = stream.close(None).await;
        return attempt;
    }

    let mut sub_id = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break; // timed out while connected — not a connection failure
        }
        match timeout(remaining, stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => match serde_json::from_str::<Value>(&text) {
                Ok(value) => {
                    if value.get("method").and_then(Value::as_str)
                        == Some(subscription.notification())
                    {
                        attempt.first_slot = subscription.extract_slot(&value);
                        attempt.time_to_first_notification_ms =
                            Some(subscribe_started.elapsed().as_millis());
                        attempt.got_notification = true;
                        break;
                    }
                    // A confirmation `{"result":<subId>,"id":1}` confirms the
                    // subscription and gives the id used to unsubscribe.
                    if value.get("id").and_then(Value::as_u64) == Some(1)
                        && value.get("result").is_some()
                    {
                        attempt.subscribed = true;
                        sub_id = value.get("result").and_then(Value::as_u64);
                    }
                }
                Err(_) => break, // malformed frame — not a connection failure
            },
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                attempt.reconnectable = true; // server closed mid-stream
                break;
            }
            Ok(Some(Ok(_))) => {} // ping/pong/binary
            Ok(Some(Err(_))) => {
                attempt.reconnectable = true; // protocol error
                break;
            }
            Err(_) => break, // timed out — not a connection failure
        }
    }

    if attempt.got_notification {
        attempt.subscribed = true;
        attempt.unsubscribed = unsubscribe(&mut stream, sub_id, subscription).await;
        attempt.closed_cleanly = stream.close(None).await.is_ok();
    } else {
        let _ = stream.close(None).await;
    }

    attempt
}

async fn unsubscribe(
    stream: &mut WsStream,
    sub_id: Option<u64>,
    subscription: Subscription,
) -> bool {
    let Some(id) = sub_id else {
        return false;
    };
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"{}","params":[{id}]}}"#,
        subscription.unsubscribe_method()
    );
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
        // A notification arrived (slot subscriptions also carry a slot; log
        // subscriptions do not, so key on the timing, not the slot).
        report.time_to_first_notification_ms.is_some(),
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
