use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Ok;
use kata_lifecycle::{
    CommandResult, CommandRunner, CtrClient, ProcessCommandRunner, SigkillScenario, SmokeService,
};

struct FakeRunner;

#[async_trait::async_trait]
impl CommandRunner for FakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["version"]);
        assert_eq!(timeout, Duration::from_secs(3));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: "ctr version ok".to_string(),
            stderr: String::new(),
            duration_ms: 42,
        })
    }
}

#[tokio::test]
async fn smoke_passes_when_ctr_version_succeeds() {
    let service = SmokeService::new(FakeRunner, Duration::from_secs(3));

    let report = service.run().await.unwrap();

    assert_eq!(report.name, "smoke");
    assert!(report.passed);
    assert_eq!(report.reason, None);
}

struct FailingRunner;

#[async_trait::async_trait]
impl CommandRunner for FailingRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["version"]);
        assert_eq!(timeout, Duration::from_secs(3));

        Ok(CommandResult {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "ctr failed".to_string(),
            duration_ms: 12,
        })
    }
}

#[tokio::test]
async fn smoke_fails_when_ctr_version_returns_non_zero() {
    let service = SmokeService::new(FailingRunner, Duration::from_secs(3));

    let report = service.run().await.unwrap();

    assert_eq!(report.name, "smoke");
    assert!(!report.passed);
    assert!(report.reason.as_deref().unwrap().contains("ctr failed"));
}

struct ErrorRunner;

#[async_trait::async_trait]
impl CommandRunner for ErrorRunner {
    async fn run(
        &self,
        _program: &str,
        _args: &[&str],
        _timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        Err(anyhow::anyhow!("runner unavailable"))
    }
}

#[tokio::test]
async fn smoke_returns_error_when_runner_fails() {
    let service = SmokeService::new(ErrorRunner, Duration::from_secs(3));

    let result = service.run().await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("runner unavailable")
    );
}

#[tokio::test]
async fn process_runner_captures_command_output() {
    let runner = ProcessCommandRunner;

    let result = runner
        .run("printf", &["hello"], Duration::from_secs(3))
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout, "hello");
    assert_eq!(result.stderr, "");
    assert!(result.duration_ms < 3_000);
}

struct CtrFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for CtrFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["version"]);
        assert_eq!(timeout, Duration::from_secs(5));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: "Client: \n  Version: 1.0.0".to_string(),
            stderr: String::new(),
            duration_ms: 20,
        })
    }
}

#[tokio::test]
async fn ctr_client_returns_version_command_result() {
    let client = CtrClient::new(CtrFakeRunner, Duration::from_secs(5));

    let result = client.version().await.unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("Version: 1.0.0"));
    assert_eq!(result.stderr, "");
}

struct PullImageFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for PullImageFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(
            args,
            &["images", "pull", "docker.io/library/busybox:latest"]
        );
        assert_eq!(timeout, Duration::from_secs(10));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: "unpacking docker.io/library/busybox:latest".to_string(),
            stderr: String::new(),
            duration_ms: 120,
        })
    }
}

#[tokio::test]
async fn ctr_client_pulls_image() {
    let client = CtrClient::new(PullImageFakeRunner, Duration::from_secs(10));

    let result = client
        .pull_image("docker.io/library/busybox:latest")
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("unpacking"));
    assert_eq!(result.stderr, "");
}

struct RunContainerFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for RunContainerFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(
            args,
            &[
                "run",
                "--runtime",
                "io.containerd.kata.v2",
                "--detach",
                "docker.io/library/busybox:latest",
                "kata-lifecycle-test",
                "sleep",
                "300",
            ]
        );
        assert_eq!(timeout, Duration::from_secs(30));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 500,
        })
    }
}

#[tokio::test]
async fn ctr_client_runs_container_with_kata_runtime() {
    let client = CtrClient::new(RunContainerFakeRunner, Duration::from_secs(30));

    let result = client
        .run_container(
            "docker.io/library/busybox:latest",
            "kata-lifecycle-test",
            &["sleep", "300"],
        )
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
}

struct ListTasksFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for ListTasksFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["tasks", "list"]);
        assert_eq!(timeout, Duration::from_secs(5));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: concat!(
                "TASK                 PID     STATUS\n",
                "kata-lifecycle-test  1234    RUNNING\n",
            )
            .to_string(),
            stderr: String::new(),
            duration_ms: 15,
        })
    }
}

#[tokio::test]
async fn ctr_client_lists_tasks() {
    let client = CtrClient::new(ListTasksFakeRunner, Duration::from_secs(5));

    let result = client.list_tasks().await.unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("kata-lifecycle-test"));
    assert!(result.stdout.contains("RUNNING"));
    assert_eq!(result.stderr, "");
}

struct KillTaskFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for KillTaskFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(
            args,
            &[
                "tasks",
                "kill",
                "--signal",
                "SIGKILL",
                "kata-lifecycle-test",
            ]
        );
        assert_eq!(timeout, Duration::from_secs(5));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 18,
        })
    }
}

#[tokio::test]
async fn ctr_client_sends_signal_to_task() {
    let client = CtrClient::new(KillTaskFakeRunner, Duration::from_secs(5));

    let result = client
        .kill_task("kata-lifecycle-test", "SIGKILL")
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stderr, "");
}

struct RemoveTaskFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for RemoveTaskFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["tasks", "rm", "kata-lifecycle-test"]);
        assert_eq!(timeout, Duration::from_secs(5));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 25,
        })
    }
}

#[tokio::test]
async fn ctr_client_removes_stopped_task() {
    let client = CtrClient::new(RemoveTaskFakeRunner, Duration::from_secs(5));

    let result = client.remove_task("kata-lifecycle-test").await.unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stderr, "");
}

struct RemoveContainerFakeRunner;

#[async_trait::async_trait]
impl CommandRunner for RemoveContainerFakeRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(args, &["containers", "rm", "kata-lifecycle-test"]);
        assert_eq!(timeout, Duration::from_secs(5));

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 16,
        })
    }
}

#[tokio::test]
async fn ctr_client_removes_container() {
    let client = CtrClient::new(RemoveContainerFakeRunner, Duration::from_secs(5));

    let result = client
        .remove_container("kata-lifecycle-test")
        .await
        .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stderr, "");
}

struct RecordingRunner {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait::async_trait]
impl CommandRunner for RecordingRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(timeout, Duration::from_secs(5));

        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| arg.to_string()).collect());

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 10,
        })
    }
}

#[tokio::test]
async fn sigkill_scenario_runs_and_cleans_up_container() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let runner = RecordingRunner {
        calls: Arc::clone(&calls),
    };

    let client = CtrClient::new(runner, Duration::from_secs(5));

    let scenario = SigkillScenario::new(
        client,
        "docker.io/library/busybox:latest",
        "kata-lifecycle-test",
    );

    let report = scenario.run().await.unwrap();

    assert_eq!(report.name, "sigkill");
    assert!(report.passed);
    assert_eq!(report.reason, None);

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            vec![
                "run",
                "--runtime",
                "io.containerd.kata.v2",
                "--detach",
                "docker.io/library/busybox:latest",
                "kata-lifecycle-test",
                "sleep",
                "300",
            ],
            vec![
                "tasks",
                "kill",
                "--signal",
                "SIGKILL",
                "kata-lifecycle-test",
            ],
            vec!["tasks", "rm", "kata-lifecycle-test"],
            vec!["containers", "rm", "kata-lifecycle-test"]
        ]
    );
}

struct StartFailureRunner {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait::async_trait]
impl CommandRunner for StartFailureRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(timeout, Duration::from_secs(5));

        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| arg.to_string()).collect());

        if args.first() == Some(&"run") {
            return Ok(CommandResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "failed to create Kata sandbox".to_string(),
                duration_ms: 40,
            });
        }

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 10,
        })
    }
}

#[tokio::test]
async fn sigkill_scenario_reports_start_failure_and_attempts_cleanup() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let runner = StartFailureRunner {
        calls: Arc::clone(&calls),
    };

    let client = CtrClient::new(runner, Duration::from_secs(5));

    let scenario = SigkillScenario::new(
        client,
        "docker.io/library/busybox:latest",
        "kata-lifecycle-test",
    );

    let report = scenario.run().await.unwrap();

    assert_eq!(report.name, "sigkill");
    assert!(!report.passed);
    assert!(
        report
            .reason
            .as_deref()
            .unwrap()
            .contains("failed to create Kata sandbox")
    );

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            vec![
                "run",
                "--runtime",
                "io.containerd.kata.v2",
                "--detach",
                "docker.io/library/busybox:latest",
                "kata-lifecycle-test",
                "sleep",
                "300",
            ],
            vec!["tasks", "rm", "kata-lifecycle-test"],
            vec!["containers", "rm", "kata-lifecycle-test"],
        ]
    );
}

struct KillFailureRunner {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait::async_trait]
impl CommandRunner for KillFailureRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(timeout, Duration::from_secs(5));

        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| arg.to_string()).collect());

        if args.starts_with(&["tasks", "kill"]) {
            return Ok(CommandResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "failed to signal task".to_string(),
                duration_ms: 12,
            });
        }

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 10,
        })
    }
}

#[tokio::test]
async fn sigkill_scenario_reports_kill_failure_and_attempts_cleanup() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let runner = KillFailureRunner {
        calls: Arc::clone(&calls),
    };

    let client = CtrClient::new(runner, Duration::from_secs(5));

    let scenario = SigkillScenario::new(
        client,
        "docker.io/library/busybox:latest",
        "kata-lifecycle-test",
    );

    let report = scenario.run().await.unwrap();

    assert_eq!(report.name, "sigkill");
    assert!(!report.passed);
    assert!(
        report
            .reason
            .as_deref()
            .unwrap()
            .contains("failed to signal task")
    );

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            vec![
                "run",
                "--runtime",
                "io.containerd.kata.v2",
                "--detach",
                "docker.io/library/busybox:latest",
                "kata-lifecycle-test",
                "sleep",
                "300",
            ],
            vec![
                "tasks",
                "kill",
                "--signal",
                "SIGKILL",
                "kata-lifecycle-test",
            ],
            vec!["tasks", "rm", "kata-lifecycle-test"],
            vec!["containers", "rm", "kata-lifecycle-test"],
        ]
    );
}

struct RemoveTaskFailureRunner {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait::async_trait]
impl CommandRunner for RemoveTaskFailureRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert_eq!(program, "ctr");
        assert_eq!(timeout, Duration::from_secs(5));

        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|arg| arg.to_string()).collect());

        if args.starts_with(&["tasks", "rm"]) {
            return Ok(CommandResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "failed to remove task".to_string(),
                duration_ms: 14,
            });
        }

        Ok(CommandResult {
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 10,
        })
    }
}

#[tokio::test]
async fn sigkill_scenario_reports_task_removal_failure_and_removes_container() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let runner = RemoveTaskFailureRunner {
        calls: Arc::clone(&calls),
    };

    let client = CtrClient::new(runner, Duration::from_secs(5));

    let scenario = SigkillScenario::new(
        client,
        "docker.io/library/busybox:latest",
        "kata-lifecycle-test",
    );

    let report = scenario.run().await.unwrap();

    assert_eq!(report.name, "sigkill");
    assert!(!report.passed);
    assert!(
        report
            .reason
            .as_deref()
            .unwrap()
            .contains("failed to remove task")
    );

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            vec![
                "run",
                "--runtime",
                "io.containerd.kata.v2",
                "--detach",
                "docker.io/library/busybox:latest",
                "kata-lifecycle-test",
                "sleep",
                "300",
            ],
            vec![
                "tasks",
                "kill",
                "--signal",
                "SIGKILL",
                "kata-lifecycle-test",
            ],
            vec!["tasks", "rm", "kata-lifecycle-test"],
            vec!["containers", "rm", "kata-lifecycle-test"],
        ]
    );
}
