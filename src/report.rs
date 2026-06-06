use crate::{
    checks::{CheckCategory, CheckReport, CheckStatus, ProgramAccountsReadiness, RpcCheck},
    color::Palette,
    error::AppError,
    output::{
        style::{self, Status},
        table::{self, Cell},
    },
};

const CATEGORY_ORDER: [CheckCategory; 5] = [
    CheckCategory::Core,
    CheckCategory::Blockhash,
    CheckCategory::Performance,
    CheckCategory::Token,
    CheckCategory::Data,
];

/// Print the machine-readable JSON report to stdout.
pub fn print_json(report: &CheckReport) -> Result<(), AppError> {
    let json = render_json(report)?;
    println!("{json}");
    Ok(())
}

/// Render the human-readable `check` report.
///
/// The default view is a concise summary: a safe endpoint label, the overall
/// verdict, average latency, and a per-category pass/fail table. `verbose` adds
/// the full per-check detail — the full redacted URL, every method's latency,
/// full hashes, and error kinds. `verbose` affects human output only.
pub fn render_human(report: &CheckReport, palette: Palette, verbose: bool) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor · RPC Readiness"));
    output.push_str("\n\n");

    // Target: a safe hostname by default; the full redacted URL in verbose.
    output.push_str(&palette.heading("Target"));
    output.push('\n');
    let (target_label, target_value) = if verbose {
        ("RPC URL", report.rpc_url.clone())
    } else {
        ("Endpoint", style::endpoint_label(&report.rpc_url))
    };
    output.push_str(&table::render(
        &[vec![
            Cell::styled(target_label, palette.label(target_label)),
            Cell::plain(target_value),
        ]],
        3,
    ));
    output.push('\n');

    // Result: verdict, average latency, and pass/fail counts.
    output.push_str(&palette.heading("Result"));
    output.push('\n');
    let average = report.average_latency_ms.map_or_else(
        || "n/a".to_string(),
        |value| format!("{} average", style::millis(value)),
    );
    let passed = report
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Success)
        .count();
    let failed = report.checks.len() - passed;
    let mut result_rows = vec![
        vec![
            Cell::styled(report.verdict.to_string(), palette.verdict(report.verdict)),
            Cell::plain(report.summary.clone()),
        ],
        vec![
            Cell::styled("Latency", palette.label("Latency")),
            Cell::plain(average),
        ],
        vec![
            Cell::styled("Checks", palette.label("Checks")),
            Cell::plain(format!("{passed} passed · {failed} failed")),
        ],
    ];
    if let Some(stats) = &report.latency_samples {
        let detail = if verbose {
            format!(
                "p50 {} · p95 {} · min {} · max {} ({} runs)",
                style::millis(stats.p50_ms),
                style::millis(stats.p95_ms),
                style::millis(stats.min_ms),
                style::millis(stats.max_ms),
                stats.count
            )
        } else {
            format!(
                "p50 {} · p95 {} ({} runs)",
                style::millis(stats.p50_ms),
                style::millis(stats.p95_ms),
                stats.count
            )
        };
        result_rows.push(vec![
            Cell::styled("Samples", palette.label("Samples")),
            Cell::plain(detail),
        ]);
    }
    if let Some(lag) = report.block_time_lag_secs {
        result_rows.push(vec![
            Cell::styled("Block time", palette.label("Block time")),
            Cell::plain(format!("{lag}s behind (finalized)")),
        ]);
    }
    if let Some(fee) = report.prioritization_fee_median {
        result_rows.push(vec![
            Cell::styled("Fee market", palette.label("Fee market")),
            Cell::plain(format!("median {fee} micro-lamports/CU")),
        ]);
    }
    if report
        .checks
        .iter()
        .any(|check| check.category == CheckCategory::Token)
    {
        let ready = |ready: bool| if ready { "ready" } else { "not ready" };
        result_rows.push(vec![
            Cell::styled("Token", palette.label("Token")),
            Cell::plain(format!(
                "Token Program {} · Token-2022 {}",
                ready(report.token_program_ready),
                ready(report.token_2022_ready)
            )),
        ]);
    }
    if let Some(readiness) = report.program_accounts {
        let gpa = match readiness {
            ProgramAccountsReadiness::Ready => "getProgramAccounts ready",
            ProgramAccountsReadiness::Gated => "getProgramAccounts gated",
            ProgramAccountsReadiness::Degraded => "getProgramAccounts degraded",
        };
        let archival = match report.oldest_available_slot {
            Some(0) => " · history full (from genesis)".to_string(),
            Some(oldest) => format!(" · history from slot {oldest}"),
            None => String::new(),
        };
        result_rows.push(vec![
            Cell::styled("Data", palette.label("Data")),
            Cell::plain(format!("{gpa}{archival}")),
        ]);
    }
    output.push_str(&table::render(&result_rows, 3));
    output.push('\n');

    if verbose {
        render_checks_verbose(report, palette, &mut output);
    } else {
        render_checks_summary(report, palette, &mut output);
        output.push('\n');
        output.push_str(&palette.dim("Tip: run with --verbose to see full details."));
        output.push('\n');
    }

    output
}

/// Per-category status for the concise summary: `PASS` when every member check
/// passed, `FAIL` when a critical check failed, otherwise `WARN` (only
/// non-critical checks failed). Presentation only — derived from existing
/// results, it never changes the verdict.
fn category_status(checks: &[&RpcCheck]) -> Status {
    if checks
        .iter()
        .all(|check| check.status == CheckStatus::Success)
    {
        Status::Pass
    } else if checks
        .iter()
        .any(|check| check.critical && check.status == CheckStatus::Failed)
    {
        Status::Fail
    } else {
        Status::Warn
    }
}

fn render_checks_summary(report: &CheckReport, palette: Palette, output: &mut String) {
    output.push_str(&palette.heading("Checks"));
    output.push('\n');
    let mut rows = vec![vec![
        Cell::styled("Category", palette.label("Category")),
        Cell::styled("Status", palette.label("Status")),
        Cell::styled("Summary", palette.label("Summary")),
    ]];
    for category in CATEGORY_ORDER {
        let checks: Vec<_> = report
            .checks
            .iter()
            .filter(|check| check.category == category)
            .collect();
        if checks.is_empty() {
            continue;
        }
        let total = checks.len();
        let passed = checks
            .iter()
            .filter(|check| check.status == CheckStatus::Success)
            .count();
        let status = category_status(&checks);
        rows.push(vec![
            Cell::plain(category.label()),
            Cell::styled(status.label(), status.paint(palette)),
            Cell::plain(format!("{passed} / {total}")),
        ]);
    }
    output.push_str(&table::render(&rows, 4));
}

fn render_checks_verbose(report: &CheckReport, palette: Palette, output: &mut String) {
    output.push_str(&palette.heading("Checks"));
    output.push('\n');
    if report.fail_on_warning {
        output.push_str(
            &palette.dim("Warning policy: --fail-on-warning is enabled; WARNING exits 1 for CI."),
        );
        output.push('\n');
    }
    for category in CATEGORY_ORDER {
        let checks: Vec<_> = report
            .checks
            .iter()
            .filter(|check| check.category == category)
            .collect();
        if checks.is_empty() {
            continue;
        }
        output.push('\n');
        output.push_str(&palette.heading(category.label()));
        output.push('\n');
        let rows: Vec<Vec<Cell>> = checks
            .iter()
            .map(|check| {
                let status = match check.status {
                    CheckStatus::Success => Status::Pass,
                    CheckStatus::Failed => Status::Fail,
                };
                let latency = check
                    .latency_ms
                    .map_or_else(|| "n/a".to_string(), style::millis);
                let detail = match check.error_kind {
                    Some(kind) => Cell::styled(
                        format!("{} [{}]", check.detail, kind.label()),
                        format!(
                            "{}{}",
                            check.detail,
                            palette.dim(&format!(" [{}]", kind.label()))
                        ),
                    ),
                    None => Cell::plain(check.detail.clone()),
                };
                vec![
                    Cell::plain(format!("- {}", check.method)),
                    Cell::styled(status.label(), status.paint(palette)),
                    Cell::plain(latency),
                    detail,
                ]
            })
            .collect();
        output.push_str(&table::render(&rows, 2));
    }
}

/// Serialize a check report to pretty-printed JSON.
pub fn render_json(report: &CheckReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;
    use crate::{
        checks::{ErrorKind, RpcCheck},
        verdict::Verdict,
    };
    use serde_json::Value;

    fn report() -> CheckReport {
        CheckReport {
            schema_version: 1,
            verdict: Verdict::Good,
            rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
            summary: "all RPC readiness checks succeeded".to_string(),
            average_latency_ms: Some(100),
            latency_samples: None,
            block_time_lag_secs: None,
            prioritization_fee_median: None,
            token_program_ready: true,
            token_2022_ready: true,
            program_accounts: None,
            oldest_available_slot: None,
            archival_depth_slots: None,
            fail_on_warning: true,
            checks: vec![
                RpcCheck {
                    category: CheckCategory::Core,
                    method: "getHealth",
                    status: CheckStatus::Success,
                    latency_ms: Some(100),
                    detail: "health is ok".to_string(),
                    error_kind: None,
                    critical: true,
                },
                RpcCheck {
                    category: CheckCategory::Blockhash,
                    method: "isBlockhashValid",
                    status: CheckStatus::Success,
                    latency_ms: Some(80),
                    detail: "latest blockhash is valid".to_string(),
                    error_kind: None,
                    critical: true,
                },
                RpcCheck {
                    category: CheckCategory::Performance,
                    method: "getRecentPerformanceSamples",
                    status: CheckStatus::Failed,
                    latency_ms: Some(90),
                    detail: "RPC error -32000: unavailable".to_string(),
                    error_kind: Some(ErrorKind::RpcError),
                    critical: false,
                },
            ],
        }
    }

    #[test]
    fn concise_output_summarizes_by_category_and_hides_detail() {
        let rendered = render_human(&report(), Palette::new(false), false);
        assert!(rendered.contains("Solana Infra Doctor · RPC Readiness"));
        assert!(rendered.contains("GOOD"));
        assert!(rendered.contains("Core"));
        assert!(rendered.contains("Blockhash"));
        assert!(rendered.contains("Performance"));
        assert!(rendered.contains("Tip: run with --verbose"));
        // Concise output hides per-method detail and the fail-on-warning note.
        assert!(!rendered.contains("health is ok"));
        assert!(!rendered.contains("--fail-on-warning"));
    }

    #[test]
    fn verbose_output_shows_per_check_detail() {
        let rendered = render_human(&report(), Palette::new(false), true);
        assert!(rendered.contains("getHealth"));
        assert!(rendered.contains("PASS"));
        assert!(rendered.contains("health is ok"));
        assert!(rendered.contains("100 ms"));
        assert!(rendered.contains("--fail-on-warning is enabled"));
    }

    #[test]
    fn renders_json_report_shape() {
        let rendered = render_json(&report()).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed["verdict"], "GOOD");
        assert_eq!(parsed["average_latency_ms"], 100);
        assert_eq!(parsed["fail_on_warning"], true);
        assert_eq!(parsed["checks"][0]["category"], "core");
        assert_eq!(parsed["checks"][0]["method"], "getHealth");
        assert_eq!(parsed["checks"][0]["status"], "success");
        assert_eq!(parsed["checks"][2]["error_kind"], "rpc_error");
        assert_eq!(parsed["checks"][2]["critical"], false);
    }
}
