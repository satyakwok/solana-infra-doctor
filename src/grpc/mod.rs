//! Yellowstone gRPC readiness diagnostics: connect, optionally authenticate with
//! an `x-token`, run safe unary probes, open a narrow slot-only `Subscribe`
//! stream, and (optionally) cross-check the latest observed slot against an HTTP
//! RPC endpoint.
//!
//! The command is safe by default: it never sends transactions, never modifies
//! remote state, subscribes only to slots (never accounts/transactions/blocks),
//! and bounds every connection, request, and stream with a deadline. The token
//! is read only from an environment variable and is never printed, serialized,
//! or logged.

use crate::{
    cli::GrpcCheckArgs,
    error::AppError,
    output::style::Status,
    redact,
    rpc::{JsonRpcRequest, RpcClient, RpcEndpoint},
    verdict::Verdict,
};
use serde::Serialize;
use std::time::Duration;
use tonic::metadata::{Ascii, MetadataValue};

mod check;
pub mod compare;
pub mod endpoint;
pub mod error_kind;
mod render;

pub use endpoint::GrpcEndpoint;
pub use error_kind::GrpcErrorKind;
pub use render::{render_human, render_json, render_markdown, write_markdown_report};

/// The JSON schema version for the gRPC readiness result. Bump on any
/// breaking change to the serialized [`GrpcReport`] shape.
pub const GRPC_SCHEMA_VERSION: u32 = 1;

/// Slot difference (gRPC vs HTTP RPC) beyond which the endpoints are flagged as
/// materially out of sync. ~150 slots is roughly a minute on mainnet.
const SLOT_DIFF_WARN: i64 = 150;

/// A per-check status, serialized as a stable lowercase string. Distinct from
/// the overall [`Verdict`]; maps to the shared presentation [`Status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

impl CheckStatus {
    /// Map to the shared human-output status vocabulary.
    pub fn display(self) -> Status {
        match self {
            Self::Pass => Status::Pass,
            Self::Warn => Status::Warn,
            Self::Fail => Status::Fail,
            Self::Skip => Status::Skip,
        }
    }
}

/// The authentication outcome, independent of whether a token was supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    /// At least one request was accepted by the server.
    Accepted,
    /// The server rejected the request with `UNAUTHENTICATED`.
    Unauthenticated,
    /// The server rejected the request with `PERMISSION_DENIED`.
    PermissionDenied,
    /// Authentication could not be determined (e.g. transport failed first).
    Unknown,
}

/// The diagnostic category a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrpcCategory {
    Target,
    Transport,
    Authentication,
    Unary,
    Stream,
    Freshness,
    CrossCheck,
}

impl GrpcCategory {
    /// The human-readable category label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Target => "Target",
            Self::Transport => "Transport",
            Self::Authentication => "Authentication",
            Self::Unary => "Unary",
            Self::Stream => "Stream",
            Self::Freshness => "Freshness",
            Self::CrossCheck => "Cross-check",
        }
    }
}

/// The result of one unary gRPC method probe.
#[derive(Debug, Clone, Serialize)]
pub struct UnaryResult {
    /// The Yellowstone method name (e.g. `GetSlot`).
    pub method: &'static str,
    /// Whether the probe passed, was skipped (unimplemented/dependency missing),
    /// or failed.
    pub status: CheckStatus,
    /// Round-trip latency in milliseconds, when the call completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u128>,
    /// A safe, secret-free one-line detail.
    pub detail: String,
    /// The classified error kind, when the probe did not pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<GrpcErrorKind>,
}

/// The result of the slot-stream readiness probe.
#[derive(Debug, Clone, Serialize)]
pub struct StreamResult {
    /// Overall stream status.
    pub status: CheckStatus,
    /// Whether the `Subscribe` stream was opened.
    pub opened: bool,
    /// Time from subscribe to the first slot update, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_event_latency_ms: Option<u128>,
    /// How many slot updates were observed within the bounded window.
    pub updates_observed: u64,
    /// The latest slot observed on the stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_slot: Option<u64>,
    /// A safe, secret-free one-line detail.
    pub detail: String,
    /// The classified error kind, when the stream did not pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<GrpcErrorKind>,
}

/// A category-level roll-up shown in the summary table.
#[derive(Debug, Clone, Serialize)]
pub struct CategoryCheck {
    pub category: GrpcCategory,
    pub status: CheckStatus,
    pub summary: String,
}

/// The full, redaction-safe result of `grpc check`. This is the serialized shape
/// emitted by `--json`; it never contains the token, raw URL credentials, or
/// request metadata.
#[derive(Debug, Clone, Serialize)]
pub struct GrpcReport {
    /// Schema version for the gRPC result shape.
    pub schema_version: u32,
    /// Overall readiness verdict (drives the process exit code).
    pub verdict: Verdict,
    /// One-line, human-readable summary of the verdict.
    pub summary: String,
    /// The redacted gRPC endpoint URL.
    pub grpc_endpoint: String,
    /// The redacted HTTP RPC endpoint, when `--rpc` was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_endpoint: Option<String>,
    /// Whether an `x-token` was supplied (never the token itself).
    pub token_provided: bool,
    /// Connection establishment latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_latency_ms: Option<u128>,
    /// The authentication outcome.
    pub authentication: AuthStatus,
    /// Per-method unary probe results.
    pub unary: Vec<UnaryResult>,
    /// The slot-stream readiness result.
    pub stream: StreamResult,
    /// The latest slot observed on the gRPC stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_slot: Option<u64>,
    /// The HTTP RPC slot, when the optional cross-check ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_slot: Option<u64>,
    /// `gRPC slot - RPC slot` (signed), when the cross-check ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_difference: Option<i64>,
    /// Category-level roll-ups.
    pub checks: Vec<CategoryCheck>,
    /// Advisory warnings about degraded behavior.
    pub warnings: Vec<String>,
    /// Concrete next-step remediation hints for warnings and failures.
    pub remediation: Vec<String>,
    /// The distinct error kinds encountered, for machine triage.
    pub error_kinds: Vec<GrpcErrorKind>,
}

/// Diagnose Yellowstone gRPC readiness. Returns a redaction-safe [`GrpcReport`].
///
/// A missing/empty token environment variable (when `--x-token-env` is set) is a
/// local configuration error and is returned as [`AppError`] before any network
/// connection is attempted.
pub async fn run_grpc_check(args: GrpcCheckArgs) -> Result<GrpcReport, AppError> {
    // Resolve the token from the environment up front. Never accept it on the
    // command line, and never put the value anywhere it could be printed.
    let token = resolve_token(args.x_token_env.as_deref())?;
    let token_provided = token.is_some();

    let timeout = Duration::from_millis(args.timeout_ms);
    let stream_duration = Duration::from_millis(args.duration_ms);

    // Validate the gRPC URL. An invalid URL is a terminal, local BAD result.
    let endpoint = match GrpcEndpoint::parse(&args.grpc) {
        Ok(endpoint) => endpoint,
        Err((kind, reason)) => {
            return Ok(invalid_target_report(
                token_provided,
                kind,
                &redact::redact_text(&reason),
            ));
        }
    };
    let grpc_redacted = endpoint.redacted();

    // Optional HTTP RPC endpoint for the slot cross-check. A bad RPC URL does not
    // fail the whole run — it just disables the cross-check with a warning.
    let (rpc_endpoint, rpc_redacted, rpc_url_warning) = match args.rpc.as_deref() {
        None => (None, None, None),
        Some(raw) => match RpcEndpoint::parse(raw) {
            Ok(endpoint) => {
                let redacted = endpoint.redacted();
                (Some(endpoint), Some(redacted), None)
            }
            Err(error) => (
                None,
                None,
                Some(format!(
                    "ignoring --rpc cross-check: {}",
                    redact::redact_text(&error.to_string())
                )),
            ),
        },
    };

    // Run the network probe (connect, auth, unary, stream).
    let probe = check::probe(&endpoint, token.as_ref(), timeout, stream_duration).await;

    // Optional cross-check: fetch the HTTP RPC slot and compare.
    let rpc_slot = match &rpc_endpoint {
        Some(endpoint) => fetch_rpc_slot(endpoint.clone(), timeout).await,
        None => None,
    };

    let report = assemble_report(
        grpc_redacted,
        rpc_redacted,
        token_provided,
        probe,
        rpc_slot,
        rpc_url_warning,
    );
    Ok(report)
}

/// Read and validate the `x-token` from the named environment variable. Returns
/// `Ok(None)` when no variable was requested. The token value is parsed into
/// gRPC metadata here and never returned as a plain string.
fn resolve_token(env_name: Option<&str>) -> Result<Option<MetadataValue<Ascii>>, AppError> {
    let Some(name) = env_name else {
        return Ok(None);
    };
    let value = std::env::var(name).map_err(|_| AppError::MissingTokenEnv {
        var: name.to_string(),
    })?;
    token_from_env_value(name, &value).map(Some)
}

/// Validate a raw `x-token` value and parse it into gRPC ASCII metadata. An empty
/// or whitespace-only value is treated as a missing token; an invalid value is
/// reported without echoing it.
fn token_from_env_value(var: &str, value: &str) -> Result<MetadataValue<Ascii>, AppError> {
    if value.trim().is_empty() {
        return Err(AppError::MissingTokenEnv {
            var: var.to_string(),
        });
    }
    MetadataValue::try_from(value).map_err(|_| AppError::InvalidTokenValue)
}

/// Fetch the current slot from an HTTP RPC endpoint for the cross-check, reusing
/// the existing resilient JSON-RPC client. Returns `None` on any failure (the
/// cross-check is best-effort and never fails the run).
async fn fetch_rpc_slot(endpoint: RpcEndpoint, timeout: Duration) -> Option<u64> {
    let client = RpcClient::new(endpoint, timeout).ok()?;
    let request = JsonRpcRequest::new(1, "getSlot");
    client.call::<u64>(&request).await.ok()?.result
}

/// Build the terminal BAD report for an unparseable/invalid gRPC URL.
fn invalid_target_report(token_provided: bool, kind: GrpcErrorKind, reason: &str) -> GrpcReport {
    let summary = format!("invalid gRPC URL: {reason}");
    GrpcReport {
        schema_version: GRPC_SCHEMA_VERSION,
        verdict: Verdict::Bad,
        summary: summary.clone(),
        grpc_endpoint: "<invalid>".to_string(),
        rpc_endpoint: None,
        token_provided,
        connect_latency_ms: None,
        authentication: AuthStatus::Unknown,
        unary: Vec::new(),
        stream: StreamResult {
            status: CheckStatus::Skip,
            opened: false,
            first_event_latency_ms: None,
            updates_observed: 0,
            latest_slot: None,
            detail: "not attempted".to_string(),
            error_kind: None,
        },
        latest_slot: None,
        rpc_slot: None,
        slot_difference: None,
        checks: vec![CategoryCheck {
            category: GrpcCategory::Target,
            status: CheckStatus::Fail,
            summary,
        }],
        warnings: Vec::new(),
        remediation: vec![
            "check the --grpc URL: it must be an http or https URL with a host".to_string(),
        ],
        error_kinds: vec![kind],
    }
}

/// Turn the raw network [`check::ProbeOutcome`] plus the optional cross-check into
/// the final report: derive category roll-ups, the verdict, warnings, and hints.
fn assemble_report(
    grpc_endpoint: String,
    rpc_endpoint: Option<String>,
    token_provided: bool,
    probe: check::ProbeOutcome,
    rpc_slot: Option<u64>,
    rpc_url_warning: Option<String>,
) -> GrpcReport {
    let mut warnings = Vec::new();
    let mut remediation = Vec::new();
    let mut error_kinds: Vec<GrpcErrorKind> = Vec::new();
    let record_kind = |kinds: &mut Vec<GrpcErrorKind>, kind: GrpcErrorKind| {
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    };

    if let Some(warning) = rpc_url_warning {
        warnings.push(warning);
    }

    let mut checks = Vec::new();

    // Transport.
    let transport_ok = probe.connect_latency_ms.is_some() && probe.transport_error.is_none();
    if let Some((kind, detail)) = &probe.transport_error {
        record_kind(&mut error_kinds, *kind);
        checks.push(CategoryCheck {
            category: GrpcCategory::Transport,
            status: CheckStatus::Fail,
            summary: detail.clone(),
        });
        remediation.push(transport_remediation(*kind));
    } else {
        let summary = if grpc_endpoint.starts_with("https") {
            "Connected over TLS (HTTP/2)"
        } else {
            "Connected (HTTP/2)"
        };
        checks.push(CategoryCheck {
            category: GrpcCategory::Transport,
            status: CheckStatus::Pass,
            summary: summary.to_string(),
        });
    }

    // Authentication.
    let auth_status = probe.auth;
    let auth_check_status = match auth_status {
        AuthStatus::Accepted => CheckStatus::Pass,
        AuthStatus::Unauthenticated | AuthStatus::PermissionDenied => CheckStatus::Fail,
        AuthStatus::Unknown => CheckStatus::Skip,
    };
    let auth_summary = match auth_status {
        AuthStatus::Accepted if token_provided => "Token accepted".to_string(),
        AuthStatus::Accepted => "Request accepted (no token required)".to_string(),
        AuthStatus::Unauthenticated => "Server rejected the request: unauthenticated".to_string(),
        AuthStatus::PermissionDenied => {
            "Server rejected the request: permission denied".to_string()
        }
        AuthStatus::Unknown => "Not determined".to_string(),
    };
    if matches!(auth_status, AuthStatus::Unauthenticated) {
        record_kind(&mut error_kinds, GrpcErrorKind::Unauthenticated);
        remediation.push(if token_provided {
            "the x-token was rejected; verify the token value and that it is still active"
                .to_string()
        } else {
            "this endpoint requires authentication; pass a token via --x-token-env".to_string()
        });
    }
    if matches!(auth_status, AuthStatus::PermissionDenied) {
        record_kind(&mut error_kinds, GrpcErrorKind::PermissionDenied);
        remediation.push(
            "the token authenticated but lacks permission for this method/subscription".to_string(),
        );
    }
    if transport_ok {
        checks.push(CategoryCheck {
            category: GrpcCategory::Authentication,
            status: auth_check_status,
            summary: auth_summary,
        });
    }

    // Unary.
    for unary in &probe.unary {
        if let Some(kind) = unary.error_kind {
            record_kind(&mut error_kinds, kind);
        }
    }
    let unary_passed = probe
        .unary
        .iter()
        .filter(|u| u.status == CheckStatus::Pass)
        .count();
    let unary_supported = probe
        .unary
        .iter()
        .filter(|u| u.status != CheckStatus::Skip)
        .count();
    let unary_failed = probe
        .unary
        .iter()
        .filter(|u| u.status == CheckStatus::Fail)
        .count();
    if transport_ok && !probe.unary.is_empty() {
        let status = if unary_failed > 0 {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };
        checks.push(CategoryCheck {
            category: GrpcCategory::Unary,
            status,
            summary: format!("{unary_passed} / {unary_supported} supported checks passed"),
        });
        if unary_failed > 0 {
            warnings.push(format!(
                "{unary_failed} unary method check(s) failed; the endpoint may have limited unary support"
            ));
        }
    }

    // Stream.
    if let Some(kind) = probe.stream.error_kind {
        record_kind(&mut error_kinds, kind);
        remediation.push(stream_remediation(kind));
    }
    if transport_ok {
        checks.push(CategoryCheck {
            category: GrpcCategory::Stream,
            status: probe.stream.status,
            summary: probe.stream.detail.clone(),
        });
    }

    // Freshness (derived from the stream).
    if transport_ok && probe.stream.status == CheckStatus::Pass {
        checks.push(CategoryCheck {
            category: GrpcCategory::Freshness,
            status: CheckStatus::Pass,
            summary: "Slot stream is active".to_string(),
        });
    }

    // Cross-check.
    let slot_difference = match (probe.latest_slot, rpc_slot) {
        (Some(grpc), Some(rpc)) => Some(grpc as i64 - rpc as i64),
        _ => None,
    };
    if rpc_endpoint.is_some() {
        match (rpc_slot, slot_difference) {
            (Some(_), Some(diff)) => {
                let abs = diff.unsigned_abs();
                if abs as i64 > SLOT_DIFF_WARN {
                    checks.push(CategoryCheck {
                        category: GrpcCategory::CrossCheck,
                        status: CheckStatus::Warn,
                        summary: format!("gRPC and HTTP RPC slots differ by {abs} slots"),
                    });
                    warnings.push(format!(
                        "gRPC and HTTP RPC slots differ by {abs} slots; one source may be lagging or they may be different networks"
                    ));
                    remediation.push(
                        "a large slot gap can mean a lagging endpoint or mismatched networks; verify both point at the same cluster"
                            .to_string(),
                    );
                } else {
                    checks.push(CategoryCheck {
                        category: GrpcCategory::CrossCheck,
                        status: CheckStatus::Pass,
                        summary: format!("gRPC and HTTP RPC slots agree within {abs} slots"),
                    });
                }
            }
            (Some(_), None) => {
                // RPC slot fetched but the gRPC stream produced no slot to compare.
                checks.push(CategoryCheck {
                    category: GrpcCategory::CrossCheck,
                    status: CheckStatus::Skip,
                    summary: "no gRPC slot observed to compare".to_string(),
                });
            }
            (None, _) => {
                checks.push(CategoryCheck {
                    category: GrpcCategory::CrossCheck,
                    status: CheckStatus::Skip,
                    summary: "HTTP RPC slot unavailable".to_string(),
                });
                warnings.push(
                    "--rpc cross-check skipped: could not fetch the HTTP RPC slot".to_string(),
                );
            }
        }
    }

    let (verdict, summary) = classify_verdict(
        transport_ok,
        auth_status,
        &probe.stream,
        unary_failed,
        slot_difference,
    );

    GrpcReport {
        schema_version: GRPC_SCHEMA_VERSION,
        verdict,
        summary,
        grpc_endpoint,
        rpc_endpoint,
        token_provided,
        connect_latency_ms: probe.connect_latency_ms,
        authentication: auth_status,
        unary: probe.unary,
        stream: probe.stream,
        latest_slot: probe.latest_slot,
        rpc_slot,
        slot_difference,
        checks,
        warnings,
        remediation,
        error_kinds,
    }
}

/// Compute the overall verdict and summary from the layered results. Transport,
/// authentication, and the slot stream are the load-bearing signals; unary gaps
/// and a large slot gap degrade to WARNING rather than BAD.
fn classify_verdict(
    transport_ok: bool,
    auth: AuthStatus,
    stream: &StreamResult,
    unary_failed: usize,
    slot_difference: Option<i64>,
) -> (Verdict, String) {
    if !transport_ok {
        return (
            Verdict::Bad,
            "could not connect to the gRPC endpoint".to_string(),
        );
    }
    if matches!(
        auth,
        AuthStatus::Unauthenticated | AuthStatus::PermissionDenied
    ) {
        return (
            Verdict::Bad,
            "gRPC endpoint rejected authentication".to_string(),
        );
    }
    if stream.status == CheckStatus::Fail {
        return (
            Verdict::Bad,
            "gRPC endpoint is reachable but the slot stream is not ready".to_string(),
        );
    }

    let slot_gap_large = slot_difference
        .map(i64::unsigned_abs)
        .is_some_and(|abs| abs as i64 > SLOT_DIFF_WARN);
    if unary_failed > 0 || slot_gap_large {
        return (
            Verdict::Warning,
            "gRPC endpoint is streaming, with some degraded checks".to_string(),
        );
    }

    (
        Verdict::Good,
        "Yellowstone gRPC endpoint is ready".to_string(),
    )
}

fn transport_remediation(kind: GrpcErrorKind) -> String {
    match kind {
        GrpcErrorKind::DnsError => {
            "the host could not be resolved; check the gRPC hostname".to_string()
        }
        GrpcErrorKind::TlsError => {
            "TLS handshake failed; check the scheme (https), port, and certificate".to_string()
        }
        GrpcErrorKind::Timeout => {
            "the connection timed out; check the host/port and raise --timeout-ms".to_string()
        }
        _ => "the endpoint refused or dropped the connection; verify the host and port".to_string(),
    }
}

fn stream_remediation(kind: GrpcErrorKind) -> String {
    match kind {
        GrpcErrorKind::NoFirstEvent => {
            "the stream opened but sent no slot update before the deadline; verify the endpoint, the x-token's subscription permissions, and the upstream Geyser/validator health".to_string()
        }
        GrpcErrorKind::StreamClosed => {
            "the stream closed before a slot update; inspect the Yellowstone/Geyser server logs and the token's permissions".to_string()
        }
        GrpcErrorKind::Unimplemented => {
            "this endpoint does not implement the Subscribe stream; confirm it is a Yellowstone gRPC endpoint".to_string()
        }
        _ => "the slot stream did not become ready; compare against the HTTP RPC slot with --rpc and check server logs".to_string(),
    }
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;

    #[test]
    fn missing_token_env_is_a_config_error() {
        // Use a name that is overwhelmingly unlikely to be set.
        let error = resolve_token(Some("SOL_DOCTOR_DEFINITELY_UNSET_TOKEN_XYZ")).unwrap_err();
        assert!(matches!(error, AppError::MissingTokenEnv { .. }));
        // The error message names the variable, never a value.
        assert!(
            error
                .to_string()
                .contains("SOL_DOCTOR_DEFINITELY_UNSET_TOKEN_XYZ")
        );
    }

    #[test]
    fn no_token_env_resolves_to_none() {
        assert!(resolve_token(None).unwrap().is_none());
    }

    #[test]
    fn present_token_value_resolves_to_metadata() {
        assert!(token_from_env_value("X_TOKEN", "secret-token-value").is_ok());
    }

    #[test]
    fn empty_token_value_is_a_config_error() {
        assert!(matches!(
            token_from_env_value("X_TOKEN", "   "),
            Err(AppError::MissingTokenEnv { .. })
        ));
    }

    fn ready_stream() -> StreamResult {
        StreamResult {
            status: CheckStatus::Pass,
            opened: true,
            first_event_latency_ms: Some(120),
            updates_observed: 4,
            latest_slot: Some(424_000_123),
            detail: "first slot update in 120 ms".to_string(),
            error_kind: None,
        }
    }

    #[test]
    fn good_verdict_when_all_layers_pass() {
        let (verdict, _) =
            classify_verdict(true, AuthStatus::Accepted, &ready_stream(), 0, Some(3));
        assert_eq!(verdict, Verdict::Good);
    }

    #[test]
    fn bad_verdict_on_transport_failure() {
        let (verdict, _) = classify_verdict(false, AuthStatus::Unknown, &ready_stream(), 0, None);
        assert_eq!(verdict, Verdict::Bad);
    }

    #[test]
    fn bad_verdict_on_auth_failure() {
        let (verdict, _) =
            classify_verdict(true, AuthStatus::Unauthenticated, &ready_stream(), 0, None);
        assert_eq!(verdict, Verdict::Bad);
    }

    #[test]
    fn bad_verdict_when_stream_fails() {
        let mut stream = ready_stream();
        stream.status = CheckStatus::Fail;
        stream.error_kind = Some(GrpcErrorKind::NoFirstEvent);
        let (verdict, _) = classify_verdict(true, AuthStatus::Accepted, &stream, 0, None);
        assert_eq!(verdict, Verdict::Bad);
    }

    #[test]
    fn warning_when_unary_degraded_but_stream_ok() {
        let (verdict, _) = classify_verdict(true, AuthStatus::Accepted, &ready_stream(), 2, None);
        assert_eq!(verdict, Verdict::Warning);
    }

    #[test]
    fn warning_on_large_slot_gap() {
        let (verdict, _) =
            classify_verdict(true, AuthStatus::Accepted, &ready_stream(), 0, Some(5_000));
        assert_eq!(verdict, Verdict::Warning);
    }

    #[test]
    fn invalid_target_report_is_bad_and_redacted() {
        let report = invalid_target_report(false, GrpcErrorKind::InvalidGrpcUrl, "missing host");
        assert_eq!(report.verdict, Verdict::Bad);
        assert_eq!(report.grpc_endpoint, "<invalid>");
        assert!(report.error_kinds.contains(&GrpcErrorKind::InvalidGrpcUrl));
    }

    fn healthy_probe(latest: u64) -> check::ProbeOutcome {
        check::ProbeOutcome {
            connect_latency_ms: Some(10),
            transport_error: None,
            auth: AuthStatus::Accepted,
            unary: vec![UnaryResult {
                method: "Ping",
                status: CheckStatus::Pass,
                latency_ms: Some(1),
                detail: "pong".to_string(),
                error_kind: None,
            }],
            stream: ready_stream(),
            latest_slot: Some(latest),
        }
    }

    #[test]
    fn cross_check_small_difference_is_good() {
        let report = assemble_report(
            "https://grpc.example.com/".to_string(),
            Some("https://rpc.example.com/".to_string()),
            false,
            healthy_probe(1_000),
            Some(1_005),
            None,
        );
        assert_eq!(report.verdict, Verdict::Good);
        assert_eq!(report.slot_difference, Some(-5));
        let cross = report
            .checks
            .iter()
            .find(|c| c.category == GrpcCategory::CrossCheck)
            .unwrap();
        assert_eq!(cross.status, CheckStatus::Pass);
    }

    #[test]
    fn cross_check_large_difference_warns() {
        let report = assemble_report(
            "https://grpc.example.com/".to_string(),
            Some("https://rpc.example.com/".to_string()),
            false,
            healthy_probe(1_000),
            Some(1_000_000),
            None,
        );
        assert_eq!(report.verdict, Verdict::Warning);
        assert_eq!(report.slot_difference, Some(-999_000));
        let cross = report
            .checks
            .iter()
            .find(|c| c.category == GrpcCategory::CrossCheck)
            .unwrap();
        assert_eq!(cross.status, CheckStatus::Warn);
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn cross_check_skipped_when_rpc_slot_unavailable() {
        let report = assemble_report(
            "https://grpc.example.com/".to_string(),
            Some("https://rpc.example.com/".to_string()),
            false,
            healthy_probe(1_000),
            None,
            None,
        );
        let cross = report
            .checks
            .iter()
            .find(|c| c.category == GrpcCategory::CrossCheck)
            .unwrap();
        assert_eq!(cross.status, CheckStatus::Skip);
        assert!(report.slot_difference.is_none());
    }

    fn failed_stream(kind: GrpcErrorKind, detail: &str) -> StreamResult {
        StreamResult {
            status: CheckStatus::Fail,
            opened: true,
            first_event_latency_ms: None,
            updates_observed: 0,
            latest_slot: None,
            detail: detail.to_string(),
            error_kind: Some(kind),
        }
    }

    #[test]
    fn assemble_transport_failure_is_bad_with_remediation() {
        let probe = check::ProbeOutcome {
            connect_latency_ms: None,
            transport_error: Some((GrpcErrorKind::TlsError, "TLS handshake failed".to_string())),
            auth: AuthStatus::Unknown,
            unary: Vec::new(),
            stream: failed_stream(GrpcErrorKind::StreamClosed, "n/a"),
            latest_slot: None,
        };
        let report = assemble_report(
            "https://grpc.example.com/".to_string(),
            None,
            false,
            probe,
            None,
            None,
        );
        assert_eq!(report.verdict, Verdict::Bad);
        assert!(report.error_kinds.contains(&GrpcErrorKind::TlsError));
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.category == GrpcCategory::Transport && c.status == CheckStatus::Fail)
        );
        assert!(!report.remediation.is_empty());
    }

    #[test]
    fn assemble_auth_failure_is_bad() {
        let mut probe = healthy_probe(1_000);
        probe.auth = AuthStatus::Unauthenticated;
        let report = assemble_report(
            "https://grpc.example.com/".to_string(),
            None,
            true,
            probe,
            None,
            None,
        );
        assert_eq!(report.verdict, Verdict::Bad);
        assert!(
            report.checks.iter().any(
                |c| c.category == GrpcCategory::Authentication && c.status == CheckStatus::Fail
            )
        );
    }

    #[test]
    fn assemble_stream_failure_is_bad() {
        let mut probe = healthy_probe(1_000);
        probe.stream = failed_stream(
            GrpcErrorKind::NoFirstEvent,
            "no slot update before deadline",
        );
        probe.latest_slot = None;
        let report = assemble_report(
            "http://127.0.0.1:9/".to_string(),
            None,
            false,
            probe,
            None,
            None,
        );
        assert_eq!(report.verdict, Verdict::Bad);
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.category == GrpcCategory::Stream && c.status == CheckStatus::Fail)
        );
    }

    #[test]
    fn assemble_unary_failure_degrades_to_warning() {
        let mut probe = healthy_probe(1_000);
        probe.unary.push(UnaryResult {
            method: "GetSlot",
            status: CheckStatus::Fail,
            latency_ms: Some(5),
            detail: "down".to_string(),
            error_kind: Some(GrpcErrorKind::Unavailable),
        });
        let report = assemble_report(
            "http://127.0.0.1:9/".to_string(),
            None,
            false,
            probe,
            None,
            None,
        );
        assert_eq!(report.verdict, Verdict::Warning);
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.category == GrpcCategory::Unary && c.status == CheckStatus::Warn)
        );
        assert!(!report.warnings.is_empty());
    }
}
