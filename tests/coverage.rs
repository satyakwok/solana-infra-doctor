use solana_infra_doctor::{
    checks::{calculate_verdict, run_check, CheckCategory, CheckStatus, ErrorKind, RpcCheck},
    cli::{CheckArgs, CompareArgs, CompareProfile},
    compare::{
        build_compare_report, render_human as render_compare_human,
        render_json as render_compare_json, render_markdown, run_compare, score_endpoint, slot_lag,
        write_markdown_report, CompareEndpoint, CompareProfileSummary,
    },
    latency::{average_latency_ms, Latency},
    report::{render_human, render_json},
    rpc::{
        BlockhashValidResponse, JsonRpcRequest, LatestBlockhashResponse, PerformanceSample,
        RpcEndpoint,
    },
    verdict::Verdict,
};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread::{self, JoinHandle},
    time::Duration,
};

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
    assert_eq!(report.summary, "all RPC readiness checks succeeded");
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
        fail_on_warning: true,
        checks: vec![success, failed],
    };
    let human = render_human(&report);
    assert!(human.contains("Average latency: n/a"));
    assert!(human.contains("--fail-on-warning enabled"));
    assert!(human.contains("[malformed_response]"));

    let mut report_with_latency = report.clone();
    report_with_latency.average_latency_ms = Some(200);
    report_with_latency.fail_on_warning = false;
    let human_with_latency = render_human(&report_with_latency);
    assert!(human_with_latency.contains("Average latency: 200ms"));
    assert!(!human_with_latency.contains("Warning policy:"));

    let json = render_json(&report).unwrap();
    assert!(json.contains("\"verdict\": \"WARNING\""));
    solana_infra_doctor::report::print_report(&report).unwrap();
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

    let human = render_compare_human(&report);
    assert!(human.contains("Solana Infra Doctor — RPC Compare"));
    assert!(human.contains("Profile: bot"));
    assert!(human.contains("RPC #1"));
    assert!(human.contains("Slot lag: baseline"));
    assert!(human.contains("Failed checks: getRecentPerformanceSamples"));

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

    let human = render_compare_human(&report);
    assert!(human.contains("Slot: n/a"));
    assert!(human.contains("Slot lag: n/a"));
    assert!(human.contains("Average latency: n/a"));

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

    let human = render_compare_human(&report);
    assert!(human.contains("Cannot compare endpoints from different Solana networks."));
    assert!(human.contains("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"));

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
