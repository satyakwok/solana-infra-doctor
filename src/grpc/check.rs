//! The bounded Yellowstone gRPC network probe: connect (optionally over TLS),
//! attach the `x-token` when supplied, run safe unary checks, and open a narrow
//! slot-only `Subscribe` stream. Every step is deadline-bounded; nothing here
//! sends transactions, modifies state, or subscribes to broad filters.

use super::{
    AuthStatus, CheckStatus, GrpcEndpoint, StreamResult, UnaryResult, error_kind::GrpcErrorKind,
};
use crate::redact;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{Instant, timeout};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Code, Request, Status};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, GetBlockHeightRequest, GetLatestBlockhashRequest, GetSlotRequest,
    GetVersionRequest, IsBlockhashValidRequest, PingRequest, SubscribeRequest,
    SubscribeRequestFilterSlots, geyser_client::GeyserClient, subscribe_update::UpdateOneof,
};

/// Stop observing the slot stream once this many updates confirm it is live,
/// even if `--duration` has not fully elapsed (keeps the default run snappy).
const CONFIRM_UPDATES: u64 = 3;
/// Cap a displayed server version string so a chatty server cannot bloat output.
const VERSION_MAX_LEN: usize = 80;

/// The raw outcome of the network probe, assembled into a report by the caller.
pub(crate) struct ProbeOutcome {
    pub connect_latency_ms: Option<u128>,
    pub transport_error: Option<(GrpcErrorKind, String)>,
    pub auth: AuthStatus,
    pub unary: Vec<UnaryResult>,
    pub stream: StreamResult,
    pub latest_slot: Option<u64>,
}

/// Accumulates the authentication signal across every request: a definitive
/// auth-code rejection wins; otherwise any accepted request means `Accepted`.
#[derive(Default)]
struct AuthSignal {
    saw_success: bool,
    failure: Option<AuthStatus>,
}

impl AuthSignal {
    fn record_success(&mut self) {
        self.saw_success = true;
    }

    fn record_status(&mut self, status: &Status) {
        let mapped = match status.code() {
            Code::Unauthenticated => Some(AuthStatus::Unauthenticated),
            Code::PermissionDenied => Some(AuthStatus::PermissionDenied),
            _ => None,
        };
        if let Some(mapped) = mapped {
            self.failure.get_or_insert(mapped);
        }
    }

    fn resolve(&self) -> AuthStatus {
        self.failure.unwrap_or(if self.saw_success {
            AuthStatus::Accepted
        } else {
            AuthStatus::Unknown
        })
    }
}

/// Connect to `endpoint`, run the unary probes and the slot-stream probe, and
/// return a [`ProbeOutcome`]. Never panics on malformed remote data.
pub(crate) async fn probe(
    endpoint: &GrpcEndpoint,
    token: Option<&MetadataValue<Ascii>>,
    timeout_dur: Duration,
    stream_duration: Duration,
) -> ProbeOutcome {
    let started = Instant::now();
    let channel = match connect(endpoint, timeout_dur).await {
        Ok(channel) => channel,
        Err((kind, detail)) => {
            return ProbeOutcome {
                connect_latency_ms: None,
                transport_error: Some((kind, detail)),
                auth: AuthStatus::Unknown,
                unary: Vec::new(),
                stream: not_attempted_stream(),
                latest_slot: None,
            };
        }
    };
    let connect_latency_ms = Some(started.elapsed().as_millis());

    let mut client = GeyserClient::new(channel);
    let mut auth = AuthSignal::default();

    let unary = run_unary(&mut client, token, &mut auth).await;
    let (stream, latest_slot) = run_stream(&mut client, token, stream_duration, &mut auth).await;

    ProbeOutcome {
        connect_latency_ms,
        transport_error: None,
        auth: auth.resolve(),
        unary,
        stream,
        latest_slot,
    }
}

/// Build the transport channel, applying TLS for `https` endpoints. Connection
/// and per-request deadlines are bounded by `timeout_dur`.
async fn connect(
    endpoint: &GrpcEndpoint,
    timeout_dur: Duration,
) -> Result<Channel, (GrpcErrorKind, String)> {
    let mut transport = Endpoint::from_shared(endpoint.connect_target())
        .map_err(|error| {
            (
                GrpcErrorKind::ConnectError,
                redact::redact_text(&error.to_string()),
            )
        })?
        .connect_timeout(timeout_dur)
        .timeout(timeout_dur);

    if endpoint.is_tls() {
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name(endpoint.domain().to_string());
        transport = transport
            .tls_config(tls)
            .map_err(|error| classify_transport_error(&error))?;
    }

    transport
        .connect()
        .await
        .map_err(|error| classify_transport_error(&error))
}

/// Classify a tonic transport error into a gRPC error kind and a short,
/// secret-free description. The raw error is redacted defensively even though
/// transport errors do not normally embed the URL.
fn classify_transport_error(error: &tonic::transport::Error) -> (GrpcErrorKind, String) {
    // Walk the source chain to find the most specific cause.
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    let lower = text.to_ascii_lowercase();

    let (kind, detail) = if lower.contains("dns") || lower.contains("resolve") {
        (GrpcErrorKind::DnsError, "could not resolve the host")
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("handshake")
    {
        (GrpcErrorKind::TlsError, "TLS handshake failed")
    } else if lower.contains("timed out") || lower.contains("timeout") || lower.contains("deadline")
    {
        (GrpcErrorKind::Timeout, "connection timed out")
    } else {
        (GrpcErrorKind::ConnectError, "could not connect")
    };
    (kind, detail.to_string())
}

/// Wrap a message in a request, attaching the `x-token` metadata when supplied.
fn authed<T>(message: T, token: Option<&MetadataValue<Ascii>>) -> Request<T> {
    let mut request = Request::new(message);
    if let Some(token) = token {
        request.metadata_mut().insert("x-token", token.clone());
    }
    request
}

/// Classify a per-call gRPC status into a unary result status, error kind, and a
/// safe detail string. `UNIMPLEMENTED` is a SKIP (optional capability), not a
/// failure of the whole endpoint.
fn classify_call_error(status: &Status) -> (CheckStatus, GrpcErrorKind, String) {
    let kind = GrpcErrorKind::from_code(status.code());
    if status.code() == Code::Unimplemented {
        (
            CheckStatus::Skip,
            kind,
            "not implemented by this endpoint".to_string(),
        )
    } else {
        let message = redact::redact_text(status.message());
        let message = if message.is_empty() {
            kind.as_str().to_string()
        } else {
            message
        };
        (CheckStatus::Fail, kind, message)
    }
}

/// Run the safe unary probes in order. The token (if any) authenticates each
/// call; the auth signal is updated from successes and auth-code rejections.
async fn run_unary(
    client: &mut GeyserClient<Channel>,
    token: Option<&MetadataValue<Ascii>>,
    auth: &mut AuthSignal,
) -> Vec<UnaryResult> {
    let mut results = Vec::with_capacity(6);

    // Ping.
    let started = Instant::now();
    match client.ping(authed(PingRequest { count: 1 }, token)).await {
        Ok(_) => {
            auth.record_success();
            results.push(pass("Ping", started, "pong".to_string()));
        }
        Err(status) => {
            auth.record_status(&status);
            results.push(fail("Ping", started, &status));
        }
    }

    // GetVersion.
    let started = Instant::now();
    match client
        .get_version(authed(GetVersionRequest {}, token))
        .await
    {
        Ok(response) => {
            auth.record_success();
            let version = truncate(&redact::redact_text(&response.into_inner().version));
            results.push(pass("GetVersion", started, version));
        }
        Err(status) => {
            auth.record_status(&status);
            results.push(fail("GetVersion", started, &status));
        }
    }

    // GetSlot.
    let started = Instant::now();
    match client
        .get_slot(authed(GetSlotRequest { commitment: None }, token))
        .await
    {
        Ok(response) => {
            auth.record_success();
            let slot = response.into_inner().slot;
            results.push(pass("GetSlot", started, format!("slot {slot}")));
        }
        Err(status) => {
            auth.record_status(&status);
            results.push(fail("GetSlot", started, &status));
        }
    }

    // GetBlockHeight.
    let started = Instant::now();
    match client
        .get_block_height(authed(GetBlockHeightRequest { commitment: None }, token))
        .await
    {
        Ok(response) => {
            auth.record_success();
            let height = response.into_inner().block_height;
            results.push(pass(
                "GetBlockHeight",
                started,
                format!("block height {height}"),
            ));
        }
        Err(status) => {
            auth.record_status(&status);
            results.push(fail("GetBlockHeight", started, &status));
        }
    }

    // GetLatestBlockhash (captures the blockhash for IsBlockhashValid).
    let started = Instant::now();
    let blockhash = match client
        .get_latest_blockhash(authed(
            GetLatestBlockhashRequest { commitment: None },
            token,
        ))
        .await
    {
        Ok(response) => {
            auth.record_success();
            let value = response.into_inner();
            let hash = value.blockhash;
            results.push(pass(
                "GetLatestBlockhash",
                started,
                format!("blockhash {hash}"),
            ));
            Some(hash)
        }
        Err(status) => {
            auth.record_status(&status);
            results.push(fail("GetLatestBlockhash", started, &status));
            None
        }
    };

    // IsBlockhashValid (depends on the blockhash above).
    match blockhash {
        Some(hash) => {
            let started = Instant::now();
            match client
                .is_blockhash_valid(authed(
                    IsBlockhashValidRequest {
                        blockhash: hash,
                        commitment: None,
                    },
                    token,
                ))
                .await
            {
                Ok(response) => {
                    auth.record_success();
                    let valid = response.into_inner().valid;
                    let detail = if valid {
                        "latest blockhash is valid".to_string()
                    } else {
                        "latest blockhash is not valid".to_string()
                    };
                    results.push(pass("IsBlockhashValid", started, detail));
                }
                Err(status) => {
                    auth.record_status(&status);
                    results.push(fail("IsBlockhashValid", started, &status));
                }
            }
        }
        None => results.push(UnaryResult {
            method: "IsBlockhashValid",
            status: CheckStatus::Skip,
            latency_ms: None,
            detail: "skipped (no blockhash from GetLatestBlockhash)".to_string(),
            error_kind: None,
        }),
    }

    results
}

/// Open a slot-only `Subscribe` stream and observe it for a bounded window,
/// measuring time-to-first-slot-update and recording the latest slot.
async fn run_stream(
    client: &mut GeyserClient<Channel>,
    token: Option<&MetadataValue<Ascii>>,
    duration: Duration,
    auth: &mut AuthSignal,
) -> (StreamResult, Option<u64>) {
    // A narrow slot-only subscription: never accounts, transactions, or blocks.
    let mut slots = HashMap::new();
    slots.insert(
        "sol-doctor".to_string(),
        SubscribeRequestFilterSlots {
            filter_by_commitment: None,
            interslot_updates: None,
        },
    );
    let request = SubscribeRequest {
        slots,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    };
    // Keep the request stream open (never half-close) for the observation window.
    let outbound =
        futures_util::stream::once(async move { request }).chain(futures_util::stream::pending());

    let subscribe_started = Instant::now();
    let mut inbound = match client.subscribe(authed(outbound, token)).await {
        Ok(response) => {
            auth.record_success();
            response.into_inner()
        }
        Err(status) => {
            auth.record_status(&status);
            let (_, kind, detail) = classify_call_error(&status);
            return (
                StreamResult {
                    status: CheckStatus::Fail,
                    opened: false,
                    first_event_latency_ms: None,
                    updates_observed: 0,
                    latest_slot: None,
                    detail: format!("subscribe rejected: {detail}"),
                    error_kind: Some(kind),
                },
                None,
            );
        }
    };

    let deadline = subscribe_started + duration;
    let mut first_event_latency_ms: Option<u128> = None;
    let mut latest_slot: Option<u64> = None;
    let mut updates_observed: u64 = 0;
    let mut closed = false;
    let mut stream_error: Option<GrpcErrorKind> = None;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, inbound.message()).await {
            Ok(Ok(Some(update))) => {
                // Keepalives (ping/pong) and any unexpected variant are ignored
                // for a slot-only subscription; only slot updates are counted.
                if let Some(UpdateOneof::Slot(slot)) = update.update_oneof {
                    updates_observed += 1;
                    latest_slot = Some(slot.slot);
                    if first_event_latency_ms.is_none() {
                        first_event_latency_ms = Some(subscribe_started.elapsed().as_millis());
                    }
                    // Enough updates to confirm a live stream; stop early.
                    if updates_observed >= CONFIRM_UPDATES {
                        break;
                    }
                }
            }
            Ok(Ok(None)) => {
                closed = true;
                break;
            }
            Ok(Err(status)) => {
                auth.record_status(&status);
                stream_error = Some(GrpcErrorKind::from_code(status.code()));
                closed = true;
                break;
            }
            // Local deadline reached while waiting — end the observation window.
            Err(_) => break,
        }
    }

    let result = if first_event_latency_ms.is_some() {
        let ms = first_event_latency_ms.unwrap_or_default();
        StreamResult {
            status: CheckStatus::Pass,
            opened: true,
            first_event_latency_ms,
            updates_observed,
            latest_slot,
            detail: format!("first slot update in {ms} ms"),
            error_kind: None,
        }
    } else if closed {
        let kind = stream_error.unwrap_or(GrpcErrorKind::StreamClosed);
        StreamResult {
            status: CheckStatus::Fail,
            opened: true,
            first_event_latency_ms: None,
            updates_observed,
            latest_slot,
            detail: "stream closed before a slot update".to_string(),
            error_kind: Some(kind),
        }
    } else {
        StreamResult {
            status: CheckStatus::Fail,
            opened: true,
            first_event_latency_ms: None,
            updates_observed,
            latest_slot,
            detail: "no slot update before the deadline".to_string(),
            error_kind: Some(GrpcErrorKind::NoFirstEvent),
        }
    };

    (result, latest_slot)
}

fn pass(method: &'static str, started: Instant, detail: String) -> UnaryResult {
    UnaryResult {
        method,
        status: CheckStatus::Pass,
        latency_ms: Some(started.elapsed().as_millis()),
        detail,
        error_kind: None,
    }
}

fn fail(method: &'static str, started: Instant, status: &Status) -> UnaryResult {
    let (check_status, kind, detail) = classify_call_error(status);
    UnaryResult {
        method,
        status: check_status,
        latency_ms: Some(started.elapsed().as_millis()),
        detail,
        error_kind: Some(kind),
    }
}

fn not_attempted_stream() -> StreamResult {
    StreamResult {
        status: CheckStatus::Skip,
        opened: false,
        first_event_latency_ms: None,
        updates_observed: 0,
        latest_slot: None,
        detail: "not attempted (no connection)".to_string(),
        error_kind: None,
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= VERSION_MAX_LEN {
        return text.to_string();
    }
    let truncated: String = text.chars().take(VERSION_MAX_LEN).collect();
    format!("{truncated}…")
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn auth_signal_prefers_failure() {
        let mut signal = AuthSignal::default();
        signal.record_success();
        signal.record_status(&Status::unauthenticated("nope"));
        assert_eq!(signal.resolve(), AuthStatus::Unauthenticated);
    }

    #[test]
    fn auth_signal_accepted_on_success_only() {
        let mut signal = AuthSignal::default();
        signal.record_success();
        assert_eq!(signal.resolve(), AuthStatus::Accepted);
    }

    #[test]
    fn auth_signal_unknown_without_signal() {
        let signal = AuthSignal::default();
        assert_eq!(signal.resolve(), AuthStatus::Unknown);
    }

    #[test]
    fn non_auth_status_does_not_set_failure() {
        let mut signal = AuthSignal::default();
        signal.record_status(&Status::unavailable("down"));
        assert_eq!(signal.resolve(), AuthStatus::Unknown);
    }

    #[test]
    fn unimplemented_call_is_skip_not_fail() {
        let (status, kind, _) = classify_call_error(&Status::unimplemented("no"));
        assert_eq!(status, CheckStatus::Skip);
        assert_eq!(kind, GrpcErrorKind::Unimplemented);
    }

    #[test]
    fn errored_call_is_fail_with_kind() {
        let (status, kind, _) = classify_call_error(&Status::unavailable("down"));
        assert_eq!(status, CheckStatus::Fail);
        assert_eq!(kind, GrpcErrorKind::Unavailable);
    }

    #[test]
    fn truncates_long_version() {
        let long = "v".repeat(200);
        let out = truncate(&long);
        assert!(out.chars().count() <= VERSION_MAX_LEN + 1);
    }
}
