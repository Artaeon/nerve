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

    // Snapshot which paths are already dirty BEFORE the job. We commit only what
    // the job itself changes (the set that becomes dirty during the run), never
    // `git add -A` — otherwise a job running on a checkout with unrelated
    // in-progress edits would sweep those into its commit. On a dedicated server
    // checkout this set is empty, so behaviour is unchanged.
    let pre_dirty = if is_git {
        dirty_paths(repo)
    } else {
        std::collections::HashSet::new()
    };

    // Tools operate on the process CWD. The worker is sequential, so switching
    // into the repo for the duration of the job is safe; restore afterwards.
    // The agent run AND the verify → fix loop both need CWD == repo, so keep it
    // set across the whole in-repo section and restore once at the end.
    let prev_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(repo)?;
    let in_repo = run_in_repo(&provider, &model, &task, timeout, &config, repo, job).await;
    if let Some(prev) = prev_cwd {
        let _ = std::env::set_current_dir(prev);
    }
    let (outcome, verify_summary) = in_repo?;

    // Commit ONLY the paths this job newly touched, on its own branch, for
    // review — leaving any pre-existing unrelated changes untouched.
    if outcome.edited && is_git {
        let changed: Vec<String> = dirty_paths(repo).difference(&pre_dirty).cloned().collect();
        if !changed.is_empty() {
            let mut add = vec!["add", "--"];
            add.extend(changed.iter().map(String::as_str));
            let _ = git(repo, &add);
            let msg = format!("nerve job {}: {}", job.id, first_line(&job.prompt));
            let _ = git(repo, &["commit", "-m", &msg]);
        }
    }

    // Record the job in the project's memory (`.nerve/activity.jsonl`) so the
    // work is journaled the same way the interactive agent journals its turns —
    // nothing is forgotten. Best-effort.
    let _ = crate::project::ProjectStore::for_workspace(repo).record_activity(
        &job.prompt,
        outcome.edited,
        &verify_summary,
    );

    tracing::info!(
        "job {} finished in {} iteration(s) [verify: {}]: {}",
        job.id,
        outcome.iterations,
        verify_summary,
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

/// Run the agent for `task`, then — if it edited files and auto-verify is on —
/// run the project's verify command and feed any failure back to the agent to
/// self-correct (up to `MAX_VERIFY_ROUNDS`), exactly like the interactive gate.
/// Returns the final outcome plus a human-readable verify summary for the log
/// and the activity journal. Assumes CWD is already the repo.
async fn run_in_repo(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    task: &str,
    timeout: u64,
    config: &crate::config::Config,
    repo: &Path,
    job: &Job,
) -> anyhow::Result<(crate::agent::headless::HeadlessOutcome, String)> {
    // Multi-agent workflow (planner → coder → reviewer) when requested, else a
    // single agent. The verify gate below applies to both.
    let mut outcome = if job.workflow {
        let wf =
            crate::agent::headless::run_workflow(provider, model, task, MAX_ITER, timeout).await?;
        crate::agent::headless::HeadlessOutcome {
            edited: wf.edited,
            iterations: wf.coder_iterations,
            final_response: format!("## Plan\n{}\n\n## Review\n{}", wf.plan, wf.review),
            hit_max_iterations: wf.hit_max_iterations,
        }
    } else {
        crate::agent::headless::run_headless_agent(provider, model, task, MAX_ITER, timeout).await?
    };

    if !outcome.edited || !config.auto_verify {
        return Ok((outcome, "not run".to_string()));
    }
    let Some(cmd) = config
        .verify_command
        .clone()
        .or_else(|| crate::verify::detect_verify_command(repo))
    else {
        return Ok((outcome, "no verify command".to_string()));
    };

    let mut rounds: u8 = 0;
    loop {
        let (ok, output) = run_verify(repo, &cmd);
        if ok {
            let note = if rounds == 0 {
                format!("{cmd} → passed")
            } else {
                format!("{cmd} → passed after {rounds} fix round(s)")
            };
            return Ok((outcome, note));
        }
        rounds += 1;
        if rounds > crate::verify::MAX_VERIFY_ROUNDS {
            return Ok((
                outcome,
                format!("{cmd} → STILL FAILING after {} rounds", rounds - 1),
            ));
        }
        tracing::info!("job verify failed (round {rounds}); asking the agent to fix");
        let fix_task = format!(
            "The verification command `{cmd}` failed after your changes:\n\n```\n{}\n```\n\n\
             Fix the code so this check passes. Do not revert unrelated work.",
            crate::agent::context::smart_truncate(output.trim(), 4000)
        );
        let fixed = crate::agent::headless::run_headless_agent(
            provider, model, &fix_task, MAX_ITER, timeout,
        )
        .await?;
        outcome.edited = outcome.edited || fixed.edited;
    }
}

/// Run a verify command (a shell string like `cargo check` / `npm run -s lint`)
/// in `repo`, returning (success, combined stdout+stderr).
fn run_verify(repo: &Path, cmd: &str) -> (bool, String) {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(repo)
        .output()
    {
        Ok(o) => {
            let mut combined = String::from_utf8_lossy(&o.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&o.stderr));
            (o.status.success(), combined)
        }
        Err(e) => (false, format!("could not run verify command: {e}")),
    }
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

/// The set of repo-relative paths git reports as dirty (modified, added,
/// deleted, or untracked). Used to diff the tree before vs. after a job so we
/// commit only what the job changed.
fn dirty_paths(repo: &Path) -> std::collections::HashSet<String> {
    git(repo, &["status", "--porcelain"])
        .map(|out| out.lines().filter_map(parse_status_path).collect())
        .unwrap_or_default()
}

/// Extract the path from a `git status --porcelain` line. Handles renames
/// (`R  old -> new` → the new path) and quoted paths.
fn parse_status_path(line: &str) -> Option<String> {
    let rest = line.get(3..)?.trim();
    if rest.is_empty() {
        return None;
    }
    // For a rename/copy the porcelain is "old -> new"; keep the new path.
    let path = rest.rsplit(" -> ").next().unwrap_or(rest);
    Some(path.trim_matches('"').to_string())
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

    #[test]
    fn run_verify_reports_success_and_failure() {
        let dir = tempfile::tempdir().unwrap();
        let (ok, _) = run_verify(dir.path(), "exit 0");
        assert!(ok);
        let (ok, out) = run_verify(dir.path(), "echo boom >&2; exit 1");
        assert!(!ok);
        assert!(out.contains("boom"));
    }

    #[test]
    fn parse_status_path_handles_the_porcelain_forms() {
        assert_eq!(
            parse_status_path(" M lib/foo.ts").as_deref(),
            Some("lib/foo.ts")
        );
        assert_eq!(parse_status_path("?? new.md").as_deref(), Some("new.md"));
        assert_eq!(
            parse_status_path("A  added.rs").as_deref(),
            Some("added.rs")
        );
        // Rename keeps the NEW path.
        assert_eq!(
            parse_status_path("R  old.ts -> src/new.ts").as_deref(),
            Some("src/new.ts")
        );
        // Quoted (paths with spaces/unicode) are unquoted.
        assert_eq!(
            parse_status_path("A  \"a file.ts\"").as_deref(),
            Some("a file.ts")
        );
        assert_eq!(parse_status_path(""), None);
        assert_eq!(parse_status_path(" M "), None);
    }

    fn make_job(prompt: &str, has_context: bool) -> Job {
        Job {
            id: "abc".into(),
            repo: "/r".into(),
            prompt: prompt.into(),
            status: crate::queue::JobStatus::Queued,
            branch: None,
            has_context,
            workflow: false,
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
