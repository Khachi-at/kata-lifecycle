use std::{process, time::Duration};

use kata_lifecycle::run_smoke;

const USAGE: &str = "\
kata-lifecycle

USAGE:
    kata-lifecycle smoke
    kata-lifecycle --help
";

#[tokio::main]
async fn main() {
    let command = std::env::args().nth(1);

    match command.as_deref() {
        Some("--help") | None => {
            println!("{USAGE}");
        }
        Some("smoke") => {
            run_smoke(Duration::from_secs(3))
                .await
                .map(handle_smoke_report)
                .unwrap_or_else(|error| {
                    eprintln!("smoke failed to execute: {error}");
                    process::exit(1);
                });
        }
        Some(command) => {
            eprintln!("unknown command: {command}\n\n{USAGE}");
            process::exit(2);
        }
    }
}

fn handle_smoke_report(report: kata_lifecycle::SmokeReport) {
    let result = if report.passed { "pass" } else { "fail" };

    let json = serde_json::json!({
        "scenario": report.name,
        "result": result,
        "reason": report.reason,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&json).expect("smoke report should be serializable")
    );

    if !report.passed {
        process::exit(1);
    }
}
