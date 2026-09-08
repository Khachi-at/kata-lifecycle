use std::{process, time::Duration};

use kata_lifecycle::{
    CtrClient, ProcessCollector, ProcessCommandRunner, ScenarioReport, SigkillScenario, run_smoke,
};

const USAGE: &str = "\
kata-lifecycle

USAGE:
    kata-lifecycle smoke
    kata-lifecycle run sigkill
    kata-lifecycle --help
";

async fn run_sigkill() -> anyhow::Result<ScenarioReport> {
    let runner = ProcessCommandRunner;

    let client = CtrClient::new(runner, Duration::from_secs(30));

    let container_id = format!("kata-lifecycle-{}", process::id());

    let scenario = SigkillScenario::new(client, "docker.io/library/busybox:latest", &container_id);

    let collector = ProcessCollector::new("/proc");

    scenario
        .run_with_process_collector(
            &collector,
            Duration::from_secs(10),
            Duration::from_millis(100),
        )
        .await
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--help") | None => {
            println!("{USAGE}");
        }
        Some("smoke") => {
            run_smoke(Duration::from_secs(3))
                .await
                .map(handle_smoke_report)
                .unwrap_or_else(|error| {
                    eprintln!("{}", render_smoke_error(&error));
                    process::exit(1);
                });
        }
        Some("run") => {
            if args.next().as_deref() != Some("sigkill") {
                eprintln!("expected: kata-lifecycle run sigkill");
                process::exit(2);
            }

            run_sigkill()
                .await
                .map(handle_scenario_report)
                .unwrap_or_else(|error| {
                    eprintln!("{}", render_scenario_error(&error));
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

fn render_smoke_error(error: &anyhow::Error) -> String {
    let json = serde_json::json!({
        "scenario": "smoke",
        "result": "error",
        "reason": error.to_string(),
    });

    serde_json::to_string_pretty(&json).expect("smoke error should be serializable")
}

fn handle_scenario_report(report: ScenarioReport) {
    let json = report
        .to_json()
        .expect("scenario report should be serializable");

    println!("{json}");

    if !report.passed {
        process::exit(1);
    }
}

fn render_scenario_error(error: &anyhow::Error) -> String {
    let json = serde_json::json!({
        "scenario": "sigkill",
        "result":"error",
        "reason": error.to_string(),
    });

    serde_json::to_string_pretty(&json).expect("scenario error should be serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_execution_error_is_rendered_as_json() {
        let error = anyhow::anyhow!("ctr command not found");

        let output = render_smoke_error(&error);
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["scenario"], "smoke");
        assert_eq!(value["result"], "error");
        assert_eq!(value["reason"], "ctr command not found");
    }
}
