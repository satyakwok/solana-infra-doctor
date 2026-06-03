use crate::{
    checks::{CheckReport, CheckStatus},
    error::AppError,
};

pub fn print_report(report: &CheckReport) -> Result<(), AppError> {
    println!("{}", render_human(report));
    Ok(())
}

pub fn print_json(report: &CheckReport) -> Result<(), AppError> {
    let json = render_json(report)?;
    println!("{json}");
    Ok(())
}

pub fn render_human(report: &CheckReport) -> String {
    let average = report
        .average_latency_ms
        .map_or_else(|| "n/a".to_string(), |value| format!("{value}ms"));
    let mut output = String::new();

    output.push_str("Solana Infra Doctor\n");
    output.push_str("===================\n");
    output.push_str(&format!("RPC URL: {}\n", report.rpc_url));
    output.push_str(&format!("Verdict: {}\n", report.verdict));
    output.push_str(&format!("Summary: {}\n", report.summary));
    output.push_str(&format!("Average latency: {average}\n\n"));
    output.push_str("Checks:\n");

    for check in &report.checks {
        let status = match check.status {
            CheckStatus::Success => "OK",
            CheckStatus::Failed => "FAIL",
        };
        let latency = check
            .latency_ms
            .map_or_else(|| "n/a".to_string(), |value| format!("{value}ms"));
        output.push_str(&format!(
            "- {:<14} {:<4} {:>8}  {}\n",
            check.method, status, latency, check.detail
        ));
    }

    output
}

pub fn render_json(report: &CheckReport) -> Result<String, AppError> {
    serde_json::to_string_pretty(report).map_err(AppError::SerializeReport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        checks::{CheckReport, RpcCheck},
        verdict::Verdict,
    };
    use serde_json::Value;

    fn report() -> CheckReport {
        CheckReport {
            verdict: Verdict::Good,
            rpc_url: "https://api.mainnet-beta.solana.com/".to_string(),
            summary: "all required RPC checks succeeded".to_string(),
            average_latency_ms: Some(100),
            checks: vec![RpcCheck {
                method: "getHealth",
                status: CheckStatus::Success,
                latency_ms: Some(100),
                detail: "health is ok".to_string(),
            }],
        }
    }

    #[test]
    fn renders_human_report() {
        let rendered = render_human(&report());
        assert!(rendered.contains("Solana Infra Doctor"));
        assert!(rendered.contains("Verdict: GOOD"));
        assert!(rendered.contains("getHealth"));
    }

    #[test]
    fn renders_json_report_shape() {
        let rendered = render_json(&report()).unwrap();
        let parsed: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed["verdict"], "GOOD");
        assert_eq!(parsed["average_latency_ms"], 100);
        assert_eq!(parsed["checks"][0]["method"], "getHealth");
        assert_eq!(parsed["checks"][0]["status"], "success");
    }
}
