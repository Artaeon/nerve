//! Talking to a remote nerve server over SSH.
//!
//! The transport is deliberately dumb and secure: we shell out to `ssh` (which
//! the user already trusts and has configured) and run the `nerve` client on the
//! server, which connects to the server's own local daemon socket. No new port,
//! no new auth. The TUI uses this to render a live "jobs queued / running"
//! indicator for a configured server, and to submit work to it.

use crate::queue::{Job, JobStatus};

/// A one-line summary of a remote server's queue, for the status indicator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteStatus {
    pub queued: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub total: usize,
}

impl RemoteStatus {
    /// Compact badge like `2 running · 5 queued` (omits zero groups). Empty
    /// string when the queue is empty.
    pub fn badge(&self) -> String {
        let mut parts = Vec::new();
        if self.running > 0 {
            parts.push(format!("{} running", self.running));
        }
        if self.queued > 0 {
            parts.push(format!("{} queued", self.queued));
        }
        if parts.is_empty() {
            "idle".to_string()
        } else {
            parts.join(" \u{00b7} ")
        }
    }
}

/// Summarize a list of jobs into counts by status.
pub fn summarize(jobs: &[Job]) -> RemoteStatus {
    let mut s = RemoteStatus {
        total: jobs.len(),
        ..Default::default()
    };
    for job in jobs {
        match job.status {
            JobStatus::Queued => s.queued += 1,
            JobStatus::Running => s.running += 1,
            JobStatus::Done => s.done += 1,
            JobStatus::Failed => s.failed += 1,
            JobStatus::Cancelled => {}
        }
    }
    s
}

/// Parse the JSON array printed by `nerve --jobs --json`. Tolerant of leading
/// log noise: it starts at the first `[`.
pub fn parse_jobs(output: &str) -> anyhow::Result<Vec<Job>> {
    let trimmed = output.trim();
    if trimmed.starts_with("ERR") {
        anyhow::bail!("server error: {trimmed}");
    }
    let start = trimmed
        .find('[')
        .ok_or_else(|| anyhow::anyhow!("no JSON array in server response: {trimmed}"))?;
    let jobs: Vec<Job> = serde_json::from_str(&trimmed[start..])?;
    Ok(jobs)
}

/// Build the `ssh` argument vector for running a `nerve` subcommand on `host`.
/// Kept separate so it is unit-testable without spawning ssh.
fn ssh_args<'a>(host: &'a str, nerve_args: &[&'a str]) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=8".into(),
        host.into(),
        "nerve".into(),
    ];
    args.extend(nerve_args.iter().map(|s| s.to_string()));
    args
}

/// Run `ssh <host> nerve <args...>` and return stdout, mapping a non-zero exit
/// (unreachable host, auth failure) into a readable error.
fn run_ssh(host: &str, nerve_args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("ssh")
        .args(ssh_args(host, nerve_args))
        .output()
        .map_err(|e| anyhow::anyhow!("could not launch ssh: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "ssh to '{host}' failed: {}",
            err.trim().lines().next().unwrap_or("unknown error")
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Fetch the remote server's jobs (`nerve --jobs --json` over SSH).
#[allow(dead_code)] // wired into the TUI /server command + status poll
pub fn fetch_jobs(host: &str) -> anyhow::Result<Vec<Job>> {
    let out = run_ssh(host, &["--jobs", "--json"])?;
    parse_jobs(&out)
}

/// Fetch and summarize in one call — what the status indicator needs.
#[allow(dead_code)]
pub fn fetch_status(host: &str) -> anyhow::Result<RemoteStatus> {
    Ok(summarize(&fetch_jobs(host)?))
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
