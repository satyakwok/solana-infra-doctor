//! Integration tests for `grpc check` against an in-process mock Yellowstone
//! gRPC server. No public endpoints are contacted; every scenario is
//! deterministic and bounded.

use futures_util::{Stream, StreamExt};
use solana_infra_doctor::cli::GrpcCheckArgs;
use solana_infra_doctor::color::Palette;
use solana_infra_doctor::grpc::{self, AuthStatus, CheckStatus, GrpcCategory, GrpcErrorKind};
use solana_infra_doctor::verdict::Verdict;
use std::pin::Pin;
use std::time::Duration;
use tonic::{Request, Response, Status, Streaming};
use yellowstone_grpc_proto::geyser::{
    geyser_server::{Geyser, GeyserServer},
    subscribe_update::UpdateOneof,
    GetBlockHeightRequest, GetBlockHeightResponse, GetLatestBlockhashRequest,
    GetLatestBlockhashResponse, GetSlotRequest, GetSlotResponse, GetVersionRequest,
    GetVersionResponse, IsBlockhashValidRequest, IsBlockhashValidResponse, PingRequest,
    PongResponse, SubscribeDeshredRequest, SubscribeReplayInfoRequest, SubscribeReplayInfoResponse,
    SubscribeRequest, SubscribeUpdate, SubscribeUpdateDeshred, SubscribeUpdatePong,
    SubscribeUpdateSlot,
};

#[derive(Clone, Copy)]
enum StreamBehavior {
    /// Emit a steady sequence of slot updates.
    Healthy,
    /// Open the stream but never emit a slot update.
    NoEvent,
    /// Close the stream immediately with no updates.
    ClosesImmediately,
    /// Emit a non-slot (pong) update, then go quiet — never a slot update.
    UnexpectedOnly,
    /// Open, then yield a stream error before any slot update.
    ErrorsAfterOpen,
    /// Reject the subscribe call itself with UNIMPLEMENTED.
    Unimplemented,
}

#[derive(Clone, Copy)]
enum AuthBehavior {
    /// Accept anonymous requests.
    Open,
    /// Reject any request without an `x-token` as UNAUTHENTICATED.
    RequireToken,
    /// Reject every request as UNAUTHENTICATED, even with a token.
    AlwaysUnauthenticated,
    /// Reject every request as PERMISSION_DENIED.
    PermissionDenied,
}

#[derive(Clone, Copy)]
enum UnaryBehavior {
    Healthy,
    /// `GetSlot` returns UNAVAILABLE; the rest succeed.
    GetSlotUnavailable,
    /// A mix of failures: `GetVersion` INTERNAL (empty message), `GetBlockHeight`
    /// UNIMPLEMENTED (skip), `IsBlockhashValid` UNAVAILABLE.
    Degraded,
}

#[derive(Clone, Copy)]
struct MockGeyser {
    stream: StreamBehavior,
    auth: AuthBehavior,
    unary: UnaryBehavior,
}

impl MockGeyser {
    fn check_auth<T>(&self, request: &Request<T>) -> Result<(), Status> {
        match self.auth {
            AuthBehavior::Open => Ok(()),
            AuthBehavior::RequireToken => {
                if request.metadata().get("x-token").is_some() {
                    Ok(())
                } else {
                    Err(Status::unauthenticated("missing x-token"))
                }
            }
            AuthBehavior::AlwaysUnauthenticated => Err(Status::unauthenticated("denied")),
            AuthBehavior::PermissionDenied => Err(Status::permission_denied("no access")),
        }
    }
}

fn slot_update(slot: u64) -> SubscribeUpdate {
    SubscribeUpdate {
        update_oneof: Some(UpdateOneof::Slot(SubscribeUpdateSlot {
            slot,
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn pong_update() -> SubscribeUpdate {
    SubscribeUpdate {
        update_oneof: Some(UpdateOneof::Pong(SubscribeUpdatePong { id: 1 })),
        ..Default::default()
    }
}

type UpdateStream = Pin<Box<dyn Stream<Item = Result<SubscribeUpdate, Status>> + Send>>;
type DeshredStream = Pin<Box<dyn Stream<Item = Result<SubscribeUpdateDeshred, Status>> + Send>>;

#[tonic::async_trait]
impl Geyser for MockGeyser {
    type SubscribeStream = UpdateStream;
    type SubscribeDeshredStream = DeshredStream;

    async fn subscribe(
        &self,
        request: Request<Streaming<SubscribeRequest>>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        self.check_auth(&request)?;
        if matches!(self.stream, StreamBehavior::Unimplemented) {
            return Err(Status::unimplemented("subscribe not supported"));
        }
        let stream: UpdateStream = match self.stream {
            StreamBehavior::Healthy => {
                Box::pin(futures_util::stream::unfold(1_000u64, |slot| async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Some((Ok(slot_update(slot)), slot + 1))
                }))
            }
            StreamBehavior::NoEvent => Box::pin(futures_util::stream::pending()),
            StreamBehavior::ClosesImmediately => Box::pin(futures_util::stream::empty()),
            StreamBehavior::UnexpectedOnly => Box::pin(
                futures_util::stream::once(async { Ok(pong_update()) })
                    .chain(futures_util::stream::pending()),
            ),
            StreamBehavior::ErrorsAfterOpen => Box::pin(futures_util::stream::once(async {
                Err(Status::internal("boom"))
            })),
            StreamBehavior::Unimplemented => unreachable!("handled above"),
        };
        Ok(Response::new(stream))
    }

    async fn subscribe_deshred(
        &self,
        _request: Request<Streaming<SubscribeDeshredRequest>>,
    ) -> Result<Response<Self::SubscribeDeshredStream>, Status> {
        Err(Status::unimplemented("not supported by the mock"))
    }

    async fn subscribe_replay_info(
        &self,
        _request: Request<SubscribeReplayInfoRequest>,
    ) -> Result<Response<SubscribeReplayInfoResponse>, Status> {
        Err(Status::unimplemented("not supported by the mock"))
    }

    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PongResponse>, Status> {
        self.check_auth(&request)?;
        Ok(Response::new(PongResponse {
            count: request.into_inner().count,
        }))
    }

    async fn get_version(
        &self,
        request: Request<GetVersionRequest>,
    ) -> Result<Response<GetVersionResponse>, Status> {
        self.check_auth(&request)?;
        if matches!(self.unary, UnaryBehavior::Degraded) {
            // Empty status message exercises the error-detail fallback.
            return Err(Status::internal(""));
        }
        Ok(Response::new(GetVersionResponse {
            version: "mock-yellowstone 1.0.0".to_string(),
        }))
    }

    async fn get_slot(
        &self,
        request: Request<GetSlotRequest>,
    ) -> Result<Response<GetSlotResponse>, Status> {
        self.check_auth(&request)?;
        if matches!(self.unary, UnaryBehavior::GetSlotUnavailable) {
            return Err(Status::unavailable("slot source draining"));
        }
        Ok(Response::new(GetSlotResponse { slot: 1_000 }))
    }

    async fn get_block_height(
        &self,
        request: Request<GetBlockHeightRequest>,
    ) -> Result<Response<GetBlockHeightResponse>, Status> {
        self.check_auth(&request)?;
        if matches!(self.unary, UnaryBehavior::Degraded) {
            return Err(Status::unimplemented("not supported"));
        }
        Ok(Response::new(GetBlockHeightResponse { block_height: 900 }))
    }

    async fn get_latest_blockhash(
        &self,
        request: Request<GetLatestBlockhashRequest>,
    ) -> Result<Response<GetLatestBlockhashResponse>, Status> {
        self.check_auth(&request)?;
        Ok(Response::new(GetLatestBlockhashResponse {
            slot: 1_000,
            blockhash: "MockBlockhash1111111111111111111111111111111".to_string(),
            last_valid_block_height: 1_150,
        }))
    }

    async fn is_blockhash_valid(
        &self,
        request: Request<IsBlockhashValidRequest>,
    ) -> Result<Response<IsBlockhashValidResponse>, Status> {
        self.check_auth(&request)?;
        if matches!(self.unary, UnaryBehavior::Degraded) {
            return Err(Status::unavailable("validator unavailable"));
        }
        Ok(Response::new(IsBlockhashValidResponse {
            slot: 1_000,
            valid: true,
        }))
    }
}

/// Start the mock server on an ephemeral port and return its `http://` URL.
async fn start_mock(mock: MockGeyser) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(GeyserServer::new(mock))
            .serve_with_incoming(incoming)
            .await
            .ok();
    });
    // Give the server a moment to begin accepting connections.
    tokio::time::sleep(Duration::from_millis(80)).await;
    format!("http://{addr}")
}

fn args(grpc: String) -> GrpcCheckArgs {
    GrpcCheckArgs {
        grpc,
        x_token_env: None,
        rpc: None,
        json: false,
        report: None,
        timeout_ms: 2_000,
        // Short observation window keeps the negative cases fast.
        duration_ms: 700,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthy_endpoint_is_good() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Good);
    assert_eq!(report.stream.status, CheckStatus::Pass);
    assert!(report.stream.first_event_latency_ms.is_some());
    assert!(report.latest_slot.is_some());
    assert_eq!(report.authentication, AuthStatus::Accepted);
    // All six unary checks should pass against the healthy mock.
    assert_eq!(
        report
            .unary
            .iter()
            .filter(|u| u.status == CheckStatus::Pass)
            .count(),
        6
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unauthenticated_endpoint_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::RequireToken,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    // No token provided → server rejects with UNAUTHENTICATED.
    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.authentication, AuthStatus::Unauthenticated);
    assert!(report.error_kinds.contains(&GrpcErrorKind::Unauthenticated));

    // Render the BAD path in every format (covers the failure-summary branches).
    let concise = grpc::render_human(&report, Palette::new(false), false);
    assert!(concise.contains("BAD"));
    let _ = grpc::render_human(&report, Palette::new(false), true);
    let _ = grpc::render_markdown(&report);
    let _ = grpc::render_json(&report).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_unlocks_authenticated_endpoint() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::RequireToken,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let env_name = "SOL_DOCTOR_TEST_TOKEN_UNLOCK";
    let secret = "supersecret-token-value-xyz";
    std::env::set_var(env_name, secret);

    let mut a = args(url);
    a.x_token_env = Some(env_name.to_string());
    let report = grpc::run_grpc_check(a).await.unwrap();
    std::env::remove_var(env_name);

    assert_eq!(report.verdict, Verdict::Good);
    assert_eq!(report.authentication, AuthStatus::Accepted);
    assert!(report.token_provided);

    // The token must not appear in any rendered form.
    let json = grpc::render_json(&report).unwrap();
    let markdown = grpc::render_markdown(&report);
    let human = grpc::render_human(
        &report,
        solana_infra_doctor::color::Palette::new(false),
        true,
    );
    for rendered in [&json, &markdown, &human] {
        assert!(
            !rendered.contains(secret),
            "token leaked into rendered output"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unary_failure_degrades_to_warning() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::GetSlotUnavailable,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    // Stream is healthy, so this is a degraded WARNING, not BAD.
    assert_eq!(report.verdict, Verdict::Warning);
    assert_eq!(report.stream.status, CheckStatus::Pass);
    let get_slot = report.unary.iter().find(|u| u.method == "GetSlot").unwrap();
    assert_eq!(get_slot.status, CheckStatus::Fail);
    assert_eq!(get_slot.error_kind, Some(GrpcErrorKind::Unavailable));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_with_no_first_event_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::NoEvent,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.stream.status, CheckStatus::Fail);
    assert_eq!(report.stream.error_kind, Some(GrpcErrorKind::NoFirstEvent));

    // Render a stream-failure report with no cross-check and no latest slot.
    let _ = grpc::render_human(&report, Palette::new(false), true);
    let md = grpc::render_markdown(&report);
    assert!(md.contains("## Slot stream"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_that_closes_immediately_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::ClosesImmediately,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.stream.status, CheckStatus::Fail);
    assert_eq!(report.stream.error_kind, Some(GrpcErrorKind::StreamClosed));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unexpected_only_updates_do_not_satisfy_first_event() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::UnexpectedOnly,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();

    // A pong (non-slot) update is ignored; the slot stream never becomes ready.
    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.stream.status, CheckStatus::Fail);
    assert_eq!(report.stream.error_kind, Some(GrpcErrorKind::NoFirstEvent));
    assert!(report.latest_slot.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_refused_is_bad_without_panic() {
    // Nothing is listening on this port → transport failure, not a panic.
    let report = grpc::run_grpc_check(args("http://127.0.0.1:1".to_string()))
        .await
        .unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.connect_latency_ms.is_none());
}

#[tokio::test]
async fn invalid_grpc_url_is_bad() {
    let report = grpc::run_grpc_check(args("not-a-url".to_string()))
        .await
        .unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.error_kinds.contains(&GrpcErrorKind::InvalidGrpcUrl));
}

/// A one-shot HTTP server that answers a single `getSlot` JSON-RPC request with
/// the given slot, for the `--rpc` cross-check. Returns the `http://` URL.
fn start_http_slot_mock(slot: u64) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut chunk = [0u8; 1024];
            // Read until headers complete (the small body fits the first reads).
            let _ = stream.read(&mut chunk);
            let body = format!(r#"{{"jsonrpc":"2.0","id":1,"result":{slot}}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    url
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_check_with_close_slots_is_good_and_renders() {
    let grpc_url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;
    // The healthy mock streams slots starting at 1000; a near RPC slot agrees.
    let rpc_url = start_http_slot_mock(1_001);

    let mut a = args(grpc_url);
    a.rpc = Some(rpc_url);
    let report = grpc::run_grpc_check(a).await.unwrap();

    assert_eq!(report.verdict, Verdict::Good);
    assert!(report.rpc_slot.is_some());
    assert!(report.slot_difference.is_some());

    // Exercise every renderer (this is the coverage-bearing test for render.rs).
    let _concise = grpc::render_human(&report, Palette::new(false), false);
    let verbose = grpc::render_human(&report, Palette::new(true), true);
    assert!(verbose.contains("Cross-check"));
    let json = grpc::render_json(&report).unwrap();
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"slot_difference\""));
    let markdown = grpc::render_markdown(&report);
    assert!(markdown.contains("## HTTP RPC cross-check"));

    // Write the Markdown report to a temp file and confirm it lands on disk.
    let path = std::env::temp_dir().join(format!("sol-doctor-grpc-{}.md", std::process::id()));
    grpc::write_markdown_report(&report, &path).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("point-in-time diagnostic snapshot"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_check_with_far_slots_warns() {
    let grpc_url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;
    let rpc_url = start_http_slot_mock(9_000_000);

    let mut a = args(grpc_url);
    a.rpc = Some(rpc_url);
    let report = grpc::run_grpc_check(a).await.unwrap();

    assert_eq!(report.verdict, Verdict::Warning);
    assert!(report.slot_difference.unwrap().unsigned_abs() > 150);
    assert!(report
        .checks
        .iter()
        .any(|c| matches!(c.category, GrpcCategory::CrossCheck) && c.status == CheckStatus::Warn));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_rpc_url_is_ignored_with_warning() {
    let grpc_url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let mut a = args(grpc_url);
    a.rpc = Some("not a url".to_string());
    let report = grpc::run_grpc_check(a).await.unwrap();

    // A bad cross-check URL does not fail the run.
    assert_eq!(report.verdict, Verdict::Good);
    assert!(report.rpc_slot.is_none());
    assert!(report.warnings.iter().any(|w| w.contains("cross-check")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tls_handshake_against_plaintext_is_transport_failure() {
    // A plaintext h2 server reached over `https` fails the TLS handshake — this
    // exercises the TLS config path and transport-error classification.
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;
    let https = url.replace("http://", "https://");

    let mut a = args(https);
    a.timeout_ms = 1_500;
    let report = grpc::run_grpc_check(a).await.unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.connect_latency_ms.is_none());
    assert!(report
        .checks
        .iter()
        .any(|c| matches!(c.category, GrpcCategory::Transport) && c.status == CheckStatus::Fail));
    // Render the transport-failure report (covers the failure remediation path).
    let _ = grpc::render_human(&report, Palette::new(false), true);
    let _ = grpc::render_markdown(&report);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_provided_but_rejected_is_unauthenticated() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::AlwaysUnauthenticated,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let env_name = "SOL_DOCTOR_TEST_TOKEN_REJECTED";
    std::env::set_var(env_name, "wrong-token");
    let mut a = args(url);
    a.x_token_env = Some(env_name.to_string());
    let report = grpc::run_grpc_check(a).await.unwrap();
    std::env::remove_var(env_name);

    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.authentication, AuthStatus::Unauthenticated);
    assert!(report.token_provided);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permission_denied_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::PermissionDenied,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.authentication, AuthStatus::PermissionDenied);
    assert!(report
        .error_kinds
        .contains(&GrpcErrorKind::PermissionDenied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn degraded_unary_methods_warn_but_stream_is_ready() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Degraded,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();
    assert_eq!(report.verdict, Verdict::Warning);
    // GetBlockHeight UNIMPLEMENTED is a SKIP, not a failure.
    let block_height = report
        .unary
        .iter()
        .find(|u| u.method == "GetBlockHeight")
        .unwrap();
    assert_eq!(block_height.status, CheckStatus::Skip);
    // Render verbose to exercise per-method detail with error kinds.
    let verbose = grpc::render_human(&report, Palette::new(false), true);
    assert!(verbose.contains("GetVersion"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_unimplemented_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::Unimplemented,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.stream.status, CheckStatus::Fail);
    assert_eq!(report.stream.error_kind, Some(GrpcErrorKind::Unimplemented));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_error_after_open_is_bad() {
    let url = start_mock(MockGeyser {
        stream: StreamBehavior::ErrorsAfterOpen,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let report = grpc::run_grpc_check(args(url)).await.unwrap();
    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.stream.status, CheckStatus::Fail);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_check_when_grpc_has_no_slot() {
    // gRPC stream produces no slot, but the HTTP RPC slot is available.
    let grpc_url = start_mock(MockGeyser {
        stream: StreamBehavior::NoEvent,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;
    let rpc_url = start_http_slot_mock(424_000_000);

    let mut a = args(grpc_url);
    a.rpc = Some(rpc_url);
    let report = grpc::run_grpc_check(a).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad); // no first event
    assert!(report.rpc_slot.is_some());
    assert!(report.slot_difference.is_none());
    assert!(report
        .checks
        .iter()
        .any(|c| matches!(c.category, GrpcCategory::CrossCheck) && c.status == CheckStatus::Skip));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_check_when_rpc_slot_fetch_fails() {
    // Point --rpc at the gRPC (h2-only) port: it is a valid URL but not a
    // JSON-RPC server, so the slot fetch fails and the cross-check is skipped.
    let grpc_url = start_mock(MockGeyser {
        stream: StreamBehavior::Healthy,
        auth: AuthBehavior::Open,
        unary: UnaryBehavior::Healthy,
    })
    .await;

    let mut a = args(grpc_url.clone());
    a.rpc = Some(grpc_url);
    a.timeout_ms = 1_500;
    let report = grpc::run_grpc_check(a).await.unwrap();

    assert_eq!(report.verdict, Verdict::Good);
    assert!(report.rpc_slot.is_none());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("could not fetch the HTTP RPC slot")));
}

#[tokio::test]
async fn missing_token_env_returns_config_error() {
    let mut a = args("https://example-yellowstone-endpoint".to_string());
    a.x_token_env = Some("SOL_DOCTOR_DEFINITELY_UNSET_GRPC_TOKEN".to_string());
    let result = grpc::run_grpc_check(a).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_url_report_renders() {
    let report = grpc::run_grpc_check(args("not-a-url".to_string()))
        .await
        .unwrap();
    // Rendering the target-only BAD report covers the Target category label.
    let human = grpc::render_human(&report, Palette::new(false), false);
    assert!(human.contains("Target"));
    let md = grpc::render_markdown(&report);
    assert!(md.contains("Yellowstone gRPC Readiness Report"));
}

#[test]
fn grpc_endpoint_parsing_and_redaction() {
    use solana_infra_doctor::grpc::GrpcEndpoint;

    let tls = GrpcEndpoint::parse("https://grpc.example.com:443").unwrap();
    assert!(tls.is_tls());
    assert_eq!(tls.domain(), "grpc.example.com");
    assert!(tls.connect_target().starts_with("https://grpc.example.com"));

    let plain = GrpcEndpoint::parse("http://127.0.0.1:10000").unwrap();
    assert!(!plain.is_tls());

    // Unsupported scheme and unparseable input are both invalid-URL errors.
    assert_eq!(
        GrpcEndpoint::parse("grpc://x").unwrap_err().0,
        GrpcErrorKind::InvalidGrpcUrl
    );
    assert_eq!(
        GrpcEndpoint::parse("::::").unwrap_err().0,
        GrpcErrorKind::InvalidGrpcUrl
    );

    // Credentials and path tokens are redacted in both display and Debug.
    let secret =
        GrpcEndpoint::parse("https://user:topsecret@grpc.example.com/v2/SUPERSECRETTOKENVALUE01")
            .unwrap();
    let redacted = secret.redacted();
    assert!(!redacted.contains("topsecret"));
    assert!(!redacted.contains("SUPERSECRETTOKENVALUE01"));
    assert!(format!("{secret:?}").contains("***"));
}

#[test]
fn grpc_error_kinds_are_stable_and_mapped() {
    use solana_infra_doctor::grpc::GrpcErrorKind::*;
    use tonic::Code;

    let all = [
        InvalidGrpcUrl,
        DnsError,
        ConnectError,
        TlsError,
        Timeout,
        Unauthenticated,
        PermissionDenied,
        Unavailable,
        ResourceExhausted,
        Unimplemented,
        DeadlineExceeded,
        InvalidArgument,
        Internal,
        MalformedResponse,
        StreamClosed,
        StreamStalled,
        NoFirstEvent,
        UnknownError,
    ];
    for kind in all {
        assert!(!kind.as_str().is_empty());
        assert_eq!(kind.to_string(), kind.as_str());
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.as_str()));
    }

    assert_eq!(
        GrpcErrorKind::from_code(Code::Unauthenticated),
        Unauthenticated
    );
    assert_eq!(
        GrpcErrorKind::from_code(Code::PermissionDenied),
        PermissionDenied
    );
    assert_eq!(GrpcErrorKind::from_code(Code::Unavailable), Unavailable);
    assert_eq!(
        GrpcErrorKind::from_code(Code::ResourceExhausted),
        ResourceExhausted
    );
    assert_eq!(GrpcErrorKind::from_code(Code::Unimplemented), Unimplemented);
    assert_eq!(
        GrpcErrorKind::from_code(Code::DeadlineExceeded),
        DeadlineExceeded
    );
    assert_eq!(
        GrpcErrorKind::from_code(Code::InvalidArgument),
        InvalidArgument
    );
    assert_eq!(GrpcErrorKind::from_code(Code::Internal), Internal);
    assert_eq!(GrpcErrorKind::from_code(Code::DataLoss), UnknownError);
    assert!(Unauthenticated.is_auth_failure());
    assert!(PermissionDenied.is_auth_failure());
    assert!(!Unavailable.is_auth_failure());
}
