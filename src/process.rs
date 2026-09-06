use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Shim,
    Qemu,
    Virtiofsd,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub state: String,
    pub rss_bytes: u64,
    pub cmdline: Vec<String>,
    pub exe: PathBuf,
}

impl ProcessInfo {
    pub fn kind(&self) -> ProcessKind {
        match self.exe.file_name().and_then(|name| name.to_str()) {
            Some("containerd-shim-kata-v2") => ProcessKind::Shim,
            Some("qemu-system-x86_64") => ProcessKind::Qemu,
            Some("virtiofsd") => ProcessKind::Virtiofsd,
            _ => ProcessKind::Other,
        }
    }
}

pub struct ProcessCollector {
    proc_root: PathBuf,
}

impl ProcessCollector {
    pub fn new(proc_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
        }
    }

    pub fn read_process(&self, pid: u32) -> anyhow::Result<ProcessInfo> {
        let process_dir = self.proc_root.join(pid.to_string());

        let status_path = process_dir.join("status");
        let status = fs::read_to_string(&status_path).map_err(|error| {
            anyhow::anyhow!("failed to read {}: {error}", status_path.display())
        })?;

        let ppid = status_field(&status, "PPid:")?.parse::<u32>()?;

        let state = status_field(&status, "State:")?
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("process {pid} has no state"))?
            .to_string();

        let rss_kib = status_field(&status, "VmRSS:")?
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("process {pid} has no RSS"))?
            .parse::<u64>()?;

        let rss_bytes = rss_kib
            .checked_mul(1024)
            .ok_or_else(|| anyhow::anyhow!("RSS overflow for process {pid}"))?;

        let cmdline = fs::read(process_dir.join("cmdline"))?
            .split(|byte| *byte == 0)
            .filter(|argument| !argument.is_empty())
            .map(|argument| String::from_utf8_lossy(argument).into_owned())
            .collect();

        let exe = fs::read_link(process_dir.join("exe"))?;

        Ok(ProcessInfo {
            pid,
            ppid,
            state,
            rss_bytes,
            cmdline,
            exe,
        })
    }

    pub fn snapshot(&self) -> anyhow::Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();

        for entry in fs::read_dir(&self.proc_root)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };

            let Ok(pid) = file_name.parse::<u32>() else {
                continue;
            };

            let Ok(process) = self.read_process(pid) else {
                continue;
            };

            processes.push(process);
        }

        processes.sort_by_key(|process| process.pid);

        Ok(processes)
    }

    pub fn snapshot_kind(&self, kind: ProcessKind) -> anyhow::Result<Vec<ProcessInfo>> {
        Ok(self
            .snapshot()?
            .into_iter()
            .filter(|process| process.kind() == kind)
            .collect())
    }

    pub async fn wait_until_clean(
        &self,
        before: &[ProcessInfo],
        kind: ProcessKind,
        wait_timeout: std::time::Duration,
        poll_interval: std::time::Duration,
    ) -> anyhow::Result<()> {
        let mut remaining = Vec::new();

        let result = tokio::time::timeout(wait_timeout, async {
            loop {
                let current = self.snapshot_kind(kind)?;
                remaining = new_processes(before, &current);

                if remaining.is_empty() {
                    return Ok(());
                }

                tokio::time::sleep(poll_interval).await;
            }
        })
        .await;

        match result {
            Ok(result) => result,
            Err(_) => {
                let pids = remaining
                    .iter()
                    .map(|process| process.pid.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                Err(anyhow::anyhow!(
                    "timed out after {wait_timeout:?} waiting for \
                    new {kind:?} processes to disappear; \
                    remaining PIDs: {pids}"
                ))
            }
        }
    }
}

fn status_field<'a>(status: &'a str, field: &str) -> anyhow::Result<&'a str> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .map(str::trim)
        .ok_or_else(|| anyhow::anyhow!("missing proc status field {field}"))
}

pub fn new_processes(before: &[ProcessInfo], after: &[ProcessInfo]) -> Vec<ProcessInfo> {
    after
        .iter()
        .filter(|after_process| {
            !before.iter().any(|before_process| {
                before_process.pid == after_process.pid && before_process.exe == after_process.exe
            })
        })
        .cloned()
        .collect()
}
