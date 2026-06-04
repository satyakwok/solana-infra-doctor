#![forbid(unsafe_code)]

use clap::Parser;
use solana_infra_doctor::{
    checks,
    cli::{Cli, Commands},
    color::Palette,
    compare, report, ws,
};
use std::io::IsTerminal;
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
    let verbose = cli.verbose;
    // Human output is colored only on a real terminal that has not opted out via
    // `--color`, `NO_COLOR`, or `TERM=dumb`. JSON is never colored (the `false`
    // json argument here is resolved per branch below, where JSON skips color).
    let palette = Palette::resolve(
        cli.color,
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("TERM").is_ok_and(|term| term == "dumb"),
        false,
    );

    match cli.command {
        Commands::Check(args) => {
            let json = args.json;
            let result = checks::run_check(args).await?;
            if json {
                report::print_json(&result)?;
            } else {
                print!("{}", report::render_human(&result, palette, verbose));
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
                print!("{}", compare::render_human(&result, palette, verbose));
            }
            // A mixed-network comparison cannot produce a reliable ranking, so it
            // exits with the UNKNOWN code (3); same-network comparisons stay 0.
            Ok(if result.network_mismatch { 3 } else { 0 })
        }
        Commands::Ws(args) => {
            let json = args.json;
            let result = ws::run_ws(args).await?;
            if json {
                println!("{}", ws::render_json(&result)?);
            } else {
                print!("{}", ws::render_human(&result, palette, verbose));
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
