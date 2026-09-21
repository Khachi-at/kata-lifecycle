use std::{
    path::{Path, PathBuf},
    process,
    time::Duration,
};

use kata_lifecycle::{
    CtrClient, ProcessCollector, ProcessCommandRunner, ScenarioReport, SigkillScenario, run_smoke,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Junit,
}

struct OutputOptions {
    format: OutputFormat,
    output_path: Option<PathBuf>,
}

struct RunOptions {
    output: OutputOptions,
    image: String,
    timeout: Duration,
}

const USAGE: &str = "\
kata-lifecycle

USAGE:
    kata-lifecycle smoke
    kata-lifecycle smoke --format json --output result.json
    kata-lifecycle run sigkill
    kata-lifecycle run sigkill --image IMAGE --timeout SECONDS
    kata-lifecycle run sigkill --format json --output result.json
    kata-lifecycle run sigkill --format junit --output results.xml
    kata-lifecycle --help
";

async fn run_sigkill(image: &str, timeout: Duration) -> anyhow::Result<ScenarioReport> {
    let runner = ProcessCommandRunner;

    let client = CtrClient::new(runner, timeout);

    let container_id = format!("kata-lifecycle-{}", process::id());

    let scenario = SigkillScenario::new(client, image, &container_id);

    let collector = ProcessCollector::new("/proc");

    scenario
        .run_with_process_collector(
            &collector,
            Duration::from_secs(10),
            Duration::from_millis(100),
        )
        .await
}

fn parse_output_options(args: &mut impl Iterator<Item = String>) -> anyhow::Result<OutputOptions> {
    let mut format = OutputFormat::Json;
    let mut output_path = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --format"))?;

                format = match value.as_str() {
                    "json" => OutputFormat::Json,
                    "junit" => OutputFormat::Junit,
                    _ => {
                        return Err(anyhow::anyhow!("unsupported format: {value}"));
                    }
                };
            }
            "--output" => {
                output_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("missing value for --output")
                    })?));
            }
            argument => return Err(anyhow::anyhow!("unexpected argument: {argument}")),
        }
    }

    Ok(OutputOptions {
        format,
        output_path,
    })
}

fn parse_run_options(args: &mut impl Iterator<Item = String>) -> anyhow::Result<RunOptions> {
    let mut format = OutputFormat::Json;
    let mut output_path = None;
    let mut image = "docker.io/library/busybox:latest".to_string();
    let mut timeout = Duration::from_secs(30);

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --format"))?;

                format = match value.as_str() {
                    "json" => OutputFormat::Json,
                    "junit" => OutputFormat::Junit,
                    _ => {
                        return Err(anyhow::anyhow!("unsupported format: {value}"));
                    }
                };
            }
            "--output" => {
                output_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("missing value for --output")
                    })?));
            }
            "--image" => {
                image = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --image"))?;

                if image.is_empty() {
                    return Err(anyhow::anyhow!("image cannot be empty"));
                }
            }
            "--timeout" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --timeout"))?;

                let seconds = value
                    .parse::<u64>()
                    .map_err(|_| anyhow::anyhow!("timeout must be a positive integer: {value}"))?;

                if seconds == 0 {
                    return Err(anyhow::anyhow!("timeout must be greater than zero"));
                }

                timeout = Duration::from_secs(seconds);
            }
            argument => {
                return Err(anyhow::anyhow!("unexpected argument: {argument}"));
            }
        }
    }

    Ok(RunOptions {
        output: OutputOptions {
            format,
            output_path,
        },
        image,
        timeout,
    })
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--help") | None => {
            println!("{USAGE}");
        }
        Some("smoke") => {
            let options = match parse_output_options(&mut args) {
                Ok(options) => options,
                Err(error) => {
                    eprintln!("{}", render_smoke_error(&error));
                    process::exit(2);
                }
            };

            if options.format != OutputFormat::Json {
                let error = anyhow::anyhow!("smoke currently supports only the json format");

                eprintln!("{}", render_smoke_error(&error));
                process::exit(2);
            }

            run_smoke(Duration::from_secs(3))
                .await
                .map(|report| {
                    handle_smoke_report(report, options.output_path.as_deref().map(Path::new))
                })
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

            let options = match parse_run_options(&mut args) {
                Ok(options) => options,
                Err(error) => {
                    eprintln!("{}", render_scenario_error(&error));
                    process::exit(2);
                }
            };

            run_sigkill(&options.image, options.timeout)
                .await
                .map(|report| {
                    handle_scenario_report(report, &options.output);
                })
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

fn write_scenario_report(report: &ScenarioReport, options: &OutputOptions) -> anyhow::Result<()> {
    let content = match options.format {
        OutputFormat::Json => report.to_json()?,
        OutputFormat::Junit => report.to_junit_xml()?,
    };

    write_output(&content, options.output_path.as_deref())
}

fn handle_scenario_report(report: ScenarioReport, options: &OutputOptions) {
    if let Err(error) = write_scenario_report(&report, options) {
        eprintln!("{}", render_scenario_error(&error));
        process::exit(1);
    }

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

    #[test]
    fn scenario_report_can_be_written_to_requested_output() {
        let directory = tempfile::tempdir().unwrap();
        let output_path = directory.path().join("sigkill.json");

        let report = ScenarioReport {
            name: "sigkill".to_string(),
            passed: true,
            reason: None,
            duration_ms: 100,
            leaked_processes: Vec::new(),
        };

        let options = OutputOptions {
            format: OutputFormat::Json,
            output_path: Some(output_path.clone()),
        };

        write_scenario_report(&report, &options).unwrap();

        let content = std::fs::read_to_string(output_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(value["scenario"], "sigkill");
        assert_eq!(value["result"], "pass");
        assert_eq!(value["duration_ms"], 100);
    }

    #[test]
    fn parse_output_options_accepts_junit_format() {
        let arguments = vec![
            "--format".to_string(),
            "junit".to_string(),
            "--output".to_string(),
            "results.xml".to_string(),
        ];

        let options = parse_output_options(&mut arguments.into_iter()).unwrap();

        assert_eq!(options.format, OutputFormat::Junit);
        assert_eq!(options.output_path, Some(PathBuf::from("results.xml")));
    }

    #[test]
    fn scenario_report_writes_junit_output() {
        let directory = tempfile::tempdir().unwrap();
        let output_path = directory.path().join("results.xml");

        let report = ScenarioReport {
            name: "sigkill".to_string(),
            passed: false,
            reason: Some("QEMU remained".to_string()),
            duration_ms: 1250,
            leaked_processes: Vec::new(),
        };

        let options = OutputOptions {
            format: OutputFormat::Junit,
            output_path: Some(output_path.clone()),
        };

        write_scenario_report(&report, &options).unwrap();

        let content = std::fs::read_to_string(output_path).unwrap();

        assert!(content.contains("<testsuite"));
        assert!(content.contains("failures=\"1\""));
        assert!(content.contains("name=\"sigkill\""));
        assert!(content.contains("QEMU remained"));
    }

    #[test]
    fn parse_run_options_accepts_image_and_timeout() {
        let arguments = vec![
            "--image".to_string(),
            "example.com/test/busybox:1.0".to_string(),
            "--timeout".to_string(),
            "45".to_string(),
            "--format".to_string(),
            "junit".to_string(),
            "--output".to_string(),
            "results.xml".to_string(),
        ];

        let options = parse_run_options(&mut arguments.into_iter()).unwrap();

        assert_eq!(options.image, "example.com/test/busybox:1.0");
        assert_eq!(options.timeout, Duration::from_secs(45));
        assert_eq!(options.output.format, OutputFormat::Junit);
        assert_eq!(
            options.output.output_path,
            Some(PathBuf::from("results.xml"))
        );
    }

    #[test]
    fn parse_run_options_uses_defaults() {
        let options = parse_run_options(&mut Vec::<String>::new().into_iter()).unwrap();

        assert_eq!(options.image, "docker.io/library/busybox:latest");
        assert_eq!(options.timeout, Duration::from_secs(30));
        assert_eq!(options.output.format, OutputFormat::Json);
        assert_eq!(options.output.output_path, None);
    }
}
