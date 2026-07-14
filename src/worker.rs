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
use crate::queue::{Job, Queue, now_secs};

/// How often to check for new work when the queue is empty.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// How many jobs to run before proactively restarting the worker process.
///
/// The long-running worker accumulates in-process state that, past roughly a
/// dozen jobs' worth of tool activity, wedges it so every tool call fails (see
/// the reactive self-heal in `run_one`). Rather than only recover *after* a
/// wedge corrupts a job, we cycle the process well before that threshold: after
/// this many completed jobs the worker exits cleanly and systemd (Restart=always)
/// brings up a fresh one. The disk-backed queue means remaining jobs resume
/// seamlessly on the next process. Kept conservative — a restart costs ~1s and a
/// mid-batch wedge costs a whole job.
const RESTART_AFTER_JOBS: usize = 6;

/// How long to defer a job when the provider reports a quota / session-limit
/// exhaustion. The worker keeps polling, so once the usage window resets the job
/// runs on the next poll after this delay; if still limited it defers again. A
/// conservative value avoids hammering the provider while a window is closed.
const QUOTA_BACKOFF_SECS: u64 = 20 * 60;

/// Drain the queue forever (until a proactive restart). Spawned as a background
/// task inside the daemon.
pub async fn run_worker() {
    let queue = Queue::default_location();
    tracing::info!("nerve worker started; draining {:?}", "~/.nerve/queue");
    // A fresh worker means any job still marked `Running` was orphaned by the
    // previous process (crash, hang, or a proactive recycle mid-job). Reclaim
    // them onto the queue so nothing is stranded — bounded by MAX_WEDGE_RETRIES
    // so a job that reliably kills the worker eventually fails instead of looping.
    match queue.reclaim_orphaned_running(MAX_WEDGE_RETRIES) {
        Ok(ids) if !ids.is_empty() => {
            tracing::warn!(
                "worker: reclaimed {} orphaned running job(s): {ids:?}",
                ids.len()
            )
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("worker: could not reclaim orphaned jobs: {e}"),
    }
    let mut completed = 0usize;
    loop {
        match queue.next_queued() {
            Ok(Some(job)) => {
                run_one(&queue, job).await;
                completed += 1;
                if completed >= RESTART_AFTER_JOBS {
                    tracing::info!(
                        "worker: {completed} jobs done — recycling the process (fresh worker) \
                         before in-process state can accumulate into a wedge"
                    );
                    std::process::exit(0);
                }
            }
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
        Ok(Wedge::Healthy) => {
            let _ = queue.mark_done(&id);
            tracing::info!("job {id} done");
        }
        Ok(Wedge::Wedged) => {
            // Every tool call failed — the long-running worker process is wedged
            // (accumulated in-process state that only a fresh process clears).
            // A fresh process runs the identical job cleanly, so auto-requeue the
            // job (bounded by MAX_WEDGE_RETRIES) and EXIT for a systemd restart —
            // the disk-backed queue means the fresh worker picks it right back
            // up with no manual resubmit. Past the retry bound we mark it failed
            // so a genuinely-unrunnable job can't loop forever.
            match queue.requeue(&id) {
                Ok(attempts) if attempts <= MAX_WEDGE_RETRIES => {
                    tracing::error!(
                        "job {id}: worker wedged (all tools failed) — requeued (attempt \
                         {attempts}/{MAX_WEDGE_RETRIES}); exiting for a fresh worker"
                    );
                }
                _ => {
                    let _ = queue.mark_failed(
                        &id,
                        "worker wedged repeatedly on this job (every tool call failed); giving up \
                         after retries — please resubmit.",
                    );
                    tracing::error!(
                        "job {id}: wedged past {MAX_WEDGE_RETRIES} retries — marked failed; \
                         exiting for a fresh worker"
                    );
                }
            }
            std::process::exit(1);
        }
        Err(e) => {
            // A provider quota / session-limit exhaustion doesn't clear on retry —
            // it clears when the usage window resets. DEFER the job (keep it
            // Queued, gated behind `not_before`) so it resumes automatically once
            // the window reopens, instead of failing and losing the work. Any
            // other error is a real failure.
            if crate::ai::retry::is_quota_error(&e) {
                let until = now_secs() + QUOTA_BACKOFF_SECS;
                let _ = queue.defer(&id, until, &format!("deferred (provider quota): {e}"));
                tracing::warn!(
                    "job {id}: provider quota/session limit — deferred ~{}min until the window \
                     resets: {e}",
                    QUOTA_BACKOFF_SECS / 60
                );
            } else {
                let _ = queue.mark_failed(&id, &e.to_string());
                tracing::warn!("job {id} failed: {e}");
            }
        }
    }
}

/// Whether a job ran on a healthy worker or hit the all-tools-failed wedge.
enum Wedge {
    Healthy,
    Wedged,
}

async fn execute(job: &Job) -> anyhow::Result<Wedge> {
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
    if is_git {
        // Fork every job from a CLEAN base (the repo's main branch), not from
        // wherever HEAD happens to be. The worker is sequential and leaves HEAD
        // on the previous job's `nerve/job-*` branch — without this reset, each
        // job would branch off the last one's result and inherit (and possibly
        // build on) its changes, so one bad job silently contaminates every job
        // after it.
        //
        // CRUCIAL: start from a truly PRISTINE tree. `git checkout <base>` alone
        // leaves any uncommitted edits in place — and a job that hung mid-edit
        // and was then reclaimed carries that stale dirt into the retry. Stale
        // dirt lands in the `pre_dirty` snapshot below and is then EXCLUDED from
        // the commit (`changed = dirty − pre_dirty`), so the job's *real* edit is
        // silently dropped — producing a commit that references a change that
        // isn't there (seen live on job 962b6d94: service.ts + a test referenced
        // a template field whose defining edit was dropped, so the branch didn't
        // even compile while the log said "verify passed"). Force-checkout,
        // hard-reset, and clean untracked (non-ignored) files so `pre_dirty` is
        // empty and `changed` is exactly this job's diff. Safe on the dedicated
        // server copy the worker operates on, where any pre-existing dirt is a
        // stale leftover, never precious WIP.
        let base = base_branch(repo);
        let _ = git(repo, &["checkout", "-f", &base]);
        let _ = git(repo, &["reset", "--hard", &base]);
        let _ = git(repo, &["clean", "-fd"]);
        if git(repo, &["checkout", "-b", &branch]).is_err() {
            // The branch already exists — this is a REQUEUED attempt. Its branch
            // may carry committed progress from a prior attempt (a decompose job
            // commits each finished sub-task, so a mid-run wedge doesn't discard
            // the steps already done). Switch to it and clean only UNCOMMITTED
            // dirt — do NOT reset to base, which would throw that progress away.
            // A normal job has no commits ahead of base, so this is equivalent to
            // the old reset for it.
            let _ = git(repo, &["checkout", "-f", &branch]);
            let _ = git(repo, &["clean", "-fd"]);
        }
    }

    // Build the provider from the CURRENT on-disk config each run, so adding an
    // API key or authenticating the Claude CLI takes effect without a restart.
    let mut config = crate::config::Config::load().unwrap_or_default();
    default_deterministic_sampling(&mut config);
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

    // Wedge check: if every tool call failed, the worker process is in a bad
    // state that only a fresh restart clears. Nothing was written (all writes
    // failed), so there is nothing to commit or journal — bail out and let
    // `run_one` mark the job failed and restart the worker.
    if outcome.all_tools_failed {
        return Ok(Wedge::Wedged);
    }

    // Commit ONLY the paths this job newly touched, on its own branch, for
    // review — leaving any pre-existing unrelated changes untouched. The changed
    // set is also journaled below, so compute it whenever the job edited.
    let changed: Vec<String> = if outcome.edited && is_git {
        dirty_paths(repo).difference(&pre_dirty).cloned().collect()
    } else {
        Vec::new()
    };
    // When the agent stopped at the iteration cap, the commit may be a partial
    // result (it ran out of steps mid-task). Mark that plainly IN the commit
    // message so a reviewer scanning `git log` sees it — the verify gate proves
    // the code type-checks, not that the feature is complete.
    let incomplete_note = if outcome.hit_max_iterations {
        " [INCOMPLETE: stopped at iteration cap — review for missing work]"
    } else {
        ""
    };
    if !changed.is_empty() {
        let mut add = vec!["add", "--"];
        add.extend(changed.iter().map(String::as_str));
        let _ = git(repo, &add);
        let msg = format!(
            "nerve job {}: {}{}",
            job.id,
            first_line(&job.prompt),
            incomplete_note
        );
        let _ = git(repo, &["commit", "-m", &msg]);
    }

    // Record the job in the project's memory (`.nerve/activity.jsonl`) so the
    // work is journaled the same way the interactive agent journals its turns —
    // nothing is forgotten. We journal the *semantic* record: the agent's own
    // summary (what changed and why), the concrete files it touched, and the
    // iterations spent — not just that a job ran. Best-effort.
    let journal_verify = if outcome.hit_max_iterations {
        format!("{verify_summary} · INCOMPLETE (hit iteration cap)")
    } else {
        verify_summary.clone()
    };
    let _ = crate::project::ProjectStore::for_workspace(repo).record_activity_full(
        &job.prompt,
        outcome.edited,
        &journal_verify,
        &outcome.final_response,
        &changed,
        outcome.iterations,
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

    Ok(Wedge::Healthy)
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
            all_tools_failed: false,
        }
    } else if job.decompose {
        // Self-decompose: a planner splits the task into small sub-tasks, each
        // executed in turn — the systematic form of the "decompose cross-cutting
        // work" rule that makes edit-existing tasks succeed.
        crate::agent::headless::run_decomposed_agent(provider, model, task, MAX_ITER, timeout)
            .await?
    } else {
        crate::agent::headless::run_headless_agent(provider, model, task, MAX_ITER, timeout).await?
    };

    if !outcome.edited || !config.auto_verify {
        return Ok((outcome, "not run".to_string()));
    }
    let Some(typecheck) = config
        .verify_command
        .clone()
        .or_else(|| crate::verify::detect_verify_command(repo))
    else {
        return Ok((outcome, "no verify command".to_string()));
    };
    // Run the project's TEST suite too, chained after the type-check (fix types
    // first, then tests). A lint-only gate this session let a job commit a
    // *failing test* that only human review caught — the suite closes that gap
    // and its failures feed back into the same self-correct loop below. Skipped
    // for watch-mode test scripts (detect_test_command returns None).
    let cmd = match crate::verify::detect_test_command(repo) {
        Some(test) => format!("{typecheck} && {test}"),
        None => typecheck,
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

/// How many times to auto-requeue a job that failed to a worker wedge before
/// giving up. A fresh worker almost always runs it cleanly, so a small bound is
/// enough; the bound only guards against a job that somehow wedges every worker.
const MAX_WEDGE_RETRIES: u32 = 2;

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

/// The branch each job should be forked from — a clean base, never a prior
/// job's result branch. Prefers `main`, then `master`; otherwise falls back to
/// the branch currently checked out (unless that is itself a `nerve/job-*`
/// branch, in which case there is no better answer than the current HEAD and we
/// return it — the caller's checkout is best-effort anyway).
fn base_branch(repo: &Path) -> String {
    for cand in ["main", "master"] {
        if git(repo, &["rev-parse", "--verify", "--quiet", cand]).is_ok() {
            return cand.to_string();
        }
    }
    git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "HEAD".to_string())
}

/// Unattended runs default to deterministic sampling (temperature 0) on
/// providers that support it, so the same job produces the same result as
/// closely as the model allows — a background worker exists for reproducibility,
/// not creativity. An explicit operator-set temperature is always respected.
/// (The `claude_code` CLI exposes no temperature knob, so this is a no-op there;
/// its residual non-determinism is inherent and documented in DETERMINISM.md.)
fn default_deterministic_sampling(config: &mut crate::config::Config) {
    if config.temperature.is_none() {
        config.temperature = Some(0.0);
    }
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
    fn deterministic_sampling_defaults_temperature_but_respects_explicit() {
        // Unset temperature → defaults to 0 for reproducible unattended runs.
        let mut c = crate::config::Config {
            temperature: None,
            ..Default::default()
        };
        default_deterministic_sampling(&mut c);
        assert_eq!(c.temperature, Some(0.0));

        // An operator-set temperature is left untouched.
        let mut c2 = crate::config::Config {
            temperature: Some(0.7),
            ..Default::default()
        };
        default_deterministic_sampling(&mut c2);
        assert_eq!(c2.temperature, Some(0.7));
    }

    #[test]
    fn git_errors_on_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        // A fresh temp dir is not a git work tree.
        assert!(git(dir.path(), &["rev-parse", "--is-inside-work-tree"]).is_err());
    }

    #[test]
    fn base_branch_prefers_main_over_a_job_branch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]).unwrap();
        git(p, &["config", "user.email", "t@t"]).unwrap();
        git(p, &["config", "user.name", "t"]).unwrap();
        git(p, &["checkout", "-q", "-b", "main"]).unwrap();
        std::fs::write(p.join("f.txt"), "x").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "init"]).unwrap();
        // Simulate the worker having left HEAD on a previous job's branch.
        git(p, &["checkout", "-q", "-b", "nerve/job-deadbeef"]).unwrap();
        // The next job must still fork from main, not from the job branch.
        assert_eq!(base_branch(p), "main");
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
            decompose: false,
            created_at: 0,
            started_at: None,
            finished_at: None,
            error: None,
            attempts: 0,
            not_before: None,
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
