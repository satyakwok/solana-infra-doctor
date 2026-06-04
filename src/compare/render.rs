//! Human-readable terminal, JSON, and Markdown rendering for compare reports.

use super::CompareReport;
use crate::{color::Palette, error::AppError};
use std::fs;

pub fn render_human(report: &CompareReport) -> String {
    render_human_colored(report, Palette::new(false))
}

pub fn render_human_colored(report: &CompareReport, palette: Palette) -> String {
    let mut output = String::new();
    output.push_str(&palette.title("Solana Infra Doctor — RPC Compare"));
    output.push_str("\n\n");
    output.push_str(&format!(
        "{} {}\n\n",
        palette.label("Profile:"),
        report.profile.label()
    ));

    if report.network_mismatch {
        output.push_str(&palette.label("Cannot compare endpoints from different Solana networks."));
        output.push('\n');
        output.push_str(&palette.label(
            "Endpoints returned different genesis hashes; ranking and slot lag are disabled.",
        ));
        output.push_str("\n\n");
    }

    for endpoint in &report.endpoints {
        output.push_str(&palette.heading(&format!("RPC #{}", endpoint.index)));
        output.push('\n');
        output.push_str(&format!("{} {}\n", palette.label("URL:"), endpoint.url));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Genesis:"),
            format_genesis(&endpoint.genesis_hash)
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Verdict:"),
            palette.verdict(endpoint.verdict)
        ));
        output.push_str(&format!(
            "{} {}/100\n",
            palette.label("Score:"),
            endpoint.score
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Slot:"),
            format_slot(endpoint.slot)
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Slot lag:"),
            format_slot_lag(endpoint.slot_lag)
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Average latency:"),
            format_latency(endpoint.average_latency_ms)
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Failed checks:"),
            format_failed_checks(&endpoint.failed_checks)
        ));
        output.push_str(&format!(
            "{} {}\n",
            palette.label("Blockhash valid:"),
            if endpoint.blockhash_valid {
                palette.good("yes")
            } else {
                palette.bad("no")
            }
        ));
        if !endpoint.notes.is_empty() {
            output.push_str(&palette.label("Notes:"));
            output.push('\n');
            for note in &endpoint.notes {
                output.push_str(&format!("- {note}\n"));
            }
        }
        output.push('\n');
    }

    output.push_str(&palette.heading("Recommendation:"));
    output.push('\n');
    output.push_str(&report.recommendation);
    output.push('\n');
    output
}

pub fn render_json(report: &CompareReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

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
