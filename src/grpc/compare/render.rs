//! Human-terminal, JSON, and Markdown rendering for the gRPC comparison report.
//! Mirrors the `compare` renderer: a concise one-row-per-endpoint table plus the
//! recommendation, a `--verbose` per-endpoint expansion, and a Markdown report.
//! None of these ever surface a token.

use super::GrpcCompareReport;
use crate::{
    color::Palette,
    error::AppError,
    output::{
        style::{self, endpoint_label},
        table::{self, Cell},
    },
};
use std::fs;

/// Render the human-readable `grpc compare` report.
///
/// The default view is a one-row-per-endpoint summary table (verdict, score,
/// connect, first event, slot lag) plus the recommendation. `verbose` expands
/// each endpoint into a full detail block. `verbose` affects human output only.
pub fn render_human(report: &GrpcCompareReport, palette: Palette, verbose: bool) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor · Yellowstone gRPC Comparison"));
    output.push_str("\n\n");
    output.push_str(&format!(
        "{} {}\n\n",
        palette.label("Profile:"),
        report.profile.label()
    ));

    if verbose {
        render_endpoints_verbose(report, palette, &mut output);
    } else {
        render_endpoints_summary(report, palette, &mut output);
    }

    output.push_str(&palette.heading("Recommendation"));
    output.push('\n');
    output.push_str(&recommendation_block(report, &palette));
    if !verbose {
        output.push('\n');
        output.push_str(&palette.dim("Tip: run with --verbose to see full details per endpoint."));
        output.push('\n');
    }
    output
}

fn render_endpoints_summary(report: &GrpcCompareReport, palette: Palette, output: &mut String) {
    let mut rows = vec![vec![
        Cell::styled("gRPC", palette.label("gRPC")),
        Cell::styled("Endpoint", palette.label("Endpoint")),
        Cell::styled("Verdict", palette.label("Verdict")),
        Cell::styled("Score", palette.label("Score")),
        Cell::styled("Connect", palette.label("Connect")),
        Cell::styled("First event", palette.label("First event")),
        Cell::styled("Slot lag", palette.label("Slot lag")),
    ]];
    for endpoint in &report.endpoints {
        rows.push(vec![
            Cell::plain(format!("#{}", endpoint.index)),
            Cell::plain(endpoint_label(&endpoint.endpoint)),
            Cell::styled(
                endpoint.verdict.to_string(),
                palette.verdict(endpoint.verdict),
            ),
            Cell::plain(format!("{}/100", endpoint.score)),
            Cell::plain(format_latency(endpoint.connect_latency_ms)),
            Cell::plain(format_latency(endpoint.first_event_latency_ms)),
            Cell::plain(format_slot_lag_compact(endpoint.slot_lag)),
        ]);
    }
    output.push_str(&table::render(&rows, 3));
    output.push('\n');
}

fn render_endpoints_verbose(report: &GrpcCompareReport, palette: Palette, output: &mut String) {
    for endpoint in &report.endpoints {
        output.push_str(&palette.heading(&format!("gRPC #{}", endpoint.index)));
        output.push('\n');
        let rows = vec![
            detail_row(palette, "Endpoint", endpoint.endpoint.clone()),
            vec![
                Cell::styled("Verdict", palette.label("Verdict")),
                Cell::styled(
                    endpoint.verdict.to_string(),
                    palette.verdict(endpoint.verdict),
                ),
            ],
            detail_row(palette, "Score", format!("{}/100", endpoint.score)),
            detail_row(palette, "Token", format_token(endpoint.token_provided)),
            detail_row(
                palette,
                "Connect",
                format_latency(endpoint.connect_latency_ms),
            ),
            detail_row(
                palette,
                "First event",
                format_latency(endpoint.first_event_latency_ms),
            ),
            detail_row(palette, "Latest slot", format_slot(endpoint.latest_slot)),
            detail_row(palette, "Slot lag", format_slot_lag(endpoint.slot_lag)),
            detail_row(
                palette,
                "Updates observed",
                endpoint.updates_observed.to_string(),
            ),
            detail_row(
                palette,
                "Unary",
                format!(
                    "{} passed · {} failed",
                    endpoint.unary_passed, endpoint.unary_failed
                ),
            ),
            detail_row(
                palette,
                "Failed methods",
                format_failed_methods(&endpoint.failed_methods),
            ),
        ];
        output.push_str(&table::render(&rows, 3));
        if !endpoint.notes.is_empty() {
            output.push_str(&palette.label("Notes"));
            output.push('\n');
            for note in &endpoint.notes {
                output.push_str(&format!("- {note}\n"));
            }
        }
        output.push('\n');
    }
}

fn detail_row(palette: Palette, label: &str, value: String) -> Vec<Cell> {
    vec![
        Cell::styled(label.to_string(), palette.label(label)),
        Cell::plain(value),
    ]
}

/// Build the recommendation block: lead with a compact `Best gRPC: #N · host`
/// line, then the narrative with the raw `Best gRPC:` / `Worst gRPC:` lines the
/// scorer emits dropped.
fn recommendation_block(report: &GrpcCompareReport, palette: &Palette) -> String {
    let Some(best) = report.best_endpoint_index else {
        return format!("{}\n", report.recommendation);
    };
    let host = report
        .endpoints
        .iter()
        .find(|endpoint| endpoint.index == best)
        .map_or_else(
            || best.to_string(),
            |endpoint| endpoint_label(&endpoint.endpoint),
        );
    let mut output = format!("{} #{best} · {host}\n", palette.label("Best gRPC:"));
    for line in report.recommendation.lines() {
        if line.starts_with("Best gRPC:") || line.starts_with("Worst gRPC:") {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

/// Serialize a gRPC comparison report to pretty-printed JSON.
pub fn render_json(report: &GrpcCompareReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

/// Render a gRPC comparison report as a shareable Markdown document (no ANSI,
/// redacted, no token).
pub fn render_markdown(report: &GrpcCompareReport) -> String {
    let mut output = String::new();
    output.push_str("# Solana Infra Doctor Yellowstone gRPC Compare Report\n\n");
    output.push_str(&format!("Profile: `{}`\n\n", report.profile.label()));

    output.push_str("## Summary\n\n");
    output.push_str(&format!(
        "- Best gRPC: {}\n- Worst gRPC: {}\n\n",
        format_rank(report.best_endpoint_index),
        format_rank(report.worst_endpoint_index)
    ));

    output.push_str("## Comparison\n\n");
    output.push_str(
        "| gRPC | Endpoint | Verdict | Score | Connect | First event | Latest slot | Slot lag | Failed methods |\n",
    );
    output.push_str("| --- | --- | --- | ---: | --- | --- | --- | --- | --- |\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!(
            "| gRPC #{} | `{}` | `{}` | {}/100 | {} | {} | {} | {} | {} |\n",
            endpoint.index,
            endpoint.endpoint,
            endpoint.verdict,
            endpoint.score,
            format_latency(endpoint.connect_latency_ms),
            format_latency(endpoint.first_event_latency_ms),
            format_slot(endpoint.latest_slot),
            format_slot_lag(endpoint.slot_lag),
            format_failed_methods(&endpoint.failed_methods),
        ));
    }

    output.push_str("\n## Per-Endpoint Details\n\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!("### gRPC #{}\n\n", endpoint.index));
        output.push_str(&format!("- Endpoint: `{}`\n", endpoint.endpoint));
        output.push_str(&format!("- Verdict: `{}`\n", endpoint.verdict));
        output.push_str(&format!("- Score: {}/100\n", endpoint.score));
        output.push_str(&format!(
            "- Token provided: {}\n",
            if endpoint.token_provided { "yes" } else { "no" }
        ));
        output.push_str(&format!(
            "- Connect latency: {}\n",
            format_latency(endpoint.connect_latency_ms)
        ));
        output.push_str(&format!(
            "- Time to first event: {}\n",
            format_latency(endpoint.first_event_latency_ms)
        ));
        output.push_str(&format!(
            "- Latest slot: {}\n",
            format_slot(endpoint.latest_slot)
        ));
        output.push_str(&format!(
            "- Slot lag: {}\n",
            format_slot_lag(endpoint.slot_lag)
        ));
        output.push_str(&format!(
            "- Updates observed: {}\n",
            endpoint.updates_observed
        ));
        output.push_str(&format!(
            "- Unary: {} passed, {} failed\n",
            endpoint.unary_passed, endpoint.unary_failed
        ));
        output.push_str(&format!(
            "- Failed methods: {}\n",
            format_failed_methods(&endpoint.failed_methods)
        ));
        if endpoint.notes.is_empty() {
            output.push_str("- Notes: none\n\n");
        } else {
            output.push_str("- Notes:\n");
            for note in &endpoint.notes {
                output.push_str(&format!("  - {note}\n"));
            }
            output.push('\n');
        }
    }

    output.push_str("## Recommendation\n\n");
    output.push_str(&report.recommendation.replace('\n', "\n\n"));
    output.push_str("\n\n## Limitations\n\n");
    output.push_str(
        "- gRPC compare subscribes only to slots; it is a point-in-time diagnostic, not a benchmark or SLA.\n",
    );
    output.push_str(
        "- gRPC does not expose a genesis hash; compare endpoints on the same Solana network.\n",
    );
    output.push_str("- Scores are deterministic heuristics, not a provider guarantee.\n\n");
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
pub fn write_markdown_report(
    report: &GrpcCompareReport,
    path: &std::path::Path,
) -> Result<(), AppError> {
    fs::write(path, render_markdown(report)).map_err(|source| AppError::WriteMarkdownReport {
        path: path.display().to_string(),
        source,
    })
}

fn format_rank(index: Option<usize>) -> String {
    index.map_or_else(|| "n/a".to_string(), |index| format!("gRPC #{index}"))
}

fn format_latency(latency: Option<u128>) -> String {
    latency.map_or_else(|| "n/a".to_string(), style::millis)
}

fn format_slot(slot: Option<u64>) -> String {
    slot.map_or_else(|| "n/a".to_string(), |slot| slot.to_string())
}

fn format_slot_lag(slot_lag: Option<u64>) -> String {
    match slot_lag {
        Some(0) => "baseline".to_string(),
        Some(lag) => format!("{lag} slots behind"),
        None => "n/a".to_string(),
    }
}

fn format_slot_lag_compact(slot_lag: Option<u64>) -> String {
    match slot_lag {
        Some(0) => "baseline".to_string(),
        Some(lag) => format!("{lag} behind"),
        None => "n/a".to_string(),
    }
}

fn format_token(provided: bool) -> String {
    if provided {
        "provided".to_string()
    } else {
        "none".to_string()
    }
}

fn format_failed_methods(failed: &[String]) -> String {
    if failed.is_empty() {
        "none".to_string()
    } else {
        failed.join(", ")
    }
}
