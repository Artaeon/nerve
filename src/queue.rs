//! Persistent job queue for the nerve server (`--serve`).
//!
//! A *job* is one unit of autonomous work: run the agent on `repo` with
//! `prompt`, on its own `nerve/job-<id>` branch, and commit the result for the
//! user to review later. The queue persists each job as a JSON file under
//! `~/.nerve/queue/` so the server survives restarts and a client can submit
//! work, disconnect (close the laptop), and reconnect to find it done.
//!
//! This module is storage + state machine ONLY. The worker that actually
//! executes jobs (drains the queue, runs the agent loop, commits) is separate,
//! so this stays pure and easily tested.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Lifecycle of a queued job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Waiting to be picked up by the worker.
    Queued,
    /// Currently being executed by the worker.
    Running,
    /// Finished successfully; changes committed to the job branch.
    Done,
    /// Failed — see [`Job::error`].
    Failed,
    /// Cancelled by the user before it ran.
    Cancelled,
}

impl JobStatus {
    /// Human-readable single word, used in client listings.
    pub fn label(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    /// Whether this is a terminal state (won't change without user action).
    #[allow(dead_code)] // consumed by the queue worker (next increment) + tests
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Done | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

/// A single unit of autonomous work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Short unique id (also names the JSON file and the git branch).
    pub id: String,
    /// Absolute path to the repository on the server.
    pub repo: String,
    /// The instruction handed to the agent.
    pub prompt: String,
    pub status: JobStatus,
    /// The branch the worker isolates the job on (`nerve/job-<id>`).
    pub branch: Option<String>,
    /// Whether a full conversation-context snapshot is stored alongside this job
    /// (`<id>.context.json`) so the server can resume with everything the client
    /// had — nothing lost on handoff. Defaulted for backward compatibility with
    /// jobs written before context bundles existed.
    #[serde(default)]
    pub has_context: bool,
    /// Run this job through the multi-agent workflow (planner → coder →
    /// reviewer) instead of a single agent. Defaulted for backward compat.
    #[serde(default)]
    pub workflow: bool,
    /// Unix seconds when the job was created.
    pub created_at: u64,
    /// Unix seconds when the worker started it.
    pub started_at: Option<u64>,
    /// Unix seconds when it reached a terminal state.
    pub finished_at: Option<u64>,
    /// Failure detail when `status == Failed`.
    pub error: Option<String>,
}

impl Job {
    /// A compact one-line summary for client listings, e.g.
    /// `a1b2c3d4  running   /srv/repo  Add rate limiting to the API`.
    pub fn summary_line(&self) -> String {
        let prompt = crate::agent::context::smart_truncate(&self.prompt, 60);
        format!(
            "{}  {:<9} {}  {}",
            self.id,
            self.status.label(),
            self.repo,
            prompt.replace('\n', " ")
        )
    }
}

/// A directory-backed job queue.
pub struct Queue {
    root: PathBuf,
}

impl Queue {
    /// Open a queue rooted at `root` (created on first write).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The default queue directory: `~/.nerve/queue`. Anchored to HOME (like the
    /// daemon socket) so the server and a client compute the same path
    /// regardless of how each was launched.
    pub fn default_location() -> Self {
        let base = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
        Self::new(base.join(".nerve").join("queue"))
    }

    fn job_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    fn context_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.context.json"))
    }

    /// Add a new single-agent job in the `Queued` state and persist it.
    pub fn enqueue(&self, repo: &str, prompt: &str) -> anyhow::Result<Job> {
        self.enqueue_inner(repo, prompt, false)
    }

    /// Add a new job that runs through the multi-agent workflow.
    pub fn enqueue_workflow(&self, repo: &str, prompt: &str) -> anyhow::Result<Job> {
        self.enqueue_inner(repo, prompt, true)
    }

    fn enqueue_inner(&self, repo: &str, prompt: &str, workflow: bool) -> anyhow::Result<Job> {
        std::fs::create_dir_all(&self.root)?;
        // uuid v4 is collision-free; 8 hex chars is plenty to disambiguate and
        // stays readable in listings and branch names.
        let id: String = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        let job = Job {
            branch: Some(format!("nerve/job-{id}")),
            id,
            repo: repo.to_string(),
            prompt: prompt.to_string(),
            status: JobStatus::Queued,
            has_context: false,
            workflow,
            created_at: now_secs(),
            started_at: None,
            finished_at: None,
            error: None,
        };
        self.save(&job)?;
        Ok(job)
    }

    /// Attach a full conversation-context snapshot to a job (typically the
    /// client's session JSON) and persist it atomically alongside the job as
    /// `<id>.context.json`. Flips the job's `has_context` flag. This is the
    /// "nothing lost on handoff" path: the server later loads it to resume with
    /// everything the client had.
    pub fn save_context(&self, id: &str, context: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(format!(".{id}.context.tmp"));
        std::fs::write(&tmp, context)?;
        std::fs::rename(&tmp, self.context_path(id))?;
        self.update(id, |job| job.has_context = true)?;
        Ok(())
    }

    /// Load a job's context snapshot, or `None` if it has none.
    #[allow(dead_code)] // consumed by the queue worker (next increment) + tests
    pub fn load_context(&self, id: &str) -> anyhow::Result<Option<String>> {
        let path = self.context_path(id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(std::fs::read_to_string(path)?))
    }

    /// Persist a job atomically (write-temp-then-rename), so a crash mid-write
    /// never leaves a half-written JSON file that would fail to parse.
    pub fn save(&self, job: &Job) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let json = serde_json::to_string_pretty(job)?;
        let tmp = self.root.join(format!(".{}.tmp", job.id));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, self.job_path(&job.id))?;
        Ok(())
    }

    /// Load a single job by id, or `None` if it doesn't exist.
    pub fn get(&self, id: &str) -> anyhow::Result<Option<Job>> {
        let path = self.job_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&data)?))
    }

    /// All jobs, oldest first. Unparseable files are skipped rather than
    /// failing the whole listing.
    pub fn list(&self) -> anyhow::Result<Vec<Job>> {
        let mut jobs = Vec::new();
        if !self.root.exists() {
            return Ok(jobs);
        }
        for entry in std::fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(data) = std::fs::read_to_string(&path)
                && let Ok(job) = serde_json::from_str::<Job>(&data)
            {
                jobs.push(job);
            }
        }
        jobs.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(jobs)
    }

    // ── Worker-facing state transitions ─────────────────────────────────
    // These drive a job through its lifecycle. They are exercised by tests now
    // and consumed by the queue worker in the next increment (the worker loop
    // that drains the queue and runs the agent). Marked allow(dead_code) until
    // that lands so the -D warnings gate stays green.

    /// The oldest job still `Queued` — the next one the worker should run.
    #[allow(dead_code)]
    pub fn next_queued(&self) -> anyhow::Result<Option<Job>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|j| j.status == JobStatus::Queued))
    }

    /// Mark a job `Running` and stamp `started_at`. Returns the updated job.
    #[allow(dead_code)]
    pub fn mark_running(&self, id: &str) -> anyhow::Result<Option<Job>> {
        self.update(id, |job| {
            job.status = JobStatus::Running;
            job.started_at = Some(now_secs());
        })
    }

    /// Mark a job `Done` and stamp `finished_at`.
    #[allow(dead_code)]
    pub fn mark_done(&self, id: &str) -> anyhow::Result<Option<Job>> {
        self.update(id, |job| {
            job.status = JobStatus::Done;
            job.finished_at = Some(now_secs());
        })
    }

    /// Mark a job `Failed` with an error message.
    #[allow(dead_code)]
    pub fn mark_failed(&self, id: &str, error: &str) -> anyhow::Result<Option<Job>> {
        let error = error.to_string();
        self.update(id, move |job| {
            job.status = JobStatus::Failed;
            job.finished_at = Some(now_secs());
            job.error = Some(error);
        })
    }

    /// Cancel a job. Only jobs that haven't started yet can be cancelled;
    /// returns `true` if the job was queued and is now cancelled.
    pub fn cancel(&self, id: &str) -> anyhow::Result<bool> {
        let updated = self.update(id, |job| {
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Cancelled;
                job.finished_at = Some(now_secs());
            }
        })?;
        Ok(matches!(updated, Some(j) if j.status == JobStatus::Cancelled))
    }

    /// Load a job, apply `f`, and persist it. No-op (returns `None`) if the job
    /// doesn't exist.
    fn update<F: FnOnce(&mut Job)>(&self, id: &str, f: F) -> anyhow::Result<Option<Job>> {
        let Some(mut job) = self.get(id)? else {
            return Ok(None);
        };
        f(&mut job);
        self.save(&job)?;
        Ok(Some(job))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Path helper used by tests and callers that only need the directory.
#[allow(dead_code)]
pub fn root_for(base: &Path) -> PathBuf {
    base.join(".nerve").join("queue")
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
