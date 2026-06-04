use crate::{
    checks::{CheckCategory, CheckReport, CheckStatus},
    color::Palette,
    error::AppError,
};

const CATEGORY_ORDER: [CheckCategory; 3] = [
    CheckCategory::Core,
    CheckCategory::Blockhash,
    CheckCategory::Performance,
];

pub fn print_report(report: &CheckReport) -> Result<(), AppError> {
    println!("{}", render_human(report));
    Ok(())
}

pub fn print_report_colored(report: &CheckReport, palette: Palette) -> Result<(), AppError> {
    println!("{}", render_human_colored(report, palette));
    Ok(())
}

pub fn print_json(report: &CheckReport) -> Result<(), AppError> {
    let json = render_json(report)?;
    println!("{json}");
    Ok(())
}

pub fn render_human(report: &CheckReport) -> String {
    render_human_colored(report, Palette::new(false))
}

pub fn render_human_colored(report: &CheckReport, palette: Palette) -> String {
    let average = report
        .average_latency_ms
        .map_or_else(|| "n/a".to_string(), |value| format!("{value}ms"));
    let mut output = String::new();

    output.push_str(&palette.title("Solana Infra Doctor"));
    output.push('\n');
    output.push_str(&palette.label("==================="));
    output.push('\n');
    output.push_str(&format!(
        "{} {}\n",
        palette.label("RPC URL:"),
        report.rpc_url
    ));
    output.push_str(&format!(
        "{} {}\n",
        palette.label("Verdict:"),
        palette.verdict(report.verdict)
    ));
    output.push_str(&format!(
        "{} {}\n",
        palette.label("Summary:"),
        report.summary
    ));
    output.push_str(&format!(
        "{} {average}\n",
        palette.label("Average latency:")
    ));
    if report.fail_on_warning {
        output.push_str(
            &palette.label(
                "Warning policy: --fail-on-warning enabled; WARNING exits with code 1 for CI.",
            ),
        );
        output.push('\n');
    }
    output.push('\n');
    output.push_str(&palette.heading("Checks:"));
    output.push('\n');

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
        output.push_str(&palette.heading(&format!("{}:", category.label())));
        output.push('\n');
        for check in checks {
            // Pad each cell to a fixed visible width first, then colorize, so
            // ANSI codes never disturb column alignment.
            let status_cell = match check.status {
                CheckStatus::Success => palette.ok(&format!("{:<4}", "OK")),
                CheckStatus::Failed => palette.fail(&format!("{:<4}", "FAIL")),
            };
            let latency = check
                .latency_ms
                .map_or_else(|| "n/a".to_string(), |value| format!("{value}ms"));
            let latency_cell = palette.label(&format!("{latency:>8}"));
            let error_kind = check.error_kind.map_or_else(String::new, |kind| {
                palette.label(&format!(" [{}]", kind.label()))
            });
            output.push_str(&format!(
                "- {:<28} {} {}  {}{}\n",
                check.method, status_cell, latency_cell, check.detail, error_kind
            ));
        }
    }

    output
}

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
            verdict: Verdict::Good,
            rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
            summary: "all RPC readiness checks succeeded".to_string(),
            average_latency_ms: Some(100),
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
    fn renders_human_report_grouped_by_category() {
        let rendered = render_human(&report());
        assert!(rendered.contains("Solana Infra Doctor"));
        assert!(rendered.contains("Verdict: GOOD"));
        assert!(rendered.contains("Core:"));
        assert!(rendered.contains("Blockhash:"));
        assert!(rendered.contains("Performance:"));
        assert!(rendered.contains("--fail-on-warning enabled"));
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
