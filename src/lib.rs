mod process;

pub use process::{ProcessCollector, ProcessInfo, ProcessKind, new_processes};

use std::time::{Duration, Instant};

use tokio::process::Command;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioReport {
    pub name: String,
    pub passed: bool,
    pub reason: Option<String>,
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
            "--detach",
            image,
            container_id,
        ];

        args.extend_from_slice(command);

        self.runner.run("ctr", &args, self.timeout).await
    }

    pub async fn list_tasks(&self) -> anyhow::Result<CommandResult> {
        self.runner
            .run("ctr", &["tasks", "list"], self.timeout)
            .await
    }

    pub async fn task_exists(&self, container_id: &str) -> anyhow::Result<bool> {
        let result = self.list_tasks().await?;

        if result.exit_code != Some(0) {
            return Err(anyhow::anyhow!(
                "failed to list tasks: exit_code={:?}, stderr={}",
                result.exit_code,
                result.stderr,
            ));
        }

        Ok(result
            .stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .any(|task_id| task_id == container_id))
    }

    pub async fn wait_until_task_absent(
        &self,
        container_id: &str,
        wait_timeout: Duration,
        poll_interval: Duration,
    ) -> anyhow::Result<()> {
        tokio::time::timeout(wait_timeout, async {
            loop {
                if !self.task_exists(container_id).await? {
                    return Ok(());
                }

                tokio::time::sleep(poll_interval).await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "timed out after {wait_timeout:?} waiting for task \
            {container_id} to disappear"
            )
        })?
    }

    pub async fn kill_task(
        &self,
        container_id: &str,
        signal: &str,
    ) -> anyhow::Result<CommandResult> {
        self.runner
            .run(
                "ctr",
                &["tasks", "kill", "--signal", signal, container_id],
                self.timeout,
            )
            .await
    }

    pub async fn remove_task(&self, container_id: &str) -> anyhow::Result<CommandResult> {
        self.runner
            .run("ctr", &["tasks", "rm", container_id], self.timeout)
            .await
    }

    pub async fn remove_container(&self, container_id: &str) -> anyhow::Result<CommandResult> {
        self.runner
            .run("ctr", &["containers", "rm", container_id], self.timeout)
            .await
    }
}

struct CleanupResults {
    remove_task: anyhow::Result<CommandResult>,
    remove_container: anyhow::Result<CommandResult>,
}

pub struct SigkillScenario<R> {
    client: CtrClient<R>,
    image: String,
    container_id: String,
}

impl<R> SigkillScenario<R> {
    pub fn new(client: CtrClient<R>, image: &str, container_id: &str) -> Self {
        Self {
            client,
            image: image.to_string(),
            container_id: container_id.to_string(),
        }
    }
}

impl<R: CommandRunner> SigkillScenario<R> {
    pub async fn run(&self) -> anyhow::Result<ScenarioReport> {
        let start_result = match self
            .client
            .run_container(&self.image, &self.container_id, &["sleep", "300"])
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let _cleanup = self.cleanup().await;
                return Err(error);
            }
        };

        if start_result.exit_code != Some(0) {
            let _cleanup = self.cleanup().await;

            return Ok(ScenarioReport {
                name: "sigkill".to_string(),
                passed: false,
                reason: Some(format!(
                    "failed to start container: exit_code={:?}, stderr={}",
                    start_result.exit_code, start_result.stderr,
                )),
            });
        }

        let kill_result = match self.client.kill_task(&self.container_id, "SIGKILL").await {
            Ok(result) => result,
            Err(error) => {
                let _cleanup = self.cleanup().await;

                return Err(error);
            }
        };

        if kill_result.exit_code != Some(0) {
            let _cleanup = self.cleanup().await;

            return Ok(ScenarioReport {
                name: "sigkill".to_string(),
                passed: false,
                reason: Some(format!(
                    "failed to kill task: exit_code={:?}, stderr={}",
                    kill_result.exit_code, kill_result.stderr,
                )),
            });
        }

        let CleanupResults {
            remove_task,
            remove_container,
        } = self.cleanup().await;

        let remove_task_result = remove_task?;
        let remove_container_result = remove_container?;

        let mut cleanup_failures = Vec::new();

        if remove_task_result.exit_code != Some(0) {
            cleanup_failures.push(format!(
                "failed to remove task: exit_code={:?}, stderr={}",
                remove_task_result.exit_code, remove_task_result.stderr
            ));
        }

        if remove_container_result.exit_code != Some(0) {
            cleanup_failures.push(format!(
                "failed to remove container: exit_code={:?}, stderr={}",
                remove_container_result.exit_code, remove_container_result.stderr
            ));
        }

        if !cleanup_failures.is_empty() {
            return Ok(ScenarioReport {
                name: "sigkill".to_string(),
                passed: false,
                reason: Some(cleanup_failures.join("; ")),
            });
        }

        Ok(ScenarioReport {
            name: "sigkill".to_string(),
            passed: true,
            reason: None,
        })
    }

    async fn cleanup(&self) -> CleanupResults {
        let remove_task = self.client.remove_task(&self.container_id).await;

        let remove_container = self.client.remove_container(&self.container_id).await;

        CleanupResults {
            remove_task,
            remove_container,
        }
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

        let mut command = Command::new(program);

        command.args(args).kill_on_drop(true);

        let output = tokio::time::timeout(timeout, command.output())
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
