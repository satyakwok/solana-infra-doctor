//! Human-readable terminal, JSON, and Markdown rendering for compare reports.

use super::CompareReport;
use crate::{
    checks::ProgramAccountsReadiness,
    color::Palette,
    error::AppError,
    output::{
        style::{self, endpoint_label},
        table::{self, Cell},
    },
};
use std::fs;

/// Render the human-readable `compare` report.
///
/// The default view is a one-row-per-endpoint summary table (verdict, score,
/// latency, slot lag) plus the recommendation. `verbose` expands each endpoint
/// into a full detail block (full redacted URL, genesis hash, failed checks,
/// notes). `verbose` affects human output only.
pub fn render_human(report: &CompareReport, palette: Palette, verbose: bool) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor · RPC Comparison"));
    output.push_str("\n\n");
    output.push_str(&format!(
        "{} {}\n\n",
        palette.label("Profile:"),
        report.profile.label()
    ));

    if report.network_mismatch {
        output.push_str(&palette.warn("Endpoints are on different Solana networks."));
        output.push('\n');
        output.push_str(
            &palette.dim(
                "Genesis hashes differ; ranking and slot lag are disabled for this comparison.",
            ),
        );
        output.push_str("\n\n");
    }

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

fn render_endpoints_summary(report: &CompareReport, palette: Palette, output: &mut String) {
    let mut rows = vec![vec![
        Cell::styled("RPC", palette.label("RPC")),
        Cell::styled("Endpoint", palette.label("Endpoint")),
        Cell::styled("Verdict", palette.label("Verdict")),
        Cell::styled("Score", palette.label("Score")),
        Cell::styled("Latency", palette.label("Latency")),
        Cell::styled("Slot lag", palette.label("Slot lag")),
    ]];
    for endpoint in &report.endpoints {
        rows.push(vec![
            Cell::plain(format!("#{}", endpoint.index)),
            Cell::plain(endpoint_label(&endpoint.url)),
            Cell::styled(
                endpoint.verdict.to_string(),
                palette.verdict(endpoint.verdict),
            ),
            Cell::plain(format!("{}/100", endpoint.score)),
            Cell::plain(format_latency_spaced(endpoint.average_latency_ms)),
            Cell::plain(format_slot_lag_compact(endpoint.slot_lag)),
        ]);
    }
    output.push_str(&table::render(&rows, 3));
    output.push('\n');
}

fn render_endpoints_verbose(report: &CompareReport, palette: Palette, output: &mut String) {
    for endpoint in &report.endpoints {
        output.push_str(&palette.heading(&format!("RPC #{}", endpoint.index)));
        output.push('\n');
        let blockhash = if endpoint.blockhash_valid {
            palette.good("yes")
        } else {
            palette.bad("no")
        };
        let mut rows = vec![
            detail_row(palette, "URL", endpoint.url.clone()),
            detail_row(palette, "Genesis", format_genesis(&endpoint.genesis_hash)),
            vec![
                Cell::styled("Verdict", palette.label("Verdict")),
                Cell::styled(
                    endpoint.verdict.to_string(),
                    palette.verdict(endpoint.verdict),
                ),
            ],
            detail_row(palette, "Score", format!("{}/100", endpoint.score)),
            detail_row(palette, "Slot", format_slot(endpoint.slot)),
            detail_row(palette, "Slot lag", format_slot_lag(endpoint.slot_lag)),
            detail_row(
                palette,
                "Average latency",
                format_latency_spaced(endpoint.average_latency_ms),
            ),
            detail_row(
                palette,
                "Block time lag",
                format_block_time_lag(endpoint.block_time_lag_secs),
            ),
            detail_row(
                palette,
                "Median priority fee",
                format_priority_fee(endpoint.prioritization_fee_median),
            ),
            detail_row(
                palette,
                "Token Program",
                format_readiness(endpoint.token_program_ready),
            ),
            detail_row(
                palette,
                "Token-2022",
                format_readiness(endpoint.token_2022_ready),
            ),
            detail_row(
                palette,
                "Failed checks",
                format_failed_checks(&endpoint.failed_checks),
            ),
            vec![
                Cell::styled("Blockhash valid", palette.label("Blockhash valid")),
                Cell::styled(
                    if endpoint.blockhash_valid {
                        "yes"
                    } else {
                        "no"
                    },
                    blockhash,
                ),
            ],
        ];
        if let Some(readiness) = endpoint.program_accounts {
            rows.push(detail_row(
                palette,
                "getProgramAccounts",
                format_gpa(readiness),
            ));
            rows.push(detail_row(
                palette,
                "Archival",
                format_archival(endpoint.oldest_available_slot),
            ));
        }
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

/// Build the recommendation block. When a best endpoint exists, lead with a
/// compact `Best RPC: #N · host` line and then the narrative, dropping the raw
/// `Best RPC:` / `Worst RPC:` lines the scorer emits; on a network mismatch
/// (no ranking) the recommendation text is shown verbatim.
fn recommendation_block(report: &CompareReport, palette: &Palette) -> String {
    let Some(best) = report.best_endpoint_index else {
        return format!("{}\n", report.recommendation);
    };
    let host = report
        .endpoints
        .iter()
        .find(|endpoint| endpoint.index == best)
        .map_or_else(
            || best.to_string(),
            |endpoint| endpoint_label(&endpoint.url),
        );
    let mut output = format!("{} #{best} · {host}\n", palette.label("Best RPC:"));
    for line in report.recommendation.lines() {
        if line.starts_with("Best RPC:") || line.starts_with("Worst RPC:") {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn format_slot_lag_compact(slot_lag: Option<u64>) -> String {
    match slot_lag {
        Some(0) => "baseline".to_string(),
        Some(lag) => format!("{lag} behind"),
        None => "n/a".to_string(),
    }
}

/// Latency for human output, with a space between value and unit (`13 ms`).
/// The Markdown report keeps its own `format_latency` (`13ms`) for stability.
fn format_latency_spaced(latency: Option<u128>) -> String {
    latency.map_or_else(|| "n/a".to_string(), style::millis)
}

fn format_block_time_lag(lag: Option<i64>) -> String {
    lag.map_or_else(|| "n/a".to_string(), |lag| format!("{lag}s behind"))
}

fn format_priority_fee(fee: Option<u64>) -> String {
    fee.map_or_else(
        || "n/a".to_string(),
        |fee| format!("{fee} micro-lamports/CU"),
    )
}

fn format_readiness(ready: bool) -> String {
    if ready {
        "ready".to_string()
    } else {
        "not ready".to_string()
    }
}

fn format_gpa(readiness: ProgramAccountsReadiness) -> String {
    match readiness {
        ProgramAccountsReadiness::Ready => "ready",
        ProgramAccountsReadiness::Gated => "gated",
        ProgramAccountsReadiness::Degraded => "degraded",
    }
    .to_string()
}

fn format_archival(oldest: Option<u64>) -> String {
    match oldest {
        Some(0) => "full (from genesis)".to_string(),
        Some(oldest) => format!("from slot {oldest}"),
        None => "n/a".to_string(),
    }
}

/// Serialize a compare report to pretty-printed JSON.
pub fn render_json(report: &CompareReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

/// Render a compare report as a shareable Markdown document (no ANSI, redacted).
pub fn render_markdown(report: &CompareReport) -> String {
    let mut output = String::new();
    output.push_str("# Solana Infra Doctor RPC Compare Report\n\n");
    output.push_str(&format!("Profile: `{}`\n\n", report.profile.label()));

    if report.network_mismatch {
        output.push_str("## Network Mismatch\n\n");
        output.push_str(
            "Cannot compare endpoints from different Solana networks. Endpoints returned different genesis hashes, so ranking and slot lag are disabled.\n\n",
        );
    }

    output.push_str("## Summary\n\n");
    output.push_str(&format!(
        "- Best RPC: {}\n- Worst RPC: {}\n\n",
        format_rank(report.best_endpoint_index),
        format_rank(report.worst_endpoint_index)
    ));

    output.push_str("## Comparison\n\n");
    output.push_str("| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |\n");
    output.push_str("| --- | --- | --- | ---: | --- | --- | --- | --- | --- |\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!(
            "| RPC #{} | `{}` | `{}` | {}/100 | {} | {} | {} | {} | {} |\n",
            endpoint.index,
            endpoint.url,
            endpoint.verdict,
            endpoint.score,
            format_slot(endpoint.slot),
            format_slot_lag(endpoint.slot_lag),
            format_latency(endpoint.average_latency_ms),
            format_failed_checks(&endpoint.failed_checks),
            if endpoint.blockhash_valid {
                "yes"
            } else {
                "no"
            }
        ));
    }

    output.push_str("\n## Per-Endpoint Details\n\n");
    for endpoint in &report.endpoints {
        output.push_str(&format!("### RPC #{}\n\n", endpoint.index));
        output.push_str(&format!("- URL: `{}`\n", endpoint.url));
        output.push_str(&format!(
            "- Genesis: `{}`\n",
            format_genesis(&endpoint.genesis_hash)
        ));
        output.push_str(&format!("- Verdict: `{}`\n", endpoint.verdict));
        output.push_str(&format!("- Score: {}/100\n", endpoint.score));
        output.push_str(&format!("- Slot: {}\n", format_slot(endpoint.slot)));
        output.push_str(&format!(
            "- Slot lag: {}\n",
            format_slot_lag(endpoint.slot_lag)
        ));
        output.push_str(&format!(
            "- Average latency: {}\n",
            format_latency(endpoint.average_latency_ms)
        ));
        output.push_str(&format!(
            "- Block time lag: {}\n",
            format_block_time_lag(endpoint.block_time_lag_secs)
        ));
        output.push_str(&format!(
            "- Median priority fee: {}\n",
            format_priority_fee(endpoint.prioritization_fee_median)
        ));
        output.push_str(&format!(
            "- Token Program: {}\n",
            format_readiness(endpoint.token_program_ready)
        ));
        output.push_str(&format!(
            "- Token-2022: {}\n",
            format_readiness(endpoint.token_2022_ready)
        ));
        if let Some(readiness) = endpoint.program_accounts {
            output.push_str(&format!(
                "- getProgramAccounts: {}\n",
                format_gpa(readiness)
            ));
            output.push_str(&format!(
                "- Archival: {}\n",
                format_archival(endpoint.oldest_available_slot)
            ));
        }
        output.push_str(&format!(
            "- Failed checks: {}\n",
            format_failed_checks(&endpoint.failed_checks)
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
        "- Compare uses HTTP JSON-RPC diagnostics; run `sol-doctor ws` for WebSocket readiness.\n",
    );
    output.push_str("- Checks run sequentially for deterministic v0.1 behavior.\n");
    output.push_str("- Scores are deterministic heuristics, not a provider guarantee.\n\n");
    output.push_str("## Disclaimer\n\n");
    output.push_str(
        "Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.\n",
    );
    output
}

/// Render the Markdown report and write it to `path`.
pub fn write_markdown_report(
    report: &CompareReport,
    path: &std::path::Path,
) -> Result<(), AppError> {
    fs::write(path, render_markdown(report)).map_err(|source| AppError::WriteMarkdownReport {
        path: path.display().to_string(),
        source,
    })
}

fn format_genesis(genesis_hash: &Option<String>) -> String {
    genesis_hash
        .as_deref()
        .map_or_else(|| "n/a".to_string(), str::to_string)
}

fn format_rank(index: Option<usize>) -> String {
    index.map_or_else(
        || "n/a (different networks)".to_string(),
        |index| format!("RPC #{index}"),
    )
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

fn format_latency(latency: Option<u128>) -> String {
    latency.map_or_else(|| "n/a".to_string(), |latency| format!("{latency}ms"))
}

fn format_failed_checks(failed_checks: &[String]) -> String {
    if failed_checks.is_empty() {
        "none".to_string()
    } else {
        failed_checks.join(", ")
    }
}
