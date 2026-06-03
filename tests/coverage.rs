use solana_infra_doctor::{
    checks::{calculate_verdict, run_check, CheckCategory, CheckStatus, ErrorKind, RpcCheck},
    cli::CheckArgs,
    latency::{average_latency_ms, Latency},
    report::{render_human, render_json},
    rpc::{
        BlockhashValidResponse, JsonRpcRequest, LatestBlockhashResponse, PerformanceSample,
        RpcEndpoint,
    },
    verdict::Verdict,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
