use solana_infra_doctor::{
    checks::{
        calculate_verdict, run_check, verdict::summarize, CheckCategory, CheckStatus, ErrorKind,
        RpcCheck,
    },
    cli::{CheckArgs, CompareArgs, CompareProfile, WsArgs},
    color::{ColorChoice, Palette},
    compare::{
        build_compare_report, render_human as render_compare_human,
        render_json as render_compare_json, render_markdown, run_compare, score_endpoint, slot_lag,
        write_markdown_report, CompareEndpoint, CompareProfileSummary, CompareReport,
    },
    latency::{average_latency_ms, Latency, LatencyStats},
    redact::{redact_text, redact_url},
    report::{render_human, render_json},
    rpc::{
        BlockhashValidResponse, JsonRpcRequest, LatestBlockhashResponse, PerformanceSample,
        RpcEndpoint,
    },
    verdict::Verdict,
    ws::{
        classify, derive_ws_url, render_human as ws_render_human, render_json as ws_render_json,
        run_ws, WsReport,
    },
};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread::{self, JoinHandle},
    time::Duration,
};
use url::Url;

/// A disabled palette: human renderers emit no ANSI, so assertions can match the
/// plain text. Output is byte-identical to non-TTY default output.
fn plain() -> Palette {
    Palette::new(false)
}

/// An enabled palette: human renderers emit ANSI styling.
fn colored() -> Palette {
    Palette::new(true)
}

struct MockRpcServer {
    url: String,
    handle: JoinHandle<()>,
}

struct MockResponse {
    status: &'static str,
    body: &'static str,
}

impl MockResponse {
    fn ok(body: &'static str) -> Self {
        Self {
            status: "200 OK",
            body,
        }
    }

    fn status(status: &'static str, body: &'static str) -> Self {
        Self { status, body }
    }
}

impl MockRpcServer {
    fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                read_http_request(&mut stream);
                write_http_response(&mut stream, &response);
            }
        });

        Self { url, handle }
    }

    fn join(self) {
        self.handle.join().unwrap();
    }
}

fn read_http_request(stream: &mut TcpStream) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    loop {
        let bytes_read = stream.read(&mut chunk).unwrap();
        assert!(bytes_read > 0, "connection closed before headers completed");
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .or_else(|| line.strip_prefix("Content-Length: "))
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();

    while buffer.len() < header_end + content_length {
        let bytes_read = stream.read(&mut chunk).unwrap();
        assert!(bytes_read > 0, "connection closed before body completed");
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }
}

fn write_http_response(stream: &mut TcpStream, response: &MockResponse) {
    let response = format!(
        "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response.status,
        response.body.len(),
        response.body
    );
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn args_for(url: String) -> CheckArgs {
    CheckArgs {
        rpc: url,
        json: false,
        fail_on_warning: false,
        samples: 1,
        timeout_ms: 1_000,
    }
}

fn health_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#
}

fn version_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":2,"result":{"solana-core":"4.0.0","feature-set":123}}"#
}

fn genesis_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":3,"result":"5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"}"#
}

fn slot_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":4,"result":424013263}"#
}

fn latest_blockhash_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":5,"result":{"value":{"blockhash":"7xKXtgQvExample111111111111111111111111111","lastValidBlockHeight":123456}}}"#
}

fn blockhash_valid_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":6,"result":{"value":true}}"#
}

fn performance_ok() -> &'static str {
    r#"{"jsonrpc":"2.0","id":7,"result":[{"slot":10,"numSlots":64,"numTransactions":124000,"samplePeriodSecs":60,"numNonVoteTransactions":90000}]}"#
}

fn healthy_rpc_responses(slot: u64) -> Vec<MockResponse> {
    vec![
        MockResponse::ok(health_ok()),
        MockResponse::ok(version_ok()),
        MockResponse::ok(genesis_ok()),
        MockResponse::ok(Box::leak(
            format!(r#"{{"jsonrpc":"2.0","id":4,"result":{slot}}}"#).into_boxed_str(),
        )),
        MockResponse::ok(latest_blockhash_ok()),
        MockResponse::ok(blockhash_valid_ok()),
        MockResponse::ok(performance_ok()),
    ]
}

#[tokio::test]
async fn full_check_returns_good_for_healthy_rpc() {
    let server = MockRpcServer::start(vec![
        MockResponse::ok(health_ok()),
        MockResponse::ok(version_ok()),
        MockResponse::ok(genesis_ok()),
        MockResponse::ok(slot_ok()),
        MockResponse::ok(latest_blockhash_ok()),
        MockResponse::ok(blockhash_valid_ok()),
        MockResponse::ok(performance_ok()),
    ]);
    let expected_url = format!("{}/", server.url);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Good);
    assert_eq!(report.rpc_url, expected_url);
    assert_eq!(report.summary, "All RPC readiness checks passed");
    assert_eq!(report.checks.len(), 7);
    assert!(report.average_latency_ms.is_some());
    assert!(report
        .checks
        .iter()
        .all(|check| check.status == CheckStatus::Success));
}

#[tokio::test]
async fn full_check_reports_multiple_malformed_and_rpc_failures() {
    let server = MockRpcServer::start(vec![
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":1,"result":"behind"}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":2}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":3,"result":""}"#),
        MockResponse::ok(
            r#"{"jsonrpc":"2.0","id":4,"error":{"code":-32000,"message":"slot unavailable"}}"#,
        ),
        MockResponse::ok(
            r#"{"jsonrpc":"2.0","id":5,"result":{"value":{"blockhash":"","lastValidBlockHeight":123456}}}"#,
        ),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":7,"result":[]}"#),
    ]);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report
        .checks
        .iter()
        .any(|check| check.detail == "unexpected health response: behind"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getVersion" && check.detail == "missing result"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getGenesisHash" && check.detail == "empty genesis hash"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getSlot" && check.error_kind == Some(ErrorKind::RpcError)));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "isBlockhashValid"
            && check.detail == "latest blockhash unavailable"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getRecentPerformanceSamples"
            && check.detail == "no recent performance samples returned"));
}

#[tokio::test]
async fn full_check_reports_rpc_error_bad_metadata_and_missing_results() {
    let server = MockRpcServer::start(vec![
        MockResponse::ok(
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"node unhealthy"}}"#,
        ),
        MockResponse::ok(r#"{"jsonrpc":"1.0","id":2,"result":{"solana-core":"4.0.0"}}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":3}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":4,"result":424013263}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":5}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":7}"#),
    ]);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getHealth" && check.error_kind == Some(ErrorKind::RpcError)));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getVersion"
            && check.error_kind == Some(ErrorKind::MalformedResponse)
            && check.detail.contains("expected JSON-RPC 2.0")));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getGenesisHash" && check.detail == "missing result"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getLatestBlockhash" && check.detail == "missing result"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getRecentPerformanceSamples"
            && check.detail == "missing result"));
}

#[tokio::test]
async fn full_check_reports_invalid_blockhash_and_bad_response_metadata() {
    let server = MockRpcServer::start(vec![
        MockResponse::ok(health_ok()),
        MockResponse::ok(version_ok()),
        MockResponse::ok(genesis_ok()),
        MockResponse::ok(slot_ok()),
        MockResponse::ok(latest_blockhash_ok()),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":6,"result":{"value":false}}"#),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":99,"result":[]}"#),
    ]);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "isBlockhashValid"
            && check.detail == "latest blockhash is not valid"));
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getRecentPerformanceSamples"
            && check.error_kind == Some(ErrorKind::MalformedResponse)
            && check.detail.contains("expected response id 7")));
}

#[tokio::test]
async fn full_check_reports_missing_blockhash_valid_result() {
    let server = MockRpcServer::start(vec![
        MockResponse::ok(health_ok()),
        MockResponse::ok(version_ok()),
        MockResponse::ok(genesis_ok()),
        MockResponse::ok(slot_ok()),
        MockResponse::ok(latest_blockhash_ok()),
        MockResponse::ok(r#"{"jsonrpc":"2.0","id":6}"#),
        MockResponse::ok(performance_ok()),
    ]);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "isBlockhashValid" && check.detail == "missing result"));
}

#[tokio::test]
async fn full_check_classifies_http_and_decode_errors() {
    let server = MockRpcServer::start(vec![
        MockResponse::status("500 Internal Server Error", r#"{"error":"boom"}"#),
        MockResponse::ok(r#"not-json"#),
        MockResponse::ok(genesis_ok()),
        MockResponse::ok(slot_ok()),
        MockResponse::ok(latest_blockhash_ok()),
        MockResponse::ok(blockhash_valid_ok()),
        MockResponse::ok(performance_ok()),
    ]);

    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(
        report
            .checks
            .iter()
            .any(|check| check.method == "getHealth"
                && check.error_kind == Some(ErrorKind::HttpError))
    );
    assert!(report
        .checks
        .iter()
        .any(|check| check.method == "getVersion"
            && check.error_kind == Some(ErrorKind::MalformedResponse)));
}

#[tokio::test]
async fn invalid_rpc_url_returns_bad_report() {
    let report = run_check(args_for("not a url".to_string())).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert_eq!(report.rpc_url, "<invalid>");
    assert_eq!(report.average_latency_ms, None);
    assert_eq!(report.checks[0].error_kind, Some(ErrorKind::InvalidUrl));
}

#[test]
fn verdict_latency_and_report_helpers_are_covered() {
    let success = RpcCheck {
        category: CheckCategory::Core,
        method: "getHealth",
        status: CheckStatus::Success,
        latency_ms: Some(600),
        detail: "health is ok".to_string(),
        error_kind: None,
        critical: true,
    };
    let failed = RpcCheck {
        category: CheckCategory::Performance,
        method: "getRecentPerformanceSamples",
        status: CheckStatus::Failed,
        latency_ms: None,
        detail: "missing result".to_string(),
        error_kind: Some(ErrorKind::MalformedResponse),
        critical: false,
    };

    assert_eq!(calculate_verdict(&[], None), Verdict::Unknown);
    assert_eq!(
        calculate_verdict(std::slice::from_ref(&success), Some(600)),
        Verdict::Warning
    );
    assert_eq!(
        calculate_verdict(std::slice::from_ref(&success), None),
        Verdict::Unknown
    );
    assert_eq!(
        calculate_verdict(&[success.clone(), failed.clone()], Some(600)),
        Verdict::Warning
    );
    assert_eq!(Latency::from_duration(Duration::from_millis(42)).millis, 42);
    assert_eq!(
        average_latency_ms([Latency { millis: 100 }, Latency { millis: 300 }]),
        Some(200)
    );
    assert_eq!(average_latency_ms([]), None);

    let report = solana_infra_doctor::checks::CheckReport {
        verdict: Verdict::Warning,
        rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
        summary: "RPC checks succeeded, but average latency is elevated at 600ms".to_string(),
        average_latency_ms: None,
        latency_samples: None,
        fail_on_warning: true,
        checks: vec![success, failed],
    };
    // Verbose human output shows n/a latency, the fail-on-warning policy note,
    // and per-check error kinds.
    let verbose = render_human(&report, plain(), true);
    assert!(verbose.contains("n/a"));
    assert!(verbose.contains("--fail-on-warning is enabled"));
    assert!(verbose.contains("[malformed_response]"));

    let mut report_with_latency = report.clone();
    report_with_latency.average_latency_ms = Some(200);
    report_with_latency.fail_on_warning = false;
    let with_latency = render_human(&report_with_latency, plain(), false);
    assert!(with_latency.contains("200 ms"));
    assert!(!with_latency.contains("Warning policy"));

    let json = render_json(&report).unwrap();
    assert!(json.contains("\"verdict\": \"WARNING\""));
    solana_infra_doctor::report::print_json(&report).unwrap();

    assert_eq!(ErrorKind::InvalidUrl.label(), "invalid_url");
    assert_eq!(ErrorKind::Timeout.label(), "timeout");
    assert_eq!(ErrorKind::HttpError.label(), "http_error");
    assert_eq!(ErrorKind::RpcError.label(), "rpc_error");
    assert_eq!(ErrorKind::MalformedResponse.label(), "malformed_response");
    assert_eq!(ErrorKind::UnknownError.label(), "unknown_error");

    assert_eq!(CheckCategory::Core.label(), "Core");
    assert_eq!(CheckCategory::Blockhash.label(), "Blockhash");
    assert_eq!(CheckCategory::Performance.label(), "Performance");

    assert_eq!(Verdict::Good.exit_code(), 0);
    assert_eq!(Verdict::Warning.exit_code(), 1);
    assert_eq!(Verdict::Bad.exit_code(), 2);
    assert_eq!(Verdict::Unknown.exit_code(), 3);
    assert_eq!(Verdict::Good.to_string(), "GOOD");
    assert_eq!(Verdict::Warning.to_string(), "WARNING");
    assert_eq!(Verdict::Bad.to_string(), "BAD");
    assert_eq!(Verdict::Unknown.to_string(), "UNKNOWN");
}

#[test]
fn rpc_value_objects_are_covered() {
    let endpoint = RpcEndpoint::parse("https://user:pass@example.com/rpc?api-key=secret").unwrap();
    assert_eq!(endpoint.as_url().scheme(), "https");
    assert_eq!(endpoint.redacted(), "https://***:***@example.com/rpc");
    assert!(RpcEndpoint::parse("ftp://example.com").is_err());
    assert!(RpcEndpoint::parse("not a url").is_err());

    let request = JsonRpcRequest::new(7, "getHealth");
    assert_eq!(request.jsonrpc, "2.0");
    assert_eq!(request.id, 7);
    assert!(request.params.is_empty());

    let request = JsonRpcRequest::with_params(8, "isBlockhashValid", vec!["abc".into()]);
    assert_eq!(request.params[0], "abc");

    let latest: LatestBlockhashResponse = serde_json::from_str(
        r#"{"value":{"blockhash":"ExampleBlockhash111111111111111111111111111111","lastValidBlockHeight":123456}}"#,
    )
    .unwrap();
    assert_eq!(latest.value.last_valid_block_height, 123456);

    let validity: BlockhashValidResponse = serde_json::from_str(r#"{"value":true}"#).unwrap();
    assert!(validity.value);

    let sample: PerformanceSample = serde_json::from_str(
        r#"{"slot":10,"numSlots":64,"numTransactions":1200,"samplePeriodSecs":60,"numNonVoteTransactions":300}"#,
    )
    .unwrap();
    assert_eq!(sample.num_non_vote_transactions, Some(300));
}

fn compare_check_report(
    url: &str,
    verdict: Verdict,
    slot: Option<u64>,
    average_latency_ms: Option<u128>,
    blockhash_valid: bool,
    failed_methods: &[&'static str],
) -> solana_infra_doctor::checks::CheckReport {
    let mut checks = Vec::new();
    checks.push(RpcCheck {
        category: CheckCategory::Core,
        method: "getHealth",
        status: status_for("getHealth", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: "health is ok".to_string(),
        error_kind: error_for("getHealth", failed_methods),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Core,
        method: "getVersion",
        status: status_for("getVersion", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: "solana-core 4.0.0".to_string(),
        error_kind: error_for("getVersion", failed_methods),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Core,
        method: "getGenesisHash",
        status: status_for("getGenesisHash", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d".to_string(),
        error_kind: error_for("getGenesisHash", failed_methods),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Core,
        method: "getSlot",
        status: status_for("getSlot", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: slot.map_or_else(
            || "missing result".to_string(),
            |slot| format!("slot {slot}"),
        ),
        error_kind: error_for("getSlot", failed_methods),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Blockhash,
        method: "getLatestBlockhash",
        status: status_for("getLatestBlockhash", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: "7xKXtgQvExample111111111111111111111111111".to_string(),
        error_kind: error_for("getLatestBlockhash", failed_methods),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Blockhash,
        method: "isBlockhashValid",
        status: if blockhash_valid {
            CheckStatus::Success
        } else {
            CheckStatus::Failed
        },
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: if blockhash_valid {
            "latest blockhash is valid".to_string()
        } else {
            "latest blockhash unavailable".to_string()
        },
        error_kind: (!blockhash_valid).then_some(ErrorKind::MalformedResponse),
        critical: true,
    });
    checks.push(RpcCheck {
        category: CheckCategory::Performance,
        method: "getRecentPerformanceSamples",
        status: status_for("getRecentPerformanceSamples", failed_methods),
        latency_ms: Some(average_latency_ms.unwrap_or(0)),
        detail: "124000 transactions across 64 slots in 60s".to_string(),
        error_kind: error_for("getRecentPerformanceSamples", failed_methods),
        critical: false,
    });

    solana_infra_doctor::checks::CheckReport {
        verdict,
        rpc_url: url.to_string(),
        summary: verdict.to_string(),
        average_latency_ms,
        latency_samples: None,
        fail_on_warning: false,
        checks,
    }
}

fn status_for(method: &str, failed_methods: &[&str]) -> CheckStatus {
    if failed_methods.contains(&method) {
        CheckStatus::Failed
    } else {
        CheckStatus::Success
    }
}

fn error_for(method: &str, failed_methods: &[&str]) -> Option<ErrorKind> {
    failed_methods
        .contains(&method)
        .then_some(ErrorKind::MalformedResponse)
}

#[tokio::test]
async fn compare_rejects_fewer_than_two_rpc_urls() {
    let error = run_compare(CompareArgs {
        rpc: vec!["https://api.mainnet-beta.solana.com".to_string()],
        profile: CompareProfile::General,
        json: false,
        report: None,
        timeout_ms: 1_000,
    })
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "compare requires at least 2 RPC URLs");
}

#[tokio::test]
async fn compare_success_reuses_check_flow_and_redacts_urls() {
    let server_a = MockRpcServer::start(healthy_rpc_responses(200));
    let server_b = MockRpcServer::start(healthy_rpc_responses(190));
    let report = run_compare(CompareArgs {
        rpc: vec![
            server_a.url.replace("http://", "http://user:pass@"),
            server_b.url.clone(),
        ],
        profile: CompareProfile::General,
        json: false,
        report: None,
        timeout_ms: 1_000,
    })
    .await
    .unwrap();
    server_a.join();
    server_b.join();

    assert_eq!(report.profile.label(), "general");
    assert_eq!(report.endpoints.len(), 2);
    assert_eq!(report.endpoints[0].slot, Some(200));
    assert_eq!(report.endpoints[1].slot_lag, Some(10));
    assert!(report.endpoints[0].url.contains("***:***@"));
    assert!(!report.endpoints[0].url.contains("user:pass"));
}

#[test]
fn compare_slot_lag_scoring_and_selection_work() {
    assert_eq!(slot_lag(Some(100), Some(125)), Some(25));
    assert_eq!(slot_lag(None, Some(125)), None);

    let reports = vec![
        compare_check_report(
            "https://api.mainnet-beta.solana.com/",
            Verdict::Good,
            Some(347_000_000),
            Some(142),
            true,
            &[],
        ),
        compare_check_report(
            "https://slow.provider.com/",
            Verdict::Warning,
            Some(346_999_700),
            Some(812),
            true,
            &["getRecentPerformanceSamples"],
        ),
        compare_check_report(
            "https://bad.provider.com/",
            Verdict::Bad,
            Some(346_990_000),
            Some(2_000),
            false,
            &[
                "getHealth",
                "getVersion",
                "getGenesisHash",
                "getSlot",
                "getLatestBlockhash",
            ],
        ),
    ];

    let report = build_compare_report(CompareProfile::General, &reports);

    assert!(!report.network_mismatch);
    assert_eq!(report.best_endpoint_index, Some(1));
    assert_eq!(report.worst_endpoint_index, Some(3));
    assert_eq!(report.endpoints[0].slot_lag, Some(0));
    assert_eq!(report.endpoints[1].slot_lag, Some(300));
    assert_eq!(report.endpoints[2].score, 0);
}

#[test]
fn compare_profiles_apply_expected_adjustments_and_notes() {
    let base = CompareEndpoint {
        index: 1,
        url: "https://example.com/".to_string(),
        genesis_hash: None,
        verdict: Verdict::Good,
        score: 0,
        slot: Some(100),
        slot_lag: Some(100),
        average_latency_ms: Some(900),
        failed_checks: vec!["getRecentPerformanceSamples".to_string()],
        blockhash_valid: false,
        notes: Vec::new(),
    };

    let general = score_endpoint(CompareProfile::General, &base);
    let bot = score_endpoint(CompareProfile::Bot, &base);
    let indexer = score_endpoint(CompareProfile::Indexer, &base);
    assert!(bot < general);
    assert!(indexer < general);

    let wallet_report = build_compare_report(
        CompareProfile::Wallet,
        &[compare_check_report(
            "https://wallet.example.com/",
            Verdict::Warning,
            Some(100),
            Some(200),
            false,
            &[],
        )],
    );
    assert!(wallet_report.endpoints[0]
        .notes
        .iter()
        .any(|note| note.contains("blockhash")));

    let ci_report = build_compare_report(
        CompareProfile::Ci,
        &[
            compare_check_report(
                "https://good.example.com/",
                Verdict::Good,
                Some(100),
                Some(200),
                true,
                &[],
            ),
            compare_check_report(
                "https://warn.example.com/",
                Verdict::Warning,
                Some(99),
                Some(200),
                true,
                &[],
            ),
        ],
    );
    assert!(ci_report
        .recommendation
        .contains("WARNING or BAD endpoints are not recommended for pass gates"));

    let indexer_report = build_compare_report(
        CompareProfile::Indexer,
        &[
            compare_check_report(
                "https://fresh-indexer.example.com/",
                Verdict::Good,
                Some(200),
                Some(400),
                true,
                &[],
            ),
            compare_check_report(
                "https://stale-indexer.example.com/",
                Verdict::Warning,
                Some(100),
                Some(400),
                true,
                &["getRecentPerformanceSamples"],
            ),
        ],
    );
    assert!(indexer_report
        .recommendation
        .contains("freshness-sensitive indexer workloads"));
    assert!(indexer_report.endpoints[1]
        .notes
        .iter()
        .any(|note| note.contains("performance samples")));
    assert!(indexer_report.endpoints[1]
        .notes
        .iter()
        .any(|note| note.contains("Slot lag")));
}

#[test]
fn compare_json_markdown_and_human_outputs_have_required_shape() {
    let reports = vec![
        compare_check_report(
            "https://api.mainnet-beta.solana.com/",
            Verdict::Good,
            Some(347_000_000),
            Some(142),
            true,
            &[],
        ),
        compare_check_report(
            "https://***:***@provider.com/rpc",
            Verdict::Warning,
            Some(346_999_700),
            Some(812),
            true,
            &["getRecentPerformanceSamples"],
        ),
    ];
    let report = build_compare_report(CompareProfile::Bot, &reports);

    let concise = render_compare_human(&report, plain(), false);
    assert!(concise.contains("Solana Infra Doctor · RPC Comparison"));
    assert!(concise.contains("Profile: bot"));
    assert!(concise.contains("baseline")); // slot lag column
    assert!(concise.contains("/100")); // score column
    assert!(concise.contains("Recommendation"));
    // Concise output is a summary table: no per-endpoint detail blocks.
    assert!(!concise.contains("Failed checks"));

    let verbose = render_compare_human(&report, plain(), true);
    assert!(verbose.contains("RPC #1"));
    assert!(verbose.contains("Failed checks"));
    assert!(verbose.contains("getRecentPerformanceSamples"));

    let json = render_compare_json(&report).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["profile"], "bot");
    assert_eq!(parsed["best_endpoint_index"], 1);
    assert_eq!(parsed["worst_endpoint_index"], 2);
    assert!(parsed["endpoints"].is_array());
    assert_eq!(parsed["endpoints"][0]["index"], 1);

    let markdown = render_markdown(&report);
    assert!(markdown.contains("# Solana Infra Doctor RPC Compare Report"));
    assert!(markdown.contains("Profile: `bot`"));
    assert!(markdown.contains("| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |"));
    assert!(markdown.contains("## Recommendation"));
    assert!(markdown.contains("## Limitations"));
    assert!(markdown.contains("## Disclaimer"));
    assert!(markdown.contains("`https://***:***@provider.com/rpc`"));
    assert!(!markdown.contains("api-key="));

    let path = temp_report_path("sol-doctor-compare-report.md");
    write_markdown_report(&report, &path).unwrap();
    let written = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert!(written.contains("Solana Infra Doctor RPC Compare Report"));
}

#[test]
fn compare_tie_breakers_and_format_variants_are_covered() {
    assert_eq!(CompareProfile::General.label(), "general");
    assert_eq!(CompareProfile::Wallet.label(), "wallet");
    assert_eq!(CompareProfile::Bot.label(), "bot");
    assert_eq!(CompareProfile::Indexer.label(), "indexer");
    assert_eq!(CompareProfile::Ci.label(), "ci");
    assert_eq!(CompareProfile::Wallet.to_string(), "wallet");
    assert_eq!(CompareProfileSummary::General.label(), "general");
    assert_eq!(CompareProfileSummary::Wallet.label(), "wallet");
    assert_eq!(CompareProfileSummary::Indexer.label(), "indexer");
    assert_eq!(CompareProfileSummary::Ci.label(), "ci");

    let reports = vec![
        compare_check_report(
            "https://missing.example.com/",
            Verdict::Unknown,
            None,
            None,
            false,
            &[],
        ),
        compare_check_report(
            "https://warning-fast.example.com/",
            Verdict::Warning,
            Some(50),
            Some(500),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
        compare_check_report(
            "https://warning-slow.example.com/",
            Verdict::Warning,
            Some(49),
            Some(900),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
    ];
    let report = build_compare_report(CompareProfile::Wallet, &reports);

    assert_eq!(report.best_endpoint_index, Some(2));
    assert_eq!(report.worst_endpoint_index, Some(1));
    assert!(report.endpoints[1]
        .notes
        .iter()
        .any(|note| note.contains("core RPC methods")));

    let human = render_compare_human(&report, plain(), true);
    assert!(human.contains("Slot"));
    assert!(human.contains("Slot lag"));
    assert!(human.contains("Average latency"));
    assert!(human.contains("n/a"));

    let markdown = render_markdown(&build_compare_report(
        CompareProfile::General,
        &[
            compare_check_report(
                "https://no-notes.example.com/",
                Verdict::Good,
                Some(10),
                Some(100),
                true,
                &[],
            ),
            compare_check_report(
                "https://invalid-blockhash.example.com/",
                Verdict::Bad,
                Some(9),
                Some(2_000),
                false,
                &["getHealth"],
            ),
        ],
    ));
    assert!(markdown.contains("- Notes: none"));
    assert!(markdown.contains("| RPC #2 | `https://invalid-blockhash.example.com/` | `BAD`"));
    assert!(markdown.contains("| no |"));

    let tie_reports = vec![
        compare_check_report(
            "https://tie-good.example.com/",
            Verdict::Good,
            Some(100),
            Some(700),
            false,
            &["getHealth", "getVersion", "getGenesisHash"],
        ),
        compare_check_report(
            "https://tie-warning.example.com/",
            Verdict::Warning,
            Some(100),
            Some(700),
            true,
            &["getHealth", "getVersion", "getGenesisHash"],
        ),
    ];
    let tie_report = build_compare_report(CompareProfile::General, &tie_reports);
    assert_eq!(tie_report.best_endpoint_index, Some(1));

    let latency_tie_reports = vec![
        compare_check_report(
            "https://latency-slower.example.com/",
            Verdict::Good,
            Some(100),
            Some(700),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
        compare_check_report(
            "https://latency-faster.example.com/",
            Verdict::Good,
            Some(100),
            Some(300),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
    ];
    let latency_tie_report = build_compare_report(CompareProfile::General, &latency_tie_reports);
    assert_eq!(latency_tie_report.best_endpoint_index, Some(2));

    let slot_tie_reports = vec![
        compare_check_report(
            "https://slot-behind.example.com/",
            Verdict::Good,
            Some(90),
            Some(300),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
        compare_check_report(
            "https://slot-baseline.example.com/",
            Verdict::Good,
            Some(100),
            Some(300),
            true,
            &["getHealth", "getVersion", "getGenesisHash", "getSlot"],
        ),
    ];
    let slot_tie_report = build_compare_report(CompareProfile::General, &slot_tie_reports);
    assert_eq!(slot_tie_report.best_endpoint_index, Some(2));
}

fn temp_report_path(file_name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("{}-{file_name}", std::process::id()));
    path
}

#[test]
fn redaction_masks_credentials_query_and_path_tokens() {
    let basic = RpcEndpoint::parse("https://user:pass@rpc.example.com/rpc?api-key=FAKE_SECRET_123")
        .unwrap();
    let basic_redacted = basic.redacted();
    assert!(!basic_redacted.contains("pass"));
    assert!(!basic_redacted.contains("FAKE_SECRET_123"));
    assert!(basic_redacted.contains("***"));

    let mixed = RpcEndpoint::parse("https://rpc.example.com/?API-KEY=AAASECRET&Token=BBBSECRET")
        .unwrap()
        .redacted();
    assert!(!mixed.contains("AAASECRET"));
    assert!(!mixed.contains("BBBSECRET"));

    let alchemy = RpcEndpoint::parse("https://solana-mainnet.g.alchemy.com/v2/SECRETALCHEMYKEY")
        .unwrap()
        .redacted();
    assert_eq!(alchemy, "https://solana-mainnet.g.alchemy.com/v2/***");

    let quicknode = RpcEndpoint::parse(
        "https://example.solana-mainnet.quiknode.pro/abcdef0123456789abcdef0123/",
    )
    .unwrap()
    .redacted();
    assert!(!quicknode.contains("abcdef0123456789abcdef0123"));
    assert!(quicknode.contains("***"));

    let public = RpcEndpoint::parse("https://api.mainnet-beta.solana.com")
        .unwrap()
        .redacted();
    assert_eq!(public, "https://api.mainnet-beta.solana.com/");

    let endpoint =
        RpcEndpoint::parse("https://user:pass@rpc.example.com/v2/SECRETALCHEMYKEY").unwrap();
    let debug = format!("{endpoint:?}");
    assert!(!debug.contains("pass"));
    assert!(!debug.contains("SECRETALCHEMYKEY"));
    assert!(debug.contains("***"));
}

#[test]
fn redaction_sanitizes_error_text_and_passthrough() {
    let leaked =
        "error sending request for url (https://rpc.helius.xyz/?api-key=FAKE_SECRET_123): refused";
    let clean = redact_text(leaked);
    assert!(!clean.contains("FAKE_SECRET_123"));
    assert!(clean.contains("https://rpc.helius.xyz/"));
    assert!(clean.contains("refused"));

    let ws = redact_text("connect failed: wss://node.example.com/v2/WSSECRETTOKEN0001 closed");
    assert!(!ws.contains("WSSECRETTOKEN0001"));

    assert_eq!(redact_text("visit https:// now"), "visit https:// now");
    assert_eq!(redact_text("plain message"), "plain message");
}

#[tokio::test]
async fn check_does_not_leak_secret_in_error_output() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // free the port so the connection is refused deterministically

    let report = run_check(CheckArgs {
        rpc: format!("https://127.0.0.1:{port}/?api-key=FAKE_SECRET_123"),
        json: false,
        fail_on_warning: false,
        samples: 1,
        timeout_ms: 1_500,
    })
    .await
    .unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(!report.rpc_url.contains("FAKE_SECRET_123"));
    assert!(report
        .checks
        .iter()
        .all(|check| !check.detail.contains("FAKE_SECRET_123")));

    // The secret must never appear in default human output, verbose human
    // output (which shows the full redacted URL), or JSON.
    let concise = render_human(&report, plain(), false);
    let verbose = render_human(&report, plain(), true);
    let json = render_json(&report).unwrap();
    assert!(!concise.contains("FAKE_SECRET_123"));
    assert!(!verbose.contains("FAKE_SECRET_123"));
    assert!(!json.contains("FAKE_SECRET_123"));
}

#[derive(Clone, Copy)]
enum WsBehavior {
    Happy,
    NeverNotify,
    Malformed,
    CloseAfterConfirm,
    NotifyMissingSlot,
    BinaryThenHappy,
}

async fn start_mock_ws(behavior: WsBehavior) -> String {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut ws) = accept_async(stream).await else {
            return;
        };
        let _ = ws.next().await; // slotSubscribe request
        let confirm = r#"{"jsonrpc":"2.0","result":7,"id":1}"#;
        let notification = r#"{"jsonrpc":"2.0","method":"slotNotification","params":{"result":{"parent":1,"root":1,"slot":424000000},"subscription":7}}"#;

        match behavior {
            WsBehavior::Happy => {
                let _ = ws.send(Message::text(confirm)).await;
                let _ = ws.send(Message::text(notification)).await;
                let _ = ws.next().await; // slotUnsubscribe request
                let _ = ws
                    .send(Message::text(r#"{"jsonrpc":"2.0","result":true,"id":2}"#))
                    .await;
                let _ = ws.close(None).await;
            }
            WsBehavior::NeverNotify => {
                let _ = ws.send(Message::text(confirm)).await;
                tokio::time::sleep(Duration::from_secs(5)).await; // hold past client timeout
            }
            WsBehavior::Malformed => {
                let _ = ws.send(Message::text("not json")).await;
                let _ = ws.close(None).await;
            }
            WsBehavior::CloseAfterConfirm => {
                let _ = ws.send(Message::text(confirm)).await;
                let _ = ws.close(None).await;
            }
            WsBehavior::NotifyMissingSlot => {
                // A notification with no `slot` field and no prior confirmation.
                let _ = ws
                    .send(Message::text(
                        r#"{"jsonrpc":"2.0","method":"slotNotification","params":{"result":{"parent":1}}}"#,
                    ))
                    .await;
                let _ = ws.close(None).await;
            }
            WsBehavior::BinaryThenHappy => {
                let _ = ws.send(Message::binary(vec![1u8, 2, 3])).await;
                let _ = ws.send(Message::text(confirm)).await;
                let _ = ws.send(Message::text(notification)).await;
                let _ = ws.next().await; // slotUnsubscribe request
                let _ = ws
                    .send(Message::text(r#"{"jsonrpc":"2.0","result":true,"id":2}"#))
                    .await;
                let _ = ws.close(None).await;
            }
        }
    });

    format!("ws://127.0.0.1:{port}")
}

fn ws_args(ws_url: Option<String>, timeout_ms: u64) -> WsArgs {
    WsArgs {
        rpc: "https://example.com".to_string(),
        ws: ws_url,
        json: false,
        timeout_ms,
    }
}

#[tokio::test]
async fn ws_happy_path_reports_good() {
    let url = start_mock_ws(WsBehavior::Happy).await;
    let report = run_ws(ws_args(Some(url), 5_000)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Good);
    assert!(report.connected);
    assert!(report.subscribed);
    assert_eq!(report.first_slot, Some(424_000_000));
    assert!(report.time_to_first_notification_ms.is_some());
    assert!(report.unsubscribed);
    assert!(report.closed_cleanly);

    let human = ws_render_human(&report, plain(), false);
    assert!(human.contains("Solana Infra Doctor · WebSocket Readiness"));
    assert!(human.contains("GOOD"));
    assert!(human.contains("First notification"));
    assert!(human.contains("slot 424000000"));

    let json = ws_render_json(&report).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["verdict"], "GOOD");
    assert_eq!(parsed["first_slot"], 424_000_000);
    assert_eq!(parsed["subscription_method"], "slotSubscribe");
}

#[tokio::test]
async fn ws_timeout_without_notification_is_bad() {
    let url = start_mock_ws(WsBehavior::NeverNotify).await;
    let report = run_ws(ws_args(Some(url), 300)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.connected);
    assert!(report.subscribed); // confirmation was received
    assert!(report.first_slot.is_none());
    assert!(report.time_to_first_notification_ms.is_none());
    assert!(ws_render_human(&report, plain(), false).contains("BAD"));
}

#[tokio::test]
async fn ws_malformed_frame_is_bad() {
    let url = start_mock_ws(WsBehavior::Malformed).await;
    let report = run_ws(ws_args(Some(url), 2_000)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.connected);
    assert!(!report.subscribed);
    assert!(report.first_slot.is_none());
}

#[tokio::test]
async fn ws_server_close_after_confirm_is_bad() {
    let url = start_mock_ws(WsBehavior::CloseAfterConfirm).await;
    let report = run_ws(ws_args(Some(url), 2_000)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(report.connected);
    assert!(report.first_slot.is_none());
}

#[tokio::test]
async fn ws_connection_refused_is_bad() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // free the port so the connection is refused deterministically

    let report = run_ws(ws_args(Some(format!("ws://127.0.0.1:{port}")), 1_500))
        .await
        .unwrap();

    assert_eq!(report.verdict, Verdict::Bad);
    assert!(!report.connected);
    assert!(ws_render_human(&report, plain(), false).contains("connection failed"));
}

#[tokio::test]
async fn ws_invalid_rpc_and_ws_urls_are_rejected() {
    let invalid_rpc = run_ws(WsArgs {
        rpc: "not a url".to_string(),
        ws: None,
        json: false,
        timeout_ms: 1_000,
    })
    .await
    .unwrap();
    assert_eq!(invalid_rpc.verdict, Verdict::Bad);
    assert!(invalid_rpc.summary.contains("invalid RPC URL"));

    let invalid_ws = run_ws(WsArgs {
        rpc: "https://example.com".to_string(),
        ws: Some("ftp://example.com".to_string()),
        json: false,
        timeout_ms: 1_000,
    })
    .await
    .unwrap();
    assert_eq!(invalid_ws.verdict, Verdict::Bad);
    assert!(invalid_ws.summary.contains("invalid WebSocket URL"));
}

#[test]
fn ws_url_derivation_and_redaction() {
    let from_https = derive_ws_url(
        &Url::parse("https://api.mainnet-beta.solana.com").unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(from_https.as_str(), "wss://api.mainnet-beta.solana.com/");

    let from_http = derive_ws_url(&Url::parse("http://localhost:8899").unwrap(), None).unwrap();
    assert_eq!(from_http.as_str(), "ws://localhost:8899/");

    let override_ws = derive_ws_url(
        &Url::parse("https://example.com").unwrap(),
        Some("wss://realtime.example.com/feed"),
    )
    .unwrap();
    assert_eq!(override_ws.as_str(), "wss://realtime.example.com/feed");

    assert!(derive_ws_url(
        &Url::parse("https://example.com").unwrap(),
        Some("https://not-websocket.example.com")
    )
    .is_err());
    // Unparseable override and a non-HTTP source scheme are both rejected.
    assert!(derive_ws_url(
        &Url::parse("https://example.com").unwrap(),
        Some("not a url")
    )
    .is_err());
    assert!(derive_ws_url(&Url::parse("ftp://example.com").unwrap(), None).is_err());

    // Secrets in the derived WebSocket URL are redacted.
    let secret = derive_ws_url(
        &Url::parse("https://node.example.com/?api-key=FAKE_SECRET_123").unwrap(),
        None,
    )
    .unwrap();
    assert!(!redact_url(&secret).contains("FAKE_SECRET_123"));
}

#[test]
fn ws_classify_covers_warning_and_bad_branches() {
    let base = WsReport {
        verdict: Verdict::Unknown,
        rpc_url: "https://example.com/".to_string(),
        ws_url: "wss://example.com/".to_string(),
        connected: true,
        connect_latency_ms: Some(20),
        subscription_method: "slotSubscribe",
        subscribed: true,
        time_to_first_notification_ms: Some(120),
        first_slot: Some(100),
        unsubscribed: true,
        closed_cleanly: true,
        summary: String::new(),
        notes: Vec::new(),
    };

    assert_eq!(classify(&base).0, Verdict::Good);

    let slow = WsReport {
        time_to_first_notification_ms: Some(5_000),
        ..base.clone()
    };
    let (verdict, _, notes) = classify(&slow);
    assert_eq!(verdict, Verdict::Warning);
    assert!(notes.iter().any(|note| note.contains("slow")));

    let unclean = WsReport {
        unsubscribed: false,
        closed_cleanly: false,
        ..base.clone()
    };
    assert_eq!(classify(&unclean).0, Verdict::Warning);

    let disconnected = WsReport {
        connected: false,
        ..base
    };
    assert_eq!(classify(&disconnected).0, Verdict::Bad);
}

#[tokio::test]
async fn ws_notification_without_slot_or_subscription_is_warning() {
    let url = start_mock_ws(WsBehavior::NotifyMissingSlot).await;
    let report = run_ws(ws_args(Some(url), 2_000)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Warning);
    assert!(report.subscribed);
    assert!(report.first_slot.is_none());
    assert!(report.time_to_first_notification_ms.is_some());
    assert!(!report.unsubscribed); // no subscription id was provided

    // Rendering a notification that arrived without a slot value exercises the
    // detail fallback path.
    assert!(ws_render_human(&report, plain(), false).contains("First notification"));
}

#[tokio::test]
async fn ws_ignores_non_text_frames_before_notification() {
    let url = start_mock_ws(WsBehavior::BinaryThenHappy).await;
    let report = run_ws(ws_args(Some(url), 5_000)).await.unwrap();

    assert_eq!(report.verdict, Verdict::Good);
    assert_eq!(report.first_slot, Some(424_000_000));
}

#[test]
fn ws_render_human_shows_degraded_steps_and_notes() {
    let degraded = WsReport {
        verdict: Verdict::Warning,
        rpc_url: "https://example.com/".to_string(),
        ws_url: "wss://example.com/".to_string(),
        connected: true,
        connect_latency_ms: Some(40),
        subscription_method: "slotSubscribe",
        subscribed: true,
        time_to_first_notification_ms: Some(5_000),
        first_slot: Some(123),
        unsubscribed: false,
        closed_cleanly: false,
        summary: "websocket is reachable but realtime behavior is degraded".to_string(),
        notes: vec!["First slot notification was slow at 5000ms.".to_string()],
    };
    let report = WsReport {
        notes: classify(&degraded).2,
        ..degraded
    };

    let human = ws_render_human(&report, plain(), false);
    assert!(human.contains("WARNING"));
    assert!(human.contains("Unsubscribe"));
    assert!(human.contains("Close"));
    assert!(human.contains("FAIL"));
    assert!(human.contains("Notes"));
    assert!(human.contains("slow"));
}

fn with_genesis(
    mut report: solana_infra_doctor::checks::CheckReport,
    hash: &str,
) -> solana_infra_doctor::checks::CheckReport {
    for check in &mut report.checks {
        if check.method == "getGenesisHash" {
            check.detail = hash.to_string();
            check.status = CheckStatus::Success;
            check.error_kind = None;
        }
    }
    report
}

#[test]
fn compare_rejects_mismatched_genesis_networks() {
    let mainnet = compare_check_report(
        "https://api.mainnet-beta.solana.com/",
        Verdict::Good,
        Some(347_000_000),
        Some(140),
        true,
        &[],
    );
    let devnet = with_genesis(
        compare_check_report(
            "https://api.devnet.solana.com/",
            Verdict::Good,
            Some(466_000_000),
            Some(150),
            true,
            &[],
        ),
        "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
    );
    let report = build_compare_report(CompareProfile::Bot, &[mainnet, devnet]);

    assert!(report.network_mismatch);
    assert_eq!(report.best_endpoint_index, None);
    assert_eq!(report.worst_endpoint_index, None);
    assert!(report.mismatch_reason.is_some());
    assert!(report
        .endpoints
        .iter()
        .all(|endpoint| endpoint.slot_lag.is_none()));

    let concise = render_compare_human(&report, plain(), false);
    assert!(concise.contains("different Solana networks"));
    // The genesis hash is detail, shown only in verbose.
    let verbose = render_compare_human(&report, plain(), true);
    assert!(verbose.contains("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"));

    let json = render_compare_json(&report).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["network_mismatch"], true);
    assert!(parsed["mismatch_reason"].is_string());
    assert!(parsed["best_endpoint_index"].is_null());
    assert!(parsed["worst_endpoint_index"].is_null());

    let markdown = render_markdown(&report);
    assert!(markdown.contains("## Network Mismatch"));
    assert!(markdown.contains("different Solana networks"));
    assert!(markdown.contains("- Best RPC: n/a (different networks)"));
}

#[test]
fn compare_recommendation_describes_latency_freshness_tradeoff() {
    let fast_but_stale = compare_check_report(
        "https://fast-stale.example.com/",
        Verdict::Good,
        Some(100),
        Some(120),
        true,
        &[],
    );
    let slow_but_fresh = compare_check_report(
        "https://slow-fresh.example.com/",
        Verdict::Good,
        Some(200),
        Some(600),
        true,
        &[],
    );
    let report = build_compare_report(CompareProfile::Bot, &[fast_but_stale, slow_but_fresh]);

    assert!(!report.network_mismatch);
    assert_eq!(report.best_endpoint_index, Some(2));
    assert_eq!(report.worst_endpoint_index, Some(1));
    assert!(report
        .recommendation
        .contains("RPC #1 has lower latency, but RPC #2 is fresher"));
    assert!(report
        .recommendation
        .contains("slot freshness may matter more than raw HTTP latency"));
    assert!(!report
        .recommendation
        .contains("Avoid RPC #1 for latency-sensitive"));
}

#[test]
fn colored_human_output_is_semantic_and_disabled_is_byte_identical() {
    let on = colored();
    let off = plain();

    // --- check ---
    let check = compare_check_report(
        "https://api.mainnet-beta.solana.com/",
        Verdict::Bad,
        Some(347_000_000),
        Some(120),
        false,
        &["getRecentPerformanceSamples"],
    );
    // Disabled palette is byte-identical to plain default output.
    let check_default_plain = render_human(&check, off, false);
    assert_eq!(render_human(&check, off, false), check_default_plain);
    let check_default_colored = render_human(&check, on, false);
    assert_ne!(check_default_colored, check_default_plain);
    // Title carries the azure accent; verdict and category statuses are colored.
    assert!(check_default_colored
        .contains("\x1b[1;38;2;88;166;255mSolana Infra Doctor · RPC Readiness\x1b[0m"));
    assert!(check_default_colored.contains("\x1b[1;38;2;248;81;73mBAD\x1b[0m")); // verdict
    assert!(check_default_colored.contains("\x1b[1;38;2;63;185;80mPASS")); // a passing category
                                                                           // Verbose: the failed method renders FAIL in bold red; disabled stays plain.
    let check_verbose_plain = render_human(&check, off, true);
    assert_eq!(render_human(&check, off, true), check_verbose_plain);
    let check_verbose_colored = render_human(&check, on, true);
    assert!(check_verbose_colored.contains("\x1b[1;38;2;248;81;73mFAIL"));

    // --- compare (healthy + degraded endpoints, then a network mismatch) ---
    let compare = build_compare_report(
        CompareProfile::General,
        &[
            compare_check_report(
                "https://a.example.com/",
                Verdict::Good,
                Some(347_000_000),
                Some(100),
                true,
                &[],
            ),
            compare_check_report(
                "https://b.example.com/",
                Verdict::Warning,
                Some(346_900_000),
                Some(800),
                false,
                &["getRecentPerformanceSamples"],
            ),
        ],
    );
    let compare_plain = render_compare_human(&compare, off, false);
    assert_eq!(render_compare_human(&compare, off, false), compare_plain);
    let compare_colored = render_compare_human(&compare, on, false);
    assert!(compare_colored
        .contains("\x1b[1;38;2;88;166;255mSolana Infra Doctor · RPC Comparison\x1b[0m"));
    assert!(compare_colored.contains("\x1b[1;38;2;63;185;80mGOOD\x1b[0m")); // green verdict
    assert!(compare_colored.contains("\x1b[1;38;2;210;153;34mWARNING\x1b[0m")); // amber verdict
                                                                                // Verbose: per-endpoint blocks, bold RPC headings, blockhash yes/no colored.
    let compare_verbose = render_compare_human(&compare, on, true);
    assert!(compare_verbose.contains("\x1b[1mRPC #1\x1b[0m"));
    assert!(compare_verbose.contains("\x1b[38;2;63;185;80myes\x1b[0m")); // blockhash yes -> green
    assert!(compare_verbose.contains("\x1b[38;2;248;81;73mno\x1b[0m")); // blockhash no -> red

    let mismatch = build_compare_report(
        CompareProfile::General,
        &[
            with_genesis(
                compare_check_report(
                    "https://a.example.com/",
                    Verdict::Good,
                    Some(1),
                    Some(10),
                    true,
                    &[],
                ),
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
            with_genesis(
                compare_check_report(
                    "https://b.example.com/",
                    Verdict::Good,
                    Some(1),
                    Some(10),
                    true,
                    &[],
                ),
                "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            ),
        ],
    );
    assert!(mismatch.network_mismatch);
    let mismatch_plain = render_compare_human(&mismatch, off, false);
    assert_eq!(render_compare_human(&mismatch, off, false), mismatch_plain);
    let mismatch_colored = render_compare_human(&mismatch, on, false);
    assert!(mismatch_colored.contains("different Solana networks"));
    assert!(mismatch_colored.contains("\x1b[1;38;2;210;153;34m")); // amber mismatch banner

    // --- ws (a passing and a failing step, plus notes) ---
    let ws = WsReport {
        verdict: Verdict::Warning,
        rpc_url: "https://example.com/".to_string(),
        ws_url: "wss://example.com/".to_string(),
        connected: true,
        connect_latency_ms: Some(40),
        subscription_method: "slotSubscribe",
        subscribed: true,
        time_to_first_notification_ms: Some(5_000),
        first_slot: Some(123),
        unsubscribed: false,
        closed_cleanly: false,
        summary: "degraded".to_string(),
        notes: vec!["First notification was slow at 5000 ms.".to_string()],
    };
    let ws_plain = ws_render_human(&ws, off, false);
    assert_eq!(ws_render_human(&ws, off, false), ws_plain);
    let ws_colored = ws_render_human(&ws, on, false);
    assert!(ws_colored
        .contains("\x1b[1;38;2;88;166;255mSolana Infra Doctor · WebSocket Readiness\x1b[0m"));
    assert!(ws_colored.contains("\x1b[1;38;2;63;185;80mPASS")); // Connect PASS -> bold green
    assert!(ws_colored.contains("\x1b[1;38;2;248;81;73mFAIL")); // Close FAIL -> bold red
    assert!(ws_colored.contains("\x1b[1mNotes\x1b[0m")); // bold heading
}

#[test]
fn palette_helpers_and_choice_resolution() {
    let on = Palette::new(true);
    assert_eq!(on.dim("d"), "\x1b[38;2;139;148;158md\x1b[0m");
    assert_eq!(on.label("l"), "\x1b[38;2;139;148;158ml\x1b[0m");
    assert_eq!(on.heading("h"), "\x1b[1mh\x1b[0m");
    assert_eq!(on.bold("b"), "\x1b[1mb\x1b[0m");
    assert_eq!(on.title("t"), "\x1b[1;38;2;88;166;255mt\x1b[0m");
    assert_eq!(on.ok("PASS"), "\x1b[1;38;2;63;185;80mPASS\x1b[0m");
    assert_eq!(on.warn("WARN"), "\x1b[1;38;2;210;153;34mWARN\x1b[0m");
    assert_eq!(on.fail("FAIL"), "\x1b[1;38;2;248;81;73mFAIL\x1b[0m");
    assert_eq!(on.good("yes"), "\x1b[38;2;63;185;80myes\x1b[0m");
    assert_eq!(on.bad("no"), "\x1b[38;2;248;81;73mno\x1b[0m");
    assert_eq!(
        on.verdict(Verdict::Unknown),
        "\x1b[1;38;2;139;148;158mUNKNOWN\x1b[0m"
    );
    assert!(on.enabled());

    let off = Palette::new(false);
    assert_eq!(off.title("t"), "t");
    assert_eq!(off.verdict(Verdict::Good), "GOOD");
    assert_eq!(off.ok("PASS"), "PASS");
    assert_eq!(off.warn("WARN"), "WARN");
    assert_eq!(off.fail("FAIL"), "FAIL");
    assert_eq!(off.good("yes"), "yes");
    assert_eq!(off.bad("no"), "no");
    assert!(!off.enabled());

    // Every ColorChoice / context branch (choice, is_terminal, no_color, term_dumb, json).
    assert!(Palette::resolve(ColorChoice::Always, false, false, false, false).enabled());
    assert!(!Palette::resolve(ColorChoice::Always, true, false, false, true).enabled()); // json
    assert!(!Palette::resolve(ColorChoice::Never, true, false, false, false).enabled());
    assert!(Palette::resolve(ColorChoice::Auto, true, false, false, false).enabled());
    assert!(!Palette::resolve(ColorChoice::Auto, false, false, false, false).enabled()); // not a tty
    assert!(!Palette::resolve(ColorChoice::Auto, true, true, false, false).enabled()); // NO_COLOR
    assert!(!Palette::resolve(ColorChoice::Auto, true, false, true, false).enabled());
    // TERM=dumb
}

#[test]
fn output_style_helpers() {
    use solana_infra_doctor::output::style::{endpoint_label, millis, Status};

    assert_eq!(millis(68), "68 ms");

    // Status labels and their colored forms (all four variants).
    let on = colored();
    let off = plain();
    assert_eq!(Status::Pass.label(), "PASS");
    assert_eq!(Status::Warn.label(), "WARN");
    assert_eq!(Status::Fail.label(), "FAIL");
    assert_eq!(Status::Skip.label(), "SKIP");
    assert_eq!(Status::Pass.paint(off), "PASS");
    assert_eq!(Status::Warn.paint(on), "\x1b[1;38;2;210;153;34mWARN\x1b[0m");
    assert_eq!(Status::Fail.paint(on), "\x1b[1;38;2;248;81;73mFAIL\x1b[0m");
    assert_eq!(Status::Skip.paint(on), "\x1b[38;2;139;148;158mSKIP\x1b[0m");

    // endpoint_label extracts the host; unparseable input falls back to itself.
    assert_eq!(
        endpoint_label("https://api.mainnet-beta.solana.com/"),
        "api.mainnet-beta.solana.com"
    );
    assert_eq!(endpoint_label("not a url"), "not a url");
}

fn verdict_check(status: CheckStatus, critical: bool, kind: Option<ErrorKind>) -> RpcCheck {
    RpcCheck {
        category: CheckCategory::Core,
        method: "m",
        status,
        latency_ms: Some(10),
        detail: String::new(),
        error_kind: kind,
        critical,
    }
}

#[test]
fn verdict_summary_and_threshold_branches() {
    let pass = vec![verdict_check(CheckStatus::Success, true, None)];

    // summarize: every verdict / sub-branch, including the latency-driven and
    // Unknown cases that run_check cannot produce on its own.
    assert_eq!(
        summarize(Verdict::Good, &pass, Some(50), false),
        "All RPC readiness checks passed"
    );
    assert_eq!(
        summarize(Verdict::Unknown, &[], None, false),
        "Not enough data to produce a reliable verdict"
    );
    let one_fail = vec![verdict_check(
        CheckStatus::Failed,
        false,
        Some(ErrorKind::RpcError),
    )];
    assert!(summarize(Verdict::Warning, &one_fail, Some(50), false)
        .contains("non-critical check failed"));
    let elevated = summarize(Verdict::Warning, &pass, Some(800), true);
    assert!(elevated.contains("elevated at 800 ms"));
    assert!(elevated.contains("--fail-on-warning is enabled"));
    let two_fail = vec![
        verdict_check(CheckStatus::Failed, true, None),
        verdict_check(CheckStatus::Failed, false, None),
    ];
    assert_eq!(
        summarize(Verdict::Bad, &two_fail, Some(50), false),
        "2 RPC readiness checks failed"
    );
    assert_eq!(
        summarize(Verdict::Bad, &pass, Some(2_000), false),
        "Average latency is too high at 2000 ms"
    );

    // calculate_verdict: thresholds and failure-driven branches.
    assert_eq!(calculate_verdict(&[], None), Verdict::Unknown);
    assert_eq!(calculate_verdict(&pass, Some(50)), Verdict::Good);
    assert_eq!(calculate_verdict(&pass, None), Verdict::Unknown);
    assert_eq!(calculate_verdict(&pass, Some(800)), Verdict::Warning);
    assert_eq!(calculate_verdict(&pass, Some(2_000)), Verdict::Bad);
    let critical = vec![verdict_check(CheckStatus::Failed, true, None)];
    assert_eq!(calculate_verdict(&critical, Some(50)), Verdict::Bad);
    let invalid = vec![verdict_check(
        CheckStatus::Failed,
        false,
        Some(ErrorKind::InvalidUrl),
    )];
    assert_eq!(calculate_verdict(&invalid, Some(50)), Verdict::Bad);
    let one_non_critical = vec![
        verdict_check(CheckStatus::Success, true, None),
        verdict_check(CheckStatus::Failed, false, Some(ErrorKind::RpcError)),
    ];
    assert_eq!(
        calculate_verdict(&one_non_critical, Some(50)),
        Verdict::Warning
    );
    let timeouts = vec![
        verdict_check(CheckStatus::Failed, false, Some(ErrorKind::Timeout)),
        verdict_check(CheckStatus::Failed, false, Some(ErrorKind::Timeout)),
    ];
    assert_eq!(calculate_verdict(&timeouts, Some(50)), Verdict::Bad);
}

#[test]
fn ws_verbose_shows_full_rpc_url() {
    let report = WsReport {
        verdict: Verdict::Good,
        rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
        ws_url: "wss://api.mainnet-beta.solana.com/".to_string(),
        connected: true,
        connect_latency_ms: Some(40),
        subscription_method: "slotSubscribe",
        subscribed: true,
        time_to_first_notification_ms: Some(120),
        first_slot: Some(100),
        unsubscribed: true,
        closed_cleanly: true,
        summary: "WebSocket readiness checks passed".to_string(),
        notes: Vec::new(),
    };
    // Default hides the full URL behind a hostname label; verbose shows it.
    let concise = ws_render_human(&report, plain(), false);
    assert!(concise.contains("api.mainnet-beta.solana.com"));
    assert!(!concise.contains("https://api.mainnet-beta.solana.com/"));
    let verbose = ws_render_human(&report, plain(), true);
    assert!(verbose.contains("https://api.mainnet-beta.solana.com/"));
}

#[test]
fn compare_recommendation_falls_back_when_best_index_has_no_endpoint() {
    // A defensive path: a best index that does not match any endpoint falls back
    // to printing the bare index.
    let report = CompareReport {
        profile: CompareProfileSummary::Bot,
        endpoints: vec![CompareEndpoint {
            index: 1,
            url: "https://api.mainnet-beta.solana.com/".to_string(),
            genesis_hash: None,
            verdict: Verdict::Good,
            score: 90,
            slot: Some(100),
            slot_lag: Some(0),
            average_latency_ms: Some(20),
            failed_checks: Vec::new(),
            blockhash_valid: true,
            notes: Vec::new(),
        }],
        best_endpoint_index: Some(99),
        worst_endpoint_index: Some(1),
        network_mismatch: false,
        mismatch_reason: None,
        recommendation: "Best RPC: RPC #99\nWorst RPC: RPC #1\nUse RPC #99.".to_string(),
    };
    let human = render_compare_human(&report, plain(), false);
    assert!(human.contains("Best RPC: #99 · 99"));
    assert!(human.contains("Use RPC #99."));
}

/// Remove ANSI SGR escape sequences (`\x1b[ ... m`) so visible column positions
/// can be measured regardless of color.
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for esc in chars.by_ref() {
                if esc == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn line_with<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line containing {needle:?}"))
}

fn col(text: &str, row_needle: &str, col_needle: &str) -> usize {
    line_with(text, row_needle)
        .find(col_needle)
        .unwrap_or_else(|| panic!("{col_needle:?} not found in row {row_needle:?}"))
}

#[test]
fn human_output_columns_align_with_and_without_color() {
    let check = compare_check_report(
        "https://api.mainnet-beta.solana.com/",
        Verdict::Good,
        Some(347_000_000),
        Some(12),
        true,
        &[],
    );
    let compare = build_compare_report(
        CompareProfile::Bot,
        &[
            compare_check_report(
                "https://api.mainnet-beta.solana.com/",
                Verdict::Good,
                Some(347_000_000),
                Some(12),
                true,
                &[],
            ),
            compare_check_report(
                "https://solana-rpc.publicnode.com/",
                Verdict::Good,
                Some(347_000_050),
                Some(102),
                true,
                &[],
            ),
        ],
    );
    let ws = WsReport {
        verdict: Verdict::Good,
        rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
        ws_url: "wss://api.mainnet-beta.solana.com/".to_string(),
        connected: true,
        connect_latency_ms: Some(60),
        subscription_method: "slotSubscribe",
        subscribed: true,
        time_to_first_notification_ms: Some(269),
        first_slot: Some(424_146_684),
        unsubscribed: true,
        closed_cleanly: true,
        summary: "WebSocket readiness checks passed".to_string(),
        notes: Vec::new(),
    };

    // Alignment must hold whether color is on (after stripping ANSI) or off.
    for palette in [plain(), colored()] {
        // check: the Status column starts at the same index on every row.
        let out = strip_ansi(&render_human(&check, palette, false));
        let status = col(&out, "Category", "Status");
        assert_eq!(col(&out, "Core", "PASS"), status);
        assert_eq!(col(&out, "Blockhash", "PASS"), status);
        assert_eq!(col(&out, "Performance", "PASS"), status);

        // ws: the Status and Detail columns line up across every step.
        let out = strip_ansi(&ws_render_human(&ws, palette, false));
        let status = col(&out, "Check ", "Status");
        let detail = col(&out, "Check ", "Detail");
        assert_eq!(col(&out, "Connect", "PASS"), status);
        assert_eq!(col(&out, "First notification", "PASS"), status);
        assert_eq!(col(&out, "Close", "PASS"), status);
        assert_eq!(col(&out, "Connect", "60 ms"), detail);
        assert_eq!(col(&out, "First notification", "269 ms"), detail);

        // compare: the Verdict column lines up across every endpoint.
        let out = strip_ansi(&render_compare_human(&compare, palette, false));
        let verdict = col(&out, "Endpoint", "Verdict");
        assert_eq!(col(&out, "#1 ", "GOOD"), verdict);
        assert_eq!(col(&out, "#2 ", "GOOD"), verdict);

        // No tabs are ever used for alignment.
        assert!(!render_human(&check, palette, false).contains('\t'));
        assert!(!ws_render_human(&ws, palette, false).contains('\t'));
        assert!(!render_compare_human(&compare, palette, false).contains('\t'));
    }

    // Machine formats never carry ANSI, even rendered from the same reports.
    assert!(!render_json(&check).unwrap().contains('\x1b'));
    assert!(!render_compare_json(&compare).unwrap().contains('\x1b'));
    assert!(!ws_render_json(&ws).unwrap().contains('\x1b'));
    assert!(!render_markdown(&compare).contains('\x1b'));
    assert!(!render_markdown(&compare).contains('\t'));
}

#[test]
fn latency_stats_percentiles_and_empty() {
    assert_eq!(LatencyStats::from_samples(&[]), None);

    let single = LatencyStats::from_samples(&[42]).unwrap();
    assert_eq!(single.count, 1);
    assert_eq!(single.min_ms, 42);
    assert_eq!(single.p50_ms, 42);
    assert_eq!(single.p95_ms, 42);
    assert_eq!(single.max_ms, 42);

    // Unsorted 20..=1; nearest-rank p50 = 10th value, p95 = 19th value.
    let samples: Vec<u128> = (1..=20).rev().collect();
    let stats = LatencyStats::from_samples(&samples).unwrap();
    assert_eq!(stats.count, 20);
    assert_eq!(stats.min_ms, 1);
    assert_eq!(stats.max_ms, 20);
    assert_eq!(stats.p50_ms, 10);
    assert_eq!(stats.p95_ms, 19);
}

#[tokio::test]
async fn check_with_samples_probes_latency() {
    // 7 normal check responses, then `samples` extra getHealth responses for the
    // latency probe.
    let mut responses = healthy_rpc_responses(347_000_000);
    for _ in 0..5 {
        responses.push(MockResponse::ok(health_ok()));
    }
    let server = MockRpcServer::start(responses);

    let mut args = args_for(server.url.clone());
    args.samples = 5;
    let report = run_check(args).await.unwrap();
    server.join();

    assert_eq!(report.verdict, Verdict::Good);
    let stats = report.latency_samples.expect("samples were requested");
    assert_eq!(stats.count, 5);
    assert!(stats.min_ms <= stats.p50_ms);
    assert!(stats.p50_ms <= stats.p95_ms);
    assert!(stats.p95_ms <= stats.max_ms);

    // Concise output shows p50/p95; verbose adds min/max; JSON carries the object.
    let concise = render_human(&report, plain(), false);
    assert!(concise.contains("Samples"));
    assert!(concise.contains("p50"));
    assert!(concise.contains("p95"));
    let verbose = render_human(&report, plain(), true);
    assert!(verbose.contains("min"));
    assert!(verbose.contains("max"));
    let json = render_json(&report).unwrap();
    assert!(json.contains("\"latency_samples\""));
    assert!(json.contains("\"p95_ms\""));
}

#[tokio::test]
async fn check_without_samples_has_no_latency_samples() {
    let server = MockRpcServer::start(healthy_rpc_responses(347_000_000));
    let report = run_check(args_for(server.url.clone())).await.unwrap();
    server.join();
    assert!(report.latency_samples.is_none());
    assert!(!render_human(&report, plain(), false).contains("Samples"));
}
