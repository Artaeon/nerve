//! The queue worker: the piece that makes a nerve server actually *do* work.
//!
//! It runs inside the daemon (spawned by `start_daemon`). It drains the job
//! queue one job at a time: isolates each on its own `nerve/job-<id>` git
//! branch, runs the headless agent to completion in the repo, commits the
//! result for the user to review, and records the outcome back on the job.
//! Sequential by design — one job at a time keeps it predictable and lets us
//! switch the process CWD into the repo for tool execution safely.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::ai::provider::AiProvider;
use crate::queue::{Job, Queue};

/// How often to check for new work when the queue is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Drain the queue forever. Never returns; spawned as a background task.
pub async fn run_worker() {
    let queue = Queue::default_location();
    tracing::info!("nerve worker started; draining {:?}", "~/.nerve/queue");
    loop {
        match queue.next_queued() {
            Ok(Some(job)) => run_one(&queue, job).await,
            Ok(None) => tokio::time::sleep(POLL_INTERVAL).await,
            Err(e) => {
                tracing::warn!("worker: could not read queue: {e}");
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

/// Run a single job, translating any failure into a `Failed` status with the
/// error recorded on the job (so the client can see why).
async fn run_one(queue: &Queue, job: Job) {
    let id = job.id.clone();
    let _ = queue.mark_running(&id);
    match execute(&job).await {
        Ok(()) => {
            let _ = queue.mark_done(&id);
            tracing::info!("job {id} done");
        }
        Err(e) => {
            let _ = queue.mark_failed(&id, &e.to_string());
            tracing::warn!("job {id} failed: {e}");
        }
    }
}

async fn execute(job: &Job) -> anyhow::Result<()> {
    let repo = Path::new(&job.repo);
    if !repo.is_dir() {
        anyhow::bail!("repository path not found on the server: {}", job.repo);
    }

    // Isolate the job on its own branch so results never land on the working
    // branch unreviewed. Best-effort: create it, or switch to it if it exists.
    let is_git = git(repo, &["rev-parse", "--is-inside-work-tree"]).is_ok();
    let branch = job
        .branch
        .clone()
        .unwrap_or_else(|| format!("nerve/job-{}", job.id));
    if is_git && git(repo, &["checkout", "-b", &branch]).is_err() {
        let _ = git(repo, &["checkout", &branch]);
    }

    // Build the provider from the CURRENT on-disk config each run, so adding an
    // API key or authenticating the Claude CLI takes effect without a restart.
    let config = crate::config::Config::load().unwrap_or_default();
    let provider = crate::provider_setup::create_provider(&config, None)
        .map_err(|e| anyhow::anyhow!("no AI provider configured on the server: {e}"))?;
    let provider: Arc<dyn AiProvider> = Arc::from(provider);
    let model = config.default_model.clone();
    let timeout = config.command_timeout_secs;

    // If the client attached its conversation, fold it into the task so the
    // job resumes with that context instead of a bare prompt (nothing lost).
    let context = if job.has_context {
        Queue::default_location()
            .load_context(&job.id)
            .ok()
            .flatten()
    } else {
        None
    };
    let task = build_task(job, context.as_deref());

    // Tools operate on the process CWD. The worker is sequential, so switching
    // into the repo for the duration of the job is safe; restore afterwards.
    let prev_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(repo)?;

    let outcome =
        crate::agent::headless::run_headless_agent(&provider, &model, &task, MAX_ITER, timeout)
            .await;

    if let Some(prev) = prev_cwd {
        let _ = std::env::set_current_dir(prev);
    }

    let outcome = outcome?;

    // Commit whatever the agent changed, on the job branch, for review.
    if outcome.edited && is_git {
        let _ = git(repo, &["add", "-A"]);
        let msg = format!("nerve job {}: {}", job.id, first_line(&job.prompt));
        // A commit with nothing staged exits non-zero; that's fine (no-op).
        let _ = git(repo, &["commit", "-m", &msg]);
    }

    tracing::info!(
        "job {} finished in {} iteration(s): {}",
        job.id,
        outcome.iterations,
        first_line(&outcome.final_response)
    );
    if outcome.hit_max_iterations {
        tracing::warn!(
            "job {} stopped at the iteration cap ({MAX_ITER}); result may be incomplete",
            job.id
        );
    }

    Ok(())
}

/// Iteration cap for an unattended run.
const MAX_ITER: usize = crate::agent::headless::DEFAULT_MAX_ITERATIONS;

/// Run a git subcommand in `repo` (uses `-C`, not the process CWD, so it works
/// regardless of where the worker currently is). Returns stdout or an error
/// carrying stderr.
///
/// `-c safe.directory=*` is passed per-invocation because a synced repo is
/// usually owned by the client's uid (rsync preserves ownership) while the
/// daemon runs as a different user (often root); without this, git's
/// dubious-ownership guard refuses every command and branch isolation silently
/// breaks. Per-command trust avoids mutating global git config.
fn git(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .arg("-c")
        .arg("safe.directory=*")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// First line of a prompt, truncated, for a commit subject.
fn first_line(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or("").trim();
    crate::agent::context::smart_truncate(line, 60)
}

/// Fold an attached session snapshot into the task prompt so the job resumes
/// with the prior conversation. Best-effort: if there's no context, or it can't
/// be parsed, just returns the plain prompt.
fn build_task(job: &Job, context: Option<&str>) -> String {
    let Some(raw) = context else {
        return job.prompt.clone();
    };
    let Ok(session) = serde_json::from_str::<crate::session::Session>(raw) else {
        return job.prompt.clone();
    };
    let conv = session
        .conversations
        .get(session.active_conversation)
        .or_else(|| session.conversations.first());
    let Some(conv) = conv.filter(|c| !c.messages.is_empty()) else {
        return job.prompt.clone();
    };

    // Include the tail of the conversation (older turns are less relevant and
    // cost tokens), each message snippet-truncated.
    let start = conv.messages.len().saturating_sub(12);
    let mut preface = String::from(
        "## Prior conversation (context handed off from the client)\n\
         You are continuing this work. Recent exchange:\n\n",
    );
    for (role, content) in &conv.messages[start..] {
        let snippet = crate::agent::context::smart_truncate(content.trim(), 500);
        preface.push_str(&format!("**{role}:** {snippet}\n\n"));
    }
    format!("{preface}## Task\n{}", job.prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_takes_first_and_truncates() {
        assert_eq!(first_line("do a thing\nand then more"), "do a thing");
        let long = "x".repeat(200);
        assert!(first_line(&long).len() < 200);
    }

    #[test]
    fn git_errors_on_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        // A fresh temp dir is not a git work tree.
        assert!(git(dir.path(), &["rev-parse", "--is-inside-work-tree"]).is_err());
    }

    fn make_job(prompt: &str, has_context: bool) -> Job {
        Job {
            id: "abc".into(),
            repo: "/r".into(),
            prompt: prompt.into(),
            status: crate::queue::JobStatus::Queued,
            branch: None,
            has_context,
            created_at: 0,
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    #[test]
    fn build_task_without_context_is_just_the_prompt() {
        let job = make_job("do the thing", false);
        assert_eq!(build_task(&job, None), "do the thing");
    }

    #[test]
    fn build_task_ignores_unparseable_context() {
        let job = make_job("do the thing", true);
        assert_eq!(build_task(&job, Some("not json at all")), "do the thing");
    }

    #[test]
    fn build_task_folds_in_prior_conversation() {
        let session = r#"{"id":"s","conversations":[{"id":"c","title":"t","messages":[["user","refactor the auth module"],["assistant","I split hashing into its own file"]],"created_at":"2026-07-10T00:00:00Z"}],"active_conversation":0,"selected_model":"sonnet","selected_provider":"claude_code","agent_mode":true,"code_mode":false,"saved_at":"2026-07-10T00:05:00Z"}"#;
        let job = make_job("continue the refactor", true);
        let task = build_task(&job, Some(session));
        assert!(task.contains("Prior conversation"), "got: {task}");
        assert!(task.contains("refactor the auth module"));
        assert!(task.contains("## Task\ncontinue the refactor"));
    }
}
