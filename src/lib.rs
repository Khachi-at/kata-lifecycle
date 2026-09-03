use std::time::Duration;

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
