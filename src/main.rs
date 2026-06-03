#![forbid(unsafe_code)]

use clap::Parser;
use solana_infra_doctor::{
    checks,
    cli::{Cli, Commands},
    compare, report,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();

    let exit_code = match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            3
        }
    };

    std::process::exit(exit_code);
}

async fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check(args) => {
            let json = args.json;
            let result = checks::run_check(args).await?;
            if json {
                report::print_json(&result)?;
            } else {
                report::print_report(&result)?;
            }
            Ok(result.verdict.exit_code())
        }
        Commands::Compare(args) => {
            let json = args.json;
            let markdown_report = args.report.clone();
            let result = compare::run_compare(args).await?;
            if let Some(path) = markdown_report {
                compare::write_markdown_report(&result, &path)?;
            }
            if json {
                println!("{}", compare::render_json(&result)?);
            } else {
                print!("{}", compare::render_human(&result));
            }
            // A mixed-network comparison cannot produce a reliable ranking, so it
            // exits with the UNKNOWN code (3); same-network comparisons stay 0.
            Ok(if result.network_mismatch { 3 } else { 0 })
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_target(false)
        .init();
}
