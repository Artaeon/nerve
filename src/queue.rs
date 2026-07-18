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
///
/// `JobStatus` is serde-persisted in the disk-backed queue (each job is a JSON
/// file under `~/.nerve/queue/`). Adding a variant is safe for EXISTING
/// entries on disk — they carry the old strings and still deserialize fine;
/// only a NEW entry using a variant unknown to an OLDER binary would fail to
/// parse, and the deploy process (worker restarts onto the new binary before
/// any job can reach that state) makes that a non-issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Waiting to be picked up by the worker.
    Queued,
    /// Currently being executed by the worker.
    Running,
    /// Finished successfully; changes committed to the job branch.
    Done,
    /// Ran to completion without error, but produced NO file changes. Not a
    /// failure — a job can legitimately conclude nothing needs doing — but it
    /// must never look like a job that shipped work, or a reviewer scanning
    /// statuses will trust an empty branch. Added after three same-day
    /// incidents (jobs db33f59e, 44558bc6, and b3a4cf8a's predecessor) where a
    /// job that did nothing — a malformed tool call mistaken for a final
    /// answer, or an edit reverted externally mid-run — reported `done`
    /// identically to a job that shipped real work, and only a manual diff of
    /// the empty branch caught it.
    NoChanges,
    /// The job changed code, but the verify gate did not give it a green
    /// light: either it ran and REJECTED the code (still failing after every
    /// fix round), or there was no gate to run at all (no verify command
    /// detected, no `.nerve/verify.toml`, or the gate was skipped because
    /// auto-verify is off). This is NOT a failure to run: the agent
    /// completed and, if a gate existed, it ran to completion. It is also NOT
    /// a clean success: nobody has confirmed the code is correct. The work
    /// is real and sits committed on the job's branch; it needs a HUMAN to
    /// review it before it's trusted. Added because a job whose gate said no
    /// was, up to this point, committed and reported `done` identically to a
    /// job the gate actually approved.
    NeedsReview,
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
            JobStatus::NoChanges => "no-changes",
            JobStatus::NeedsReview => "needs-review",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    /// Whether this is a terminal state (won't change without user action).
    #[allow(dead_code)] // consumed by the queue worker (next increment) + tests
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Done
                | JobStatus::NoChanges
                | JobStatus::NeedsReview
                | JobStatus::Failed
                | JobStatus::Cancelled
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
    /// Run this job by DECOMPOSING it into small sub-tasks executed in turn (the
    /// self-decompose loop) instead of one agent run. Best for cross-cutting
    /// edit-existing work. Defaulted for backward compat.
    #[serde(default)]
    pub decompose: bool,
    /// Unix seconds when the job was created.
    pub created_at: u64,
    /// Unix seconds when the worker started it.
    pub started_at: Option<u64>,
    /// Unix seconds when it reached a terminal state.
    pub finished_at: Option<u64>,
    /// Failure detail when `status == Failed`.
    pub error: Option<String>,
    /// How many times this job has been retried after a worker wedge. Bounds the
    /// auto-requeue so a genuinely-unrunnable job can't loop forever. Defaulted
    /// for backward compat with jobs written before auto-requeue existed.
    #[serde(default)]
    pub attempts: u32,
    /// Unix seconds before which the worker must NOT run this job. Set when a job
    /// is deferred (e.g. the provider reported a session/quota limit with a reset
    /// time) so it resumes automatically after the wait instead of failing and
    /// being lost. Defaulted for backward compat.
    #[serde(default)]
    pub not_before: Option<u64>,
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
        self.enqueue_inner(repo, prompt, false, false)
    }

    /// Add a new job that runs through the multi-agent workflow.
    pub fn enqueue_workflow(&self, repo: &str, prompt: &str) -> anyhow::Result<Job> {
        self.enqueue_inner(repo, prompt, true, false)
    }

    /// Add a new job that runs through the self-decompose loop.
    pub fn enqueue_decompose(&self, repo: &str, prompt: &str) -> anyhow::Result<Job> {
        self.enqueue_inner(repo, prompt, false, true)
    }

    fn enqueue_inner(
        &self,
        repo: &str,
        prompt: &str,
        workflow: bool,
        decompose: bool,
    ) -> anyhow::Result<Job> {
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
            decompose,
            created_at: now_secs(),
            started_at: None,
            finished_at: None,
            error: None,
            attempts: 0,
            not_before: None,
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

    /// The oldest job still `Queued` and due to run now — the next one the worker
    /// should pick up. Jobs deferred into the future (`not_before` past `now`,
    /// e.g. waiting out a provider quota reset) are skipped until their time.
    #[allow(dead_code)]
    pub fn next_queued(&self) -> anyhow::Result<Option<Job>> {
        let now = now_secs();
        Ok(self
            .list()?
            .into_iter()
            .find(|j| j.status == JobStatus::Queued && j.not_before.is_none_or(|t| t <= now)))
    }

    /// Defer a job to run no earlier than `until` (Unix seconds), keeping it
    /// `Queued` rather than failing it. Used when the provider reports a
    /// session/quota limit with a reset time: the job waits out the window and a
    /// later poll runs it automatically — nothing lost, no manual resubmit.
    pub fn defer(&self, id: &str, until: u64, reason: &str) -> anyhow::Result<Option<Job>> {
        let reason = reason.to_string();
        self.update(id, move |job| {
            job.status = JobStatus::Queued;
            job.started_at = None;
            job.not_before = Some(until);
            job.error = Some(reason);
        })
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

    /// Mark a job `NoChanges` and stamp `finished_at`. The worker uses this
    /// instead of `mark_done` when the job ran to completion without error but
    /// the ground-truth diff (`git status` before vs. after) was empty — see
    /// [`JobStatus::NoChanges`] for why this must never collapse into `Done`.
    #[allow(dead_code)]
    pub fn mark_no_changes(&self, id: &str) -> anyhow::Result<Option<Job>> {
        self.update(id, |job| {
            job.status = JobStatus::NoChanges;
            job.finished_at = Some(now_secs());
        })
    }

    /// Mark a job `NeedsReview` and stamp `finished_at`, recording why the
    /// gate never gave the code a green light (the failing verify command's
    /// output, or a note that nothing verified it). The worker uses this
    /// instead of `mark_done` when the job changed code but the gate didn't
    /// approve it — see [`JobStatus::NeedsReview`] for why this must never
    /// collapse into `Done`.
    pub fn mark_needs_review(&self, id: &str, note: &str) -> anyhow::Result<Option<Job>> {
        let note = note.to_string();
        self.update(id, move |job| {
            job.status = JobStatus::NeedsReview;
            job.finished_at = Some(now_secs());
            job.error = Some(note);
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

    /// Put a job back on the queue after a worker wedge, incrementing its retry
    /// counter, so a fresh worker picks it up again. Returns the new attempt
    /// count. Used by the reactive self-heal so a job that failed only because
    /// the worker process was wedged recovers automatically instead of needing a
    /// manual resubmit.
    pub fn requeue(&self, id: &str) -> anyhow::Result<u32> {
        let updated = self.update(id, |job| {
            job.status = JobStatus::Queued;
            job.started_at = None;
            job.finished_at = None;
            job.error = None;
            job.attempts += 1;
        })?;
        Ok(updated.map(|j| j.attempts).unwrap_or(0))
    }

    /// Reclaim jobs stuck in `Running` — orphaned by a worker crash, hang, or
    /// restart. The worker is single-instance, so on startup ANY `Running` job
    /// is definitionally orphaned (no live worker is processing it). Each is put
    /// back on the queue (incrementing `attempts`) up to `max_attempts`, past
    /// which it is marked failed so a job that reliably hangs/crashes the worker
    /// can't strand the queue forever. Returns the ids that were requeued.
    ///
    /// Without this, a restart left a `Running` job orphaned indefinitely and
    /// the worker skipped straight past it to the next `Queued` job — silently
    /// dropping work.
    pub fn reclaim_orphaned_running(&self, max_attempts: u32) -> anyhow::Result<Vec<String>> {
        let mut requeued = Vec::new();
        for job in self.list()?.into_iter() {
            if job.status != JobStatus::Running {
                continue;
            }
            if job.attempts >= max_attempts {
                let _ = self.mark_failed(
                    &job.id,
                    "worker crashed or hung on this job repeatedly; giving up after retries — \
                     please resubmit.",
                );
            } else {
                let _ = self.requeue(&job.id);
                requeued.push(job.id);
            }
        }
        Ok(requeued)
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

pub(crate) fn now_secs() -> u64 {
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
