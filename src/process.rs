use std::{fs, path::PathBuf};

use anyhow::Ok;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub state: String,
    pub rss_bytes: u64,
    pub cmdline: Vec<String>,
    pub exe: PathBuf,
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
}

fn status_field<'a>(status: &'a str, field: &str) -> anyhow::Result<&'a str> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .map(str::trim)
        .ok_or_else(|| anyhow::anyhow!("missing proc status field {field}"))
}
