use std::{path::Path, process, time::Duration};

use kata_lifecycle::{
    CtrClient, ProcessCollector, ProcessCommandRunner, ScenarioReport, SigkillScenario, run_smoke,
};

const USAGE: &str = "\
kata-lifecycle

USAGE:
    kata-lifecycle smoke
    kata-lifecycle smoke --format json --output result.json
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

fn parse_output_path(args: &mut impl Iterator<Item = String>) -> anyhow::Result<Option<String>> {
    let mut output_path = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--format" => {
                let format = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --format"))?;

                if format != "json" {
                    return Err(anyhow::anyhow!("unsupported format: {format}"));
                }
            }
            "--output" => {
                output_path = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --output"))?,
                );
            }
            argument => {
                return Err(anyhow::anyhow!("unexpected argument: {argument}"));
            }
        }
    }

    Ok(output_path)
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--help") | None => {
            println!("{USAGE}");
        }
        Some("smoke") => {
            let output_path = match parse_output_path(&mut args) {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("{}", render_smoke_error(&error));
                    process::exit(2);
                }
            };

            run_smoke(Duration::from_secs(3))
                .await
                .map(|report| handle_smoke_report(report, output_path.as_deref().map(Path::new)))
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

fn write_output(content: &str, output_path: Option<&Path>) -> anyhow::Result<()> {
    match output_path {
        Some(path) => std::fs::write(path, content)
            .map_err(|error| anyhow::anyhow!("failed to write output {}: {error}", path.display())),
        None => {
            println!("{content}");
            Ok(())
        }
    }
}

fn handle_smoke_report(report: kata_lifecycle::SmokeReport, output_path: Option<&Path>) {
    let result = if report.passed { "pass" } else { "fail" };

    let json = serde_json::json!({
        "scenario": report.name,
        "result": result,
        "reason": report.reason,
    });

    let content = serde_json::to_string_pretty(&json).expect("smoke report should be serializable");

    if let Err(error) = write_output(&content, output_path) {
        eprintln!("{error}");
        process::exit(1);
    }

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

    #[test]
    fn write_output_json_writes_report_to_file() {
        let directory = tempfile::tempdir().unwrap();
        let output_path = directory.path().join("result.json");

        let json = r#"{"scenario":"smoke","result":"pass"}"#;

        write_output(json, Some(&output_path)).unwrap();

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, json);
    }
}
