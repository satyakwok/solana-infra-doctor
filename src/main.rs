#![forbid(unsafe_code)]

mod checks;
mod cli;
mod error;
mod latency;
mod report;
mod rpc;
mod verdict;

use clap::Parser;
use cli::{Cli, Commands};
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
