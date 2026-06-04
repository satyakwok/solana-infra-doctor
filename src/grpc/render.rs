//! Human-terminal, JSON, and Markdown rendering for the gRPC readiness report.
//! Mirrors the `check`/`ws` renderers: a concise default view, a `--verbose`
//! expansion, and a Markdown report. None of these ever surface the token.

use super::{CheckStatus, GrpcReport};
use crate::{
    color::Palette,
    error::AppError,
    output::{
        style,
        table::{self, Cell},
    },
};
use std::fs;

/// Render the human-readable `grpc check` report.
///
/// The default view is concise (result roll-up plus a per-category table).
/// `verbose` adds the full redacted endpoint, per-method unary detail, the
/// cross-check, warnings, and remediation hints. `verbose` affects human output
/// only.
pub fn render_human(report: &GrpcReport, palette: Palette, verbose: bool) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor · Yellowstone gRPC Readiness"));
    output.push_str("\n\n");

    // Target.
    output.push_str(&palette.heading("Target"));
    output.push('\n');
    let endpoint_value = if verbose {
        report.grpc_endpoint.clone()
    } else {
        style::endpoint_label(&report.grpc_endpoint)
    };
    let mut target_rows = vec![vec![
        Cell::styled("Endpoint", palette.label("Endpoint")),
        Cell::plain(endpoint_value),
    ]];
    if let Some(rpc) = &report.rpc_endpoint {
        let rpc_value = if verbose {
            rpc.clone()
        } else {
            style::endpoint_label(rpc)
        };
        target_rows.push(vec![
            Cell::styled("RPC", palette.label("RPC")),
            Cell::plain(rpc_value),
        ]);
    }
    output.push_str(&table::render(&target_rows, 3));
    output.push('\n');

    // Result roll-up.
    output.push_str(&palette.heading("Result"));
    output.push('\n');
    let mut result_rows = vec![vec![
        Cell::styled(report.verdict.to_string(), palette.verdict(report.verdict)),
        Cell::plain(report.summary.clone()),
    ]];
    if let Some(ms) = report.connect_latency_ms {
        result_rows.push(label_value(palette, "Connect", style::millis(ms)));
    }
    let passed = count_status(report, CheckStatus::Pass);
    let failed = count_status(report, CheckStatus::Fail);
    result_rows.push(label_value(
        palette,
        "Unary",
        format!("{passed} passed · {failed} failed"),
    ));
    if !report.stream.detail.is_empty() {
        result_rows.push(label_value(palette, "Stream", report.stream.detail.clone()));
    }
    if let Some(slot) = report.latest_slot {
        result_rows.push(label_value(palette, "Latest slot", group_thousands(slot)));
    }
    if let Some(diff) = report.slot_difference {
        result_rows.push(label_value(
            palette,
            "Slot diff",
            format!("{diff} vs HTTP RPC"),
        ));
    }
    output.push_str(&table::render(&result_rows, 3));
    output.push('\n');

    // Category checks table.
    output.push_str(&palette.heading("Checks"));
    output.push('\n');
    let mut rows = vec![vec![
        Cell::styled("Category", palette.label("Category")),
        Cell::styled("Status", palette.label("Status")),
        Cell::styled("Summary", palette.label("Summary")),
    ]];
    for check in &report.checks {
        let status = check.status.display();
        rows.push(vec![
            Cell::plain(check.category.label().to_string()),
            Cell::styled(status.label(), status.paint(palette)),
            Cell::plain(check.summary.clone()),
        ]);
    }
    output.push_str(&table::render(&rows, 4));

    if verbose {
        render_verbose(report, palette, &mut output);
    } else {
        // Warnings matter even in the concise view.
        if !report.warnings.is_empty() {
            output.push('\n');
            output.push_str(&palette.heading("Warnings"));
            output.push('\n');
            for warning in &report.warnings {
                output.push_str(&format!("- {warning}\n"));
            }
        }
        output.push('\n');
        output.push_str(&palette.dim("Tip: run with --verbose to see full details."));
        output.push('\n');
    }

    output
}

fn render_verbose(report: &GrpcReport, palette: Palette, output: &mut String) {
    // Per-method unary detail.
    if !report.unary.is_empty() {
        output.push('\n');
        output.push_str(&palette.heading("Unary methods"));
        output.push('\n');
        let mut rows = vec![vec![
            Cell::styled("Method", palette.label("Method")),
            Cell::styled("Status", palette.label("Status")),
            Cell::styled("Latency", palette.label("Latency")),
            Cell::styled("Detail", palette.label("Detail")),
        ]];
        for unary in &report.unary {
            let status = unary.status.display();
            let latency = unary.latency_ms.map(style::millis).unwrap_or_default();
            let detail = match unary.error_kind {
                Some(kind) if unary.status != CheckStatus::Pass => {
                    format!("{} [{}]", unary.detail, kind)
                }
                _ => unary.detail.clone(),
            };
            rows.push(vec![
                Cell::plain(unary.method.to_string()),
                Cell::styled(status.label(), status.paint(palette)),
                Cell::plain(latency),
                Cell::plain(detail),
            ]);
        }
        output.push_str(&table::render(&rows, 3));
    }

    // Cross-check detail.
    if let (Some(rpc_slot), Some(latest)) = (report.rpc_slot, report.latest_slot) {
        output.push('\n');
        output.push_str(&palette.heading("Cross-check"));
        output.push('\n');
        output.push_str(&table::render(
            &[
                label_value(palette, "gRPC slot", group_thousands(latest)),
                label_value(palette, "HTTP RPC slot", group_thousands(rpc_slot)),
                label_value(
                    palette,
                    "Difference",
                    report
                        .slot_difference
                        .map(|d| d.to_string())
                        .unwrap_or_default(),
                ),
            ],
            3,
        ));
    }

    if !report.warnings.is_empty() {
        output.push('\n');
        output.push_str(&palette.heading("Warnings"));
        output.push('\n');
        for warning in &report.warnings {
            output.push_str(&format!("- {warning}\n"));
        }
    }

    if !report.remediation.is_empty() {
        output.push('\n');
        output.push_str(&palette.heading("Next checks"));
        output.push('\n');
        for hint in &report.remediation {
            output.push_str(&format!("- {hint}\n"));
        }
    }
}

/// Serialize the report as pretty JSON. The shape never contains the token.
pub fn render_json(report: &GrpcReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

/// Render the Markdown report.
pub fn render_markdown(report: &GrpcReport) -> String {
    let mut output = String::new();
    output.push_str("# Solana Infra Doctor — Yellowstone gRPC Readiness Report\n\n");
    output.push_str(&format!("- Endpoint: `{}`\n", report.grpc_endpoint));
    if let Some(rpc) = &report.rpc_endpoint {
        output.push_str(&format!("- HTTP RPC: `{rpc}`\n"));
    }
    output.push_str(&format!("- Verdict: `{}`\n", report.verdict));
    output.push_str(&format!("- Summary: {}\n", report.summary));
    output.push_str(&format!(
        "- Token provided: {}\n",
        if report.token_provided { "yes" } else { "no" }
    ));
    if let Some(ms) = report.connect_latency_ms {
        output.push_str(&format!("- Connect latency: {ms} ms\n"));
    }
    output.push('\n');

    output.push_str("## Checks\n\n");
    output.push_str("| Category | Status | Summary |\n| --- | --- | --- |\n");
    for check in &report.checks {
        output.push_str(&format!(
            "| {} | `{}` | {} |\n",
            check.category.label(),
            check.status.display().label(),
            check.summary
        ));
    }
    output.push('\n');

    output.push_str("## Unary methods\n\n");
    output.push_str("| Method | Status | Latency | Detail |\n| --- | --- | --- | --- |\n");
    for unary in &report.unary {
        let latency = unary
            .latency_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "—".to_string());
        output.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            unary.method,
            unary.status.display().label(),
            latency,
            unary.detail
        ));
    }
    output.push('\n');

    output.push_str("## Slot stream\n\n");
    output.push_str(&format!(
        "- Status: `{}`\n",
        report.stream.status.display().label()
    ));
    if let Some(ms) = report.stream.first_event_latency_ms {
        output.push_str(&format!("- Time to first slot update: {ms} ms\n"));
    }
    output.push_str(&format!(
        "- Updates observed: {}\n",
        report.stream.updates_observed
    ));
    if let Some(slot) = report.latest_slot {
        output.push_str(&format!("- Latest observed slot: {slot}\n"));
    }
    output.push('\n');

    if report.rpc_endpoint.is_some() {
        output.push_str("## HTTP RPC cross-check\n\n");
        match (report.latest_slot, report.rpc_slot, report.slot_difference) {
            (Some(grpc), Some(rpc), Some(diff)) => {
                output.push_str(&format!("- gRPC slot: {grpc}\n"));
                output.push_str(&format!("- HTTP RPC slot: {rpc}\n"));
                output.push_str(&format!("- Difference: {diff}\n"));
            }
            _ => output.push_str("- Cross-check could not be completed.\n"),
        }
        output.push('\n');
    }

    if !report.warnings.is_empty() {
        output.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            output.push_str(&format!("- {warning}\n"));
        }
        output.push('\n');
    }

    if !report.remediation.is_empty() {
        output.push_str("## Remediation\n\n");
        for hint in &report.remediation {
            output.push_str(&format!("- {hint}\n"));
        }
        output.push('\n');
    }

    output.push_str("## Disclaimer\n\n");
    output.push_str(
        "This report is a point-in-time diagnostic snapshot. It is not an SLA, security audit, or guarantee of future endpoint performance.\n\n",
    );
    output.push_str(
        "Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.\n",
    );
    output
}

/// Render the Markdown report and write it to `path`.
pub fn write_markdown_report(report: &GrpcReport, path: &std::path::Path) -> Result<(), AppError> {
    fs::write(path, render_markdown(report)).map_err(|source| AppError::WriteMarkdownReport {
        path: path.display().to_string(),
        source,
    })
}

fn label_value(palette: Palette, label: &str, value: String) -> Vec<Cell> {
    vec![
        Cell::styled(label, palette.label(label)),
        Cell::plain(value),
    ]
}

fn count_status(report: &GrpcReport, status: CheckStatus) -> usize {
    report.unary.iter().filter(|u| u.status == status).count()
}

/// Group an integer with thousands separators: `424000123` → `424,000,123`.
fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let bytes = digits.as_bytes();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(*byte as char);
    }
    grouped
}

#[cfg(all(test, not(coverage)))]
mod tests {
    use super::*;
    use crate::color::Palette;
    use crate::grpc::{
        AuthStatus, CategoryCheck, GrpcCategory, GrpcErrorKind, StreamResult, UnaryResult,
        GRPC_SCHEMA_VERSION,
    };
    use crate::verdict::Verdict;

    #[test]
    fn groups_thousands() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(123), "123");
        assert_eq!(group_thousands(424_000_123), "424,000,123");
        assert_eq!(group_thousands(1_000), "1,000");
    }

    /// A representative healthy report with a cross-check, a degraded unary
    /// method, warnings, and remediation — exercises most render branches.
    fn sample_report() -> GrpcReport {
        GrpcReport {
            schema_version: GRPC_SCHEMA_VERSION,
            verdict: Verdict::Warning,
            summary: "gRPC endpoint is streaming, with some degraded checks".to_string(),
            grpc_endpoint: "https://grpc.example.com/".to_string(),
            rpc_endpoint: Some("https://rpc.example.com/".to_string()),
            token_provided: true,
            connect_latency_ms: Some(42),
            authentication: AuthStatus::Accepted,
            unary: vec![
                UnaryResult {
                    method: "Ping",
                    status: CheckStatus::Pass,
                    latency_ms: Some(6),
                    detail: "pong".to_string(),
                    error_kind: None,
                },
                UnaryResult {
                    method: "GetSlot",
                    status: CheckStatus::Fail,
                    latency_ms: Some(7),
                    detail: "slot source draining".to_string(),
                    error_kind: Some(GrpcErrorKind::Unavailable),
                },
                UnaryResult {
                    method: "IsBlockhashValid",
                    status: CheckStatus::Skip,
                    latency_ms: None,
                    detail: "skipped".to_string(),
                    error_kind: None,
                },
            ],
            stream: StreamResult {
                status: CheckStatus::Pass,
                opened: true,
                first_event_latency_ms: Some(318),
                updates_observed: 3,
                latest_slot: Some(424_000_123),
                detail: "first slot update in 318 ms".to_string(),
                error_kind: None,
            },
            latest_slot: Some(424_000_123),
            rpc_slot: Some(424_000_120),
            slot_difference: Some(3),
            checks: vec![
                CategoryCheck {
                    category: GrpcCategory::Transport,
                    status: CheckStatus::Pass,
                    summary: "Connected over TLS (HTTP/2)".to_string(),
                },
                CategoryCheck {
                    category: GrpcCategory::Unary,
                    status: CheckStatus::Warn,
                    summary: "5 / 6 supported checks passed".to_string(),
                },
            ],
            warnings: vec!["1 unary method check(s) failed".to_string()],
            remediation: vec!["inspect the endpoint".to_string()],
            error_kinds: vec![GrpcErrorKind::Unavailable],
        }
    }

    #[test]
    fn human_concise_has_key_sections_and_no_color_when_disabled() {
        let report = sample_report();
        let out = render_human(&report, Palette::new(false), false);
        assert!(out.contains("Yellowstone gRPC Readiness"));
        assert!(out.contains("WARNING"));
        assert!(out.contains("Latest slot"));
        assert!(out.contains("424,000,123"));
        assert!(out.contains("Tip: run with --verbose"));
        // Concise hostname label, not the full URL.
        assert!(out.contains("grpc.example.com"));
        // No ANSI escapes when the palette is disabled.
        assert!(!out.contains('\u{1b}'));
    }

    #[test]
    fn human_verbose_shows_methods_and_remediation() {
        let report = sample_report();
        let out = render_human(&report, Palette::new(false), true);
        assert!(out.contains("Unary methods"));
        assert!(out.contains("GetSlot"));
        // Failed methods annotate the error kind.
        assert!(out.contains("[unavailable]"));
        assert!(out.contains("Cross-check"));
        assert!(out.contains("Next checks"));
        // Verbose shows the full redacted URL.
        assert!(out.contains("https://grpc.example.com/"));
        // No verbose tip line.
        assert!(!out.contains("Tip: run with --verbose"));
    }

    #[test]
    fn json_is_stable_and_secret_free() {
        let report = sample_report();
        let json = render_json(&report).unwrap();
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"verdict\": \"WARNING\""));
        assert!(json.contains("\"unavailable\""));
        // The token is never serialized.
        assert!(!json.contains("x-token"));
        assert!(!json.contains("token_value"));
    }

    #[test]
    fn markdown_has_all_sections_and_disclaimer() {
        let report = sample_report();
        let md = render_markdown(&report);
        assert!(md.contains("# Solana Infra Doctor — Yellowstone gRPC Readiness Report"));
        assert!(md.contains("## Checks"));
        assert!(md.contains("## Unary methods"));
        assert!(md.contains("## Slot stream"));
        assert!(md.contains("## HTTP RPC cross-check"));
        assert!(md.contains("## Warnings"));
        assert!(md.contains("## Remediation"));
        assert!(md.contains("point-in-time diagnostic snapshot"));
        assert!(md.contains("not affiliated with or endorsed by Solana Foundation"));
    }

    #[test]
    fn enabled_palette_colorizes_human_output() {
        let report = sample_report();
        let out = render_human(&report, Palette::new(true), false);
        assert!(out.contains('\u{1b}'));
    }

    #[test]
    fn minimal_bad_report_renders_without_panic() {
        let mut report = sample_report();
        report.verdict = Verdict::Bad;
        report.rpc_endpoint = None;
        report.rpc_slot = None;
        report.slot_difference = None;
        report.latest_slot = None;
        report.unary.clear();
        report.warnings.clear();
        report.remediation.clear();
        let _ = render_human(&report, Palette::new(false), true);
        let _ = render_human(&report, Palette::new(false), false);
        let _ = render_markdown(&report);
        let _ = render_json(&report).unwrap();
    }
}
