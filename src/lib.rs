use std::time::{Duration, Instant};

use tokio::process::Command;

use anyhow::Ok;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeReport {
    pub name: String,
    pub passed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[async_trait::async_trait]
pub trait CommandRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult>;
}

pub struct CtrClient<R> {
    runner: R,
    timeout: Duration,
}

impl<R> CtrClient<R> {
    pub fn new(runner: R, timeout: Duration) -> Self {
        Self { runner, timeout }
    }
}

impl<R: CommandRunner> CtrClient<R> {
    pub async fn version(&self) -> anyhow::Result<CommandResult> {
        self.runner.run("ctr", &["version"], self.timeout).await
    }

    pub async fn pull_image(&self, image: &str) -> anyhow::Result<CommandResult> {
        self.runner
            .run("ctr", &["images", "pull", image], self.timeout)
            .await
    }

    pub async fn run_container(
        &self,
        image: &str,
        container_id: &str,
        command: &[&str],
    ) -> anyhow::Result<CommandResult> {
        let mut args = vec![
            "run",
            "--runtime",
            "io.containerd.kata.v2",
            image,
            container_id,
        ];

        args.extend_from_slice(command);

        self.runner.run("ctr", &args, self.timeout).await
    }
}

pub struct SmokeService<R> {
    runner: R,
    timeout: Duration,
}

impl<R: CommandRunner> SmokeService<R> {
    pub fn new(runner: R, timeout: Duration) -> Self {
        Self { runner, timeout }
    }
}

impl<R: CommandRunner> SmokeService<R> {
    pub async fn run(&self) -> anyhow::Result<SmokeReport> {
        let result = self.runner.run("ctr", &["version"], self.timeout).await?;

        if result.exit_code == Some(0) {
            Ok(SmokeReport {
                name: "smoke".to_string(),
                passed: true,
                reason: None,
            })
        } else {
            Ok(SmokeReport {
                name: "smoke".to_string(),
                passed: false,
                reason: Some(format!(
                    "ctr version failed: exit_code={:?}, stderr={}",
                    result.exit_code, result.stderr
                )),
            })
        }
    }
}

pub struct ProcessCommandRunner;

#[async_trait::async_trait]
impl CommandRunner for ProcessCommandRunner {
    async fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<CommandResult> {
        let started_at = Instant::now();

        let output = tokio::time::timeout(timeout, Command::new(program).args(args).output())
            .await
            .map_err(|_| anyhow::anyhow!("command timed out: {program}"))??;

        let duration_ms = started_at.elapsed().as_millis() as u64;

        Ok(CommandResult {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_ms,
        })
    }
}
