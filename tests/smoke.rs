use std::time::Duration;

use anyhow::Ok;
use kata_lifecycle::{CommandResult, CommandRunner, CtrClient, ProcessCommandRunner, SmokeService};

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
