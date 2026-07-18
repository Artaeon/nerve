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
use std::time::{Duration, Instant};

use chrono::Utc;

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

/// Safety margin added on top of a provider-reported quota reset time before
/// deferring the job. The provider's clock and ours aren't perfectly aligned,
/// and a job that wakes right AT the reset can still get a stale "still
/// limited" response — a small cushion avoids an immediate re-defer.
const QUOTA_RESET_MARGIN_SECS: u64 = 45;

/// Minimum defer when a reset time was parsed. Even a reset that (per our
/// clock) has already passed still needs a small wait — deferring by ~0s
/// would have the worker hammer the provider again immediately, no better
/// than the busy-retry this whole mechanism exists to avoid.
const QUOTA_MIN_WAIT_SECS: u64 = 60;

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
        // `claim_next` atomically finds-and-marks-Running in one step, so two
        // workers can never both pick up the same job — see its doc comment
        // for why that matters even though this loop is still single-threaded.
        match queue.claim_next() {
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

/// Entry point for `nerve --exec-agent`: read an [`ExecAgentRequest`] JSON from
/// stdin, run ONE full-tool agent in the requested repo, and write the resulting
/// [`HeadlessOutcome`] JSON to stdout. This runs as a FRESH child process spawned
/// by the decompose loop, so accumulated in-process state (the worker "wedge")
/// can't build up across a job's many steps. Logs go to stderr (set up in main),
/// so stdout carries only the outcome JSON.
pub async fn run_exec_agent() -> anyhow::Result<()> {
    use std::io::Read;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let req: crate::agent::headless::ExecAgentRequest = serde_json::from_str(&input)
        .map_err(|e| anyhow::anyhow!("exec-agent: invalid request JSON: {e}"))?;

    std::env::set_current_dir(&req.cwd)
        .map_err(|e| anyhow::anyhow!("exec-agent: cannot enter {}: {e}", req.cwd))?;

    let mut config = crate::config::Config::load().unwrap_or_default();
    default_deterministic_sampling(&mut config);
    let provider = crate::provider_setup::create_provider(&config, None)
        .map_err(|e| anyhow::anyhow!("exec-agent: no AI provider configured: {e}"))?;
    let provider: Arc<dyn AiProvider> = Arc::from(provider);

    let outcome = crate::agent::headless::run_headless_agent(
        &provider,
        &req.model,
        &req.task,
        req.max_iterations,
        req.timeout,
    )
    .await?;

    println!("{}", serde_json::to_string(&outcome)?);
    Ok(())
}

/// Run a single job, translating any failure into a `Failed` status with the
/// error recorded on the job (so the client can see why).
async fn run_one(queue: &Queue, job: Job) {
    let id = job.id.clone();
    let _ = queue.mark_running(&id);
    match execute(&job).await {
        Ok(Wedge::Healthy {
            changed_empty,
            verify,
            verify_summary,
            hit_iteration_cap,
        }) => {
            // Ground truth (the `changed` diff computed in `execute`, folded
            // together with `commits_ahead_of_base` — not the agent's
            // self-reported `outcome.edited`) decides the status. That
            // distinction is the whole point: three same-day incidents (jobs
            // db33f59e, 44558bc6, b3a4cf8a's predecessor) were each a job that
            // BELIEVED it had done something — a malformed tool call mistaken
            // for a final answer, or an edit reverted externally mid-run — and
            // reported `done` identically to a job that shipped real work.
            // Trusting the agent's own claim is exactly how those slipped
            // through, so only the actual diff/commits decide this.
            match status_for_run(changed_empty, verify, hit_iteration_cap) {
                crate::queue::JobStatus::NoChanges => {
                    let _ = queue.mark_no_changes(&id);
                    tracing::warn!(
                        "job {id} completed but changed NO files — reporting no-changes, not \
                         done; likely causes: the agent replied without acting (e.g. treated a \
                         malformed tool call as a final answer), or its edits were reverted \
                         externally mid-run"
                    );
                }
                // The gate did not green-light this code. It is committed on
                // the branch and it is REAL work, so this is not a failure —
                // but nobody has confirmed it is correct, so it must not read
                // as `done`. The reason travels with the job so a user seeing
                // `needs-review` does not have to go digging in the journal.
                crate::queue::JobStatus::NeedsReview => {
                    let _ = queue.mark_needs_review(&id, &verify_summary);
                    tracing::warn!(
                        "job {id} changed code but the verify gate did not approve it \
                         ({verify_summary}) — reporting needs-review, not done"
                    );
                }
                crate::queue::JobStatus::Done => {
                    let _ = queue.mark_done(&id);
                    tracing::info!("job {id} done");
                }
                // `status_for_run` is total over these three; the remaining
                // variants describe a job that never got here (still queued or
                // running) or one this arm does not decide (failed/cancelled).
                // Listed explicitly rather than with a catch-all so adding a
                // status breaks the compile here on purpose.
                crate::queue::JobStatus::Queued
                | crate::queue::JobStatus::Running
                | crate::queue::JobStatus::Failed
                | crate::queue::JobStatus::Cancelled => {
                    let _ = queue.mark_done(&id);
                    tracing::error!(
                        "job {id}: status_for_run returned an unexpected status; recorded as \
                         done — this is a bug"
                    );
                }
            }
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
                let msg = e.to_string();
                let now = Utc::now();
                // The provider often states the exact reset time (e.g. "resets
                // 12:30am (Europe/Berlin)") — use it instead of the blind
                // QUOTA_BACKOFF_SECS ceiling. Measured: a limit hit at 00:26
                // with an actual reset at 00:30 was, under the old blind
                // backoff, deferred to ~00:46 — 16 minutes of idle worker for
                // no reason. Falls back to the blind backoff when unparseable.
                let parsed_reset_secs = crate::ai::retry::parse_quota_reset(&msg, now)
                    .map(|dt| dt.timestamp().max(0) as u64);
                let until = quota_defer_until(now_secs(), parsed_reset_secs);
                let _ = queue.defer(&id, until, &format!("deferred (provider quota): {e}"));
                tracing::warn!(
                    "job {id}: provider quota/session limit — deferred {}s until the window \
                     resets: {e}",
                    until.saturating_sub(now_secs())
                );
            } else {
                let _ = queue.mark_failed(&id, &e.to_string());
                tracing::warn!("job {id} failed: {e}");
            }
        }
    }
}

/// Decide when to wake a job deferred by a provider quota/session-limit hit.
///
/// When the provider told us the exact reset instant (`parsed_reset_secs`),
/// wake `QUOTA_RESET_MARGIN_SECS` after it — a small cushion because the
/// provider's clock and ours aren't perfectly aligned, and waking right AT
/// the reset can still race a stale "still limited" response. The result is
/// CLAMPED to `[now + QUOTA_MIN_WAIT_SECS, now + QUOTA_BACKOFF_SECS]`: the
/// upper bound guarantees we never wait longer than the old blind backoff
/// (a parse that's wildly off — e.g. tomorrow instead of tonight — can't
/// strand the job), and the lower bound guarantees we never busy-retry (a
/// reset time already in the past, per clock skew, still gets a real wait).
///
/// When nothing parsed, falls back to the blind `QUOTA_BACKOFF_SECS` ceiling
/// exactly as before.
fn quota_defer_until(now: u64, parsed_reset_secs: Option<u64>) -> u64 {
    match parsed_reset_secs {
        Some(reset) => {
            let with_margin = reset.saturating_add(QUOTA_RESET_MARGIN_SECS);
            let min = now.saturating_add(QUOTA_MIN_WAIT_SECS);
            let max = now.saturating_add(QUOTA_BACKOFF_SECS);
            with_margin.clamp(min, max)
        }
        None => now.saturating_add(QUOTA_BACKOFF_SECS),
    }
}

/// What the verify gate actually concluded about this job's code.
///
/// These four are deliberately distinct. Collapsing any pair of them is how a
/// job whose gate REJECTED its code came to be reported with the same word as
/// a job the gate approved. In particular `NoGate` must never read as
/// `Passed`: one is the absence of a green light, not the presence of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyOutcome {
    /// The gate ran and the code passed it (possibly after fix rounds).
    Passed,
    /// The gate ran and the code still failed it after every fix round.
    Failed,
    /// There was no gate to run: no verify command could be detected for this
    /// project and `.nerve/verify.toml` declared none either.
    NoGate,
    /// The gate was not reached — the agent edited nothing, or auto-verify is
    /// switched off in config.
    NotRun,
}

/// The gate's verdict plus the human-readable line recorded in the journal.
struct VerifyReport {
    outcome: VerifyOutcome,
    summary: String,
}

impl VerifyReport {
    fn new(outcome: VerifyOutcome, summary: impl Into<String>) -> Self {
        Self {
            outcome,
            summary: summary.into(),
        }
    }
}

/// The final status for a run that did not wedge.
///
/// Pure and exhaustively tested, in the same style as `is_no_change` — the
/// status a user reads must be derivable from ground truth alone, never from
/// the agent's self-report.
///
/// `changed_empty` wins over everything: if the job changed nothing there is
/// no code to accept or reject, and `NoChanges` is the honest answer whatever
/// the gate said. Otherwise real work exists on the branch, and only a gate
/// that actually ran AND passed earns `Done`. Everything else — the gate said
/// no, or nothing ever checked — is `NeedsReview`: the work is real and a
/// human has to look at it.
fn status_for_run(
    changed_empty: bool,
    verify: VerifyOutcome,
    hit_iteration_cap: bool,
) -> crate::queue::JobStatus {
    if changed_empty {
        return crate::queue::JobStatus::NoChanges;
    }
    // Stopping at the cap means the agent ran out of steps MID-TASK. The gate
    // can still pass, because what it wrote compiles and lints — job d755d1bd
    // wrote 342 lines of a feature's library layer and no page, no tests and
    // no wiring, and reported `done`. The gate cannot see absence.
    //
    // The commit message already carries an [INCOMPLETE] marker, but a marker
    // buried in `git log` is not a status: anyone scanning `nerve --jobs` read
    // the same word as for a finished job. Real work exists on the branch, so
    // this is not a failure — it is exactly "a human has to look at it".
    if hit_iteration_cap {
        return crate::queue::JobStatus::NeedsReview;
    }
    match verify {
        VerifyOutcome::Passed => crate::queue::JobStatus::Done,
        VerifyOutcome::Failed | VerifyOutcome::NoGate | VerifyOutcome::NotRun => {
            crate::queue::JobStatus::NeedsReview
        }
    }
}

/// Whether a job ran on a healthy worker or hit the all-tools-failed wedge.
enum Wedge {
    /// Ran without hitting the wedge. `changed_empty` is the GROUND-TRUTH
    /// no-change signal computed in `execute` (uncommitted diff AND no
    /// commits ahead of base — not the agent's self-reported
    /// `outcome.edited`) — see the comment in `run_one` for why that
    /// distinction is the entire point of `JobStatus::NoChanges`.
    Healthy {
        changed_empty: bool,
        /// What the gate concluded. Before this existed, the gate's verdict
        /// was computed, logged, journaled — and then dropped, so a job whose
        /// gate failed reported `done`.
        verify: VerifyOutcome,
        /// The gate summary, so `run_one` can record WHY a job needs review
        /// instead of leaving the user to go read the journal.
        verify_summary: String,
        /// Whether the agent stopped at the iteration cap — i.e. ran out of
        /// steps mid-task. Ground truth from the run, not a self-report.
        hit_iteration_cap: bool,
    },
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
    // The base this job forks from — also the reference point for
    // `commits_ahead_of_base` below, so a --decompose job's per-step commits
    // are recognized as real work even when the working tree ends up clean
    // (see the comment at the `Wedge::Healthy` return for why).
    let base = base_branch(repo);
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
        let _ = git(repo, &["checkout", "-f", &base]);
        let _ = git(repo, &["reset", "--hard", &base]);
        let _ = git(repo, &["clean", "-fd", "-e", ".nerve"]);

        // Decide create-vs-resume EXPLICITLY rather than inferring it from a
        // failed `checkout -b`: that failure can mean many things (invalid
        // branch name, a stale ref lock, a D/F path conflict, a read-only
        // .git), not just "branch already exists". Treating every failure as
        // "already exists" meant that when the fallback checkout ALSO failed,
        // HEAD silently stayed on `base` (main) and the job's changes would
        // get committed straight there. `rev-parse --verify` is an
        // unambiguous existence check, so the two cases are no longer
        // conflated.
        let branch_exists = git(repo, &["rev-parse", "--verify", "--quiet", &branch]).is_ok();
        if branch_exists {
            // The branch already exists — this is a REQUEUED attempt. Its branch
            // may carry committed progress from a prior attempt (a decompose job
            // commits each finished sub-task, so a mid-run wedge doesn't discard
            // the steps already done). Switch to it and clean only UNCOMMITTED
            // dirt — do NOT reset to base, which would throw that progress away.
            // A normal job has no commits ahead of base, so this is equivalent to
            // the old reset for it.
            let _ = git(repo, &["checkout", "-f", &branch]);
            let _ = git(repo, &["clean", "-fd", "-e", ".nerve"]);
        } else {
            let _ = git(repo, &["checkout", "-b", &branch]);
        }

        // Never assume the checkout above landed us on the job branch — every
        // call above is best-effort (`let _ =`), so a failure there would
        // otherwise leave HEAD on `base`/main and the job's work would get
        // committed straight to it, defeating the whole point of this
        // isolation. Verify HEAD explicitly before doing any work and fail
        // the job loudly rather than silently commit to the wrong branch.
        let current_branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
            .map(|out| out.trim().to_string())
            .unwrap_or_default();
        if current_branch != branch {
            anyhow::bail!(
                "refusing to run job: expected HEAD on branch '{branch}' but found \
                 '{current_branch}' — branch checkout failed and running here could \
                 commit the job's changes to the wrong branch"
            );
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
    let (outcome, verify) = in_repo?;
    let verify_summary = verify.summary.clone();

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
    //
    // Generated build/cache output (`__pycache__`, `target/`,
    // `node_modules/`, etc. — see `is_generated_artifact`) is filtered OUT
    // here, never staged, even though the agent may legitimately have
    // written it as a side effect of running tests/builds while working.
    // Logged so a reviewer never wonders why a file the agent touched is
    // absent from the commit — silence there would be its own bug.
    let changed: Vec<String> = if outcome.edited && is_git {
        let raw: Vec<String> = dirty_paths(repo).difference(&pre_dirty).cloned().collect();
        let (kept, skipped): (Vec<String>, Vec<String>) =
            raw.into_iter().partition(|p| !is_generated_artifact(p));
        if !skipped.is_empty() {
            tracing::info!(
                "job {}: excluded {} generated artifact path(s) from the commit \
                 (build/cache output, never staged): {}",
                job.id,
                skipped.len(),
                skipped.join(", ")
            );
        }
        kept
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

    // A run counts as "changed" if EITHER it left an uncommitted diff OR its
    // branch is now ahead of `base` — a --decompose job COMMITS EACH STEP as
    // it goes (see `run_decomposed_agent`), so by the time we get here the
    // working tree is already clean and `changed` above is empty even though
    // the branch carries real, correct work. Job 116e5863 reported
    // `no-changes` for a decompose run that had committed several steps with
    // thousands of insertions — `no-changes` is the signature of a job that
    // did NOTHING, so reporting it for one that shipped and committed real
    // work was actively misleading. `commits_ahead_of_base` catches that work.
    let branch_ahead = is_git && commits_ahead_of_base(repo, &base) > 0;
    Ok(Wedge::Healthy {
        changed_empty: is_no_change(changed.is_empty(), branch_ahead),
        verify: verify.outcome,
        verify_summary: verify.summary,
        hit_iteration_cap: outcome.hit_max_iterations,
    })
}

/// Number of commits on the current HEAD that are not in `base` — i.e. the
/// commits this job produced (a --decompose job commits each step, so its
/// work lives here, not in the uncommitted diff).
fn commits_ahead_of_base(repo: &Path, base: &str) -> usize {
    git(repo, &["rev-list", "--count", &format!("{base}..HEAD")])
        .ok()
        .and_then(|out| out.trim().parse::<usize>().ok())
        .unwrap_or(0)
}

/// Ground truth for "this run changed nothing": true only when there is no
/// uncommitted diff AND the branch is not ahead of base. Either signal alone
/// proves real work happened — an uncommitted diff (a normal job, not yet
/// committed by `execute`) or committed-but-clean-tree commits (a --decompose
/// job, which commits per step — see job 116e5863). Factored out as a tiny
/// pure function so the decision is directly testable without a real repo.
fn is_no_change(changed_empty_dirty: bool, branch_ahead: bool) -> bool {
    changed_empty_dirty && !branch_ahead
}

/// Decide the final gate command from the auto-detected `base` (typecheck,
/// optionally chained with the test command — `None` if the project has
/// neither a recognized `Cargo.toml`/`package.json` shape) and the project's
/// declared `.nerve/verify.toml` `extra` steps.
///
/// Before this function existed, a project with no auto-detected base (e.g.
/// Python, Go — anything `detect_verify_command` doesn't recognize) got NO
/// gate at all, even if it had declared `extra` steps: the caller returned
/// early with "no verify command" before ever looking at `extra`. That made
/// a project's own declared gate — the entire point of which is to let a
/// project define verification `nerve` can't infer on its own — dead for
/// exactly the projects that most need it. Now:
///   - base + extra   → compose (unchanged behaviour)
///   - base, no extra → base alone (unchanged behaviour)
///   - no base, extra → the extras run ALONE as the gate
///   - no base, no extra → `None`, the only case with no gate at all
fn resolve_gate(base: Option<String>, extra: &[String]) -> Option<String> {
    if base.is_none() && extra.is_empty() {
        return None;
    }
    Some(crate::verify_project::compose_gate(
        &base.unwrap_or_default(),
        extra,
    ))
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
) -> anyhow::Result<(crate::agent::headless::HeadlessOutcome, VerifyReport)> {
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
        return Ok((outcome, VerifyReport::new(VerifyOutcome::NotRun, "not run")));
    }
    // The auto-detected base gate: the project's type-check, chained with its
    // test suite (a lint-only gate once let a job commit a *failing test* that
    // only human review caught). `None` when the project has no recognized
    // Cargo.toml/package.json shape at all.
    let base = config
        .verify_command
        .clone()
        .or_else(|| crate::verify::detect_verify_command(repo))
        .map(|typecheck| match crate::verify::detect_test_command(repo) {
            Some(test) => format!("{typecheck} && {test}"),
            None => typecheck,
        });
    // Project-declared extra verify steps (e.g. `npm run build`, or — for a
    // project `detect_verify_command` doesn't recognize at all, like Python
    // or Go — the ONLY gate it has) from `.nerve/verify.toml`.
    let extras = crate::verify_project::load_project_verify(repo).extra;
    let Some(cmd) = resolve_gate(base, &extras) else {
        return Ok((
            outcome,
            VerifyReport::new(VerifyOutcome::NoGate, "no verify command"),
        ));
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
            return Ok((outcome, VerifyReport::new(VerifyOutcome::Passed, note)));
        }
        rounds += 1;
        if rounds > crate::verify::MAX_VERIFY_ROUNDS {
            return Ok((
                outcome,
                VerifyReport::new(
                    VerifyOutcome::Failed,
                    format!("{cmd} → STILL FAILING after {} rounds", rounds - 1),
                ),
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

/// How long a verify command may run before it is killed. Deliberately NOT
/// `command_timeout_secs` (30s, see `headless.rs`): `.nerve/verify.toml` lets a
/// project append arbitrary steps (typically `npm run build`) to the verify
/// gate, and a real production build can legitimately take minutes — reusing
/// the 30s tool timeout would fail every good build. But it must still be
/// bounded: the worker is sequential, so a command that hangs (waiting on a
/// prompt, a lock, a dead network mirror) would otherwise block this one job
/// forever and take the entire queue down with it. 15 minutes is generous
/// enough not to punish a slow-but-healthy build, while still guaranteeing the
/// queue keeps moving.
const VERIFY_TIMEOUT_SECS: u64 = 15 * 60;

/// Run a verify command (a shell string like `cargo check` / `npm run -s lint`)
/// in `repo`, returning (success, combined stdout+stderr). Bounded by
/// `VERIFY_TIMEOUT_SECS` — see `run_verify_with_timeout` for why.
fn run_verify(repo: &Path, cmd: &str) -> (bool, String) {
    run_verify_with_timeout(repo, cmd, VERIFY_TIMEOUT_SECS)
}

/// Run a verify command with an explicit `timeout_secs` deadline (split out
/// from `run_verify` so tests can use a tiny deadline instead of waiting out
/// the real 15-minute budget).
///
/// We spawn the child with piped stdout/stderr and hand those pipes to a
/// background thread that reads them to completion via `.output()` — that
/// thread cannot deadlock the poll loop below because it never touches
/// `child`; it just blocks on the pipes, which is fine since it runs on its
/// own thread. Meanwhile we poll `child.try_wait()` on a short interval until
/// either the child exits or the deadline passes. On timeout we kill the
/// child (reaping it with `.wait()` so it doesn't linger as a zombie) and
/// return a failure message that names the cause, since this string is fed
/// straight back to the agent as a verify failure and must tell it what to do.
fn run_verify_with_timeout(repo: &Path, cmd: &str, timeout_secs: u64) -> (bool, String) {
    use std::process::Stdio;

    let mut cmd_builder = std::process::Command::new("sh");
    cmd_builder
        .arg("-c")
        .arg(cmd)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // `sh -c cmd` (e.g. `npm run build`) typically forks further children.
    // Killing only the immediate `sh` pid leaves those children running and
    // holding our pipes open, so the reader thread below would block forever
    // even after a "successful" kill. Put the whole tree in its own process
    // group so a timeout can take it all out at once, exactly as
    // `shell::run_command_with_timeout` does for the agent's own tools.
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd_builder.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = match cmd_builder.spawn() {
        Ok(c) => c,
        Err(e) => return (false, format!("could not run verify command: {e}")),
    };

    // Take the pipes now so the reader thread owns them; `child` keeps only
    // the handle we need for `try_wait`/`kill`.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut out = String::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_string(&mut out);
        }
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut out);
        }
        out
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                return (false, format!("could not wait on verify command: {e}"));
            }
        }
    };

    match status {
        Some(status) => {
            let combined = reader.join().unwrap_or_default();
            (status.success(), combined)
        }
        None => {
            // Timed out: kill the whole process group (not just the shell)
            // and reap it so it doesn't linger as a zombie, then join the
            // reader thread — it will hit EOF as soon as the pipes close,
            // which happens once every process holding them is gone.
            #[cfg(unix)]
            unsafe {
                let pid = child.id() as libc::pid_t;
                libc::kill(-pid, libc::SIGKILL);
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
            let combined = reader.join().unwrap_or_default();
            (
                false,
                format!(
                    "verify command timed out after {timeout_secs}s (killed): {cmd}\n\
                     this usually means a declared verify step hangs — check the extra \
                     steps in `.nerve/verify.toml`\n{combined}"
                ),
            )
        }
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

/// Directory names that, wherever they appear as a WHOLE path segment, mark
/// everything beneath them as generated build/cache output rather than
/// authored source. Covers the ecosystems nerve actually works in: Python
/// (`__pycache__`, `.pytest_cache`), Rust (`target`), Node
/// (`node_modules`, `.next`, `dist`, `build`).
const EXCLUDED_DIR_SEGMENTS: &[&str] = &[
    "__pycache__",
    ".pytest_cache",
    "target",
    "node_modules",
    ".next",
    "dist",
    "build",
];

/// Whether `path` (a repo-relative path as reported by `git status
/// --porcelain`) names generated build output, a cache, or editor/OS noise
/// that must NEVER be staged into a job's commit.
///
/// Real incident: a job on a Python project left `__pycache__/*.pyc`
/// bytecode dirty after running the test suite, `dirty_paths()` reported it
/// like any other change, and the commit step staged it verbatim — bytecode
/// landed in a reviewed branch. This function is the single source of truth
/// for what gets filtered out of that staged set (see the call site in
/// `execute` and `agent::headless::commit_step`). It does NOT restrict what
/// the agent is allowed to *write* — a job may legitimately create files
/// inside a build directory while working; this only governs what reaches
/// the commit.
///
/// Matches on path SEGMENTS, never substrings: `my_target_notes.md` and a
/// directory `src/building/` must NOT match despite containing "target" and
/// "build" as substrings — only an exact segment equal to `target` or
/// `build` counts. Matches at any depth (`a/b/__pycache__/c.pyc`).
pub(crate) fn is_generated_artifact(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments
        .iter()
        .any(|seg| EXCLUDED_DIR_SEGMENTS.contains(seg))
    {
        return true;
    }
    match segments.last() {
        Some(file) => {
            let lower = file.to_ascii_lowercase();
            lower.ends_with(".pyc") || lower.ends_with(".pyo") || lower == ".ds_store"
        }
        None => false,
    }
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

    // ── quota_defer_until ───────────────────────────────────────────────

    #[test]
    fn quota_defer_until_uses_parsed_reset_plus_margin_when_in_range() {
        let now = 1_000_000u64;
        // Reset 5 minutes out — well inside [now+min, now+max].
        let reset = now + 300;
        let until = quota_defer_until(now, Some(reset));
        assert_eq!(until, reset + QUOTA_RESET_MARGIN_SECS);
    }

    #[test]
    fn quota_defer_until_clamps_past_reset_up_to_min_wait() {
        let now = 1_000_000u64;
        // Reset already passed (per our clock) — must not defer by ~0s,
        // which would just hammer the provider again immediately.
        let reset = now - 500;
        let until = quota_defer_until(now, Some(reset));
        assert_eq!(until, now + QUOTA_MIN_WAIT_SECS);
    }

    #[test]
    fn quota_defer_until_clamps_far_future_reset_down_to_blind_backoff() {
        let now = 1_000_000u64;
        // A wildly-off parse (e.g. tomorrow instead of tonight) must never
        // make the job wait longer than the old blind backoff ceiling.
        let reset = now + 999_999;
        let until = quota_defer_until(now, Some(reset));
        assert_eq!(until, now + QUOTA_BACKOFF_SECS);
    }

    #[test]
    fn quota_defer_until_falls_back_to_blind_backoff_when_unparsed() {
        let now = 1_000_000u64;
        let until = quota_defer_until(now, None);
        assert_eq!(until, now + QUOTA_BACKOFF_SECS);
    }

    #[test]
    fn status_for_run_uses_ground_truth_changed_set() {
        // Empty changed set → NoChanges, never Done — this is the entire point
        // of the fix: three same-day incidents (jobs db33f59e, 44558bc6,
        // b3a4cf8a's predecessor) each reported `done` for a run that changed
        // nothing, because the agent's own belief that it had acted was
        // trusted instead of the actual diff.
        assert_eq!(
            status_for_run(true, VerifyOutcome::Passed, false),
            crate::queue::JobStatus::NoChanges
        );
        // Non-empty changed set + a gate that PASSED → Done.
        assert_eq!(
            status_for_run(false, VerifyOutcome::Passed, false),
            crate::queue::JobStatus::Done
        );
    }

    /// THE LIE THIS FIX EXISTS TO KILL.
    ///
    /// `run_in_repo` computed the gate's verdict, logged it and journaled it —
    /// and then dropped it. Only `changed_empty` crossed back to `run_one`, so
    /// a job whose `cargo check && cargo test` FAILED, which then burned every
    /// fix round and still failed, committed that code and reported `done`.
    /// That is worse than an unverified job: the gate ran, said no, and the
    /// system overrode it.
    #[test]
    fn a_failed_gate_is_never_reported_as_done() {
        let s = status_for_run(false, VerifyOutcome::Failed, false);
        assert_ne!(s, crate::queue::JobStatus::Done);
        assert_ne!(s, crate::queue::JobStatus::NoChanges);
        assert_eq!(s, crate::queue::JobStatus::NeedsReview);
    }

    #[test]
    fn a_passing_gate_with_changes_is_done() {
        assert_eq!(
            status_for_run(false, VerifyOutcome::Passed, false),
            crate::queue::JobStatus::Done
        );
    }

    /// Nothing changed → `NoChanges` whatever the gate said, because there is
    /// no code to accept or reject.
    #[test]
    fn no_changes_wins_over_any_gate_result() {
        for v in [
            VerifyOutcome::Passed,
            VerifyOutcome::Failed,
            VerifyOutcome::NoGate,
            VerifyOutcome::NotRun,
        ] {
            assert_eq!(
                status_for_run(true, v, false),
                crate::queue::JobStatus::NoChanges
            );
        }
    }

    /// A job that ran out of steps MID-TASK must not read as finished.
    ///
    /// Job d755d1bd wrote 342 lines of a feature's library layer — no page, no
    /// tests, no wiring — and reported `done`, because unused library code
    /// lints and compiles so the gate passed. The gate cannot see absence.
    #[test]
    fn hitting_the_iteration_cap_is_never_reported_as_done() {
        // Even with a gate that PASSED, stopping at the cap means incomplete.
        let s = status_for_run(false, VerifyOutcome::Passed, true);
        assert_ne!(s, crate::queue::JobStatus::Done);
        assert_eq!(s, crate::queue::JobStatus::NeedsReview);
    }

    /// ...but a job that changed nothing is still NoChanges, cap or not: there
    /// is no work on the branch for a human to review.
    #[test]
    fn iteration_cap_does_not_override_no_changes() {
        assert_eq!(
            status_for_run(true, VerifyOutcome::Passed, true),
            crate::queue::JobStatus::NoChanges
        );
    }

    /// The absence of a green light is not the presence of one. A project with
    /// no detectable gate and no `.nerve/verify.toml` produces code nothing
    /// checked, and that must be visibly different from code that passed.
    #[test]
    fn absent_gate_is_not_treated_as_a_passing_gate() {
        assert_ne!(
            status_for_run(false, VerifyOutcome::NoGate, false),
            status_for_run(false, VerifyOutcome::Passed, false)
        );
        assert_ne!(
            status_for_run(false, VerifyOutcome::NotRun, false),
            status_for_run(false, VerifyOutcome::Passed, false)
        );
    }

    // ── is_no_change ────────────────────────────────────────────────────
    // Exhaustive over the 2x2 truth table — this decision is the entire
    // point of the job-116e5863 fix, so every combination is pinned down.

    #[test]
    fn is_no_change_exhaustive() {
        // Clean tree, branch not ahead → genuinely no change.
        assert!(is_no_change(true, false));
        // Clean tree, but branch IS ahead of base → a --decompose job that
        // committed its steps; this is real work, not "no changes".
        assert!(!is_no_change(true, true));
        // Uncommitted diff present → real work regardless of branch state.
        assert!(!is_no_change(false, false));
        assert!(!is_no_change(false, true));
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

    // ── commits_ahead_of_base ───────────────────────────────────────────

    #[test]
    fn commits_ahead_of_base_counts_extra_commits_on_a_branch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]).unwrap();
        git(p, &["config", "user.email", "t@t"]).unwrap();
        git(p, &["config", "user.name", "t"]).unwrap();
        git(p, &["checkout", "-q", "-b", "main"]).unwrap();
        std::fs::write(p.join("f.txt"), "x").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "init"]).unwrap();

        git(p, &["checkout", "-q", "-b", "nerve/job-1"]).unwrap();
        // No extra commits yet — a normal job before it commits anything.
        assert_eq!(commits_ahead_of_base(p, "main"), 0);

        // Simulate a --decompose job committing two steps.
        std::fs::write(p.join("a.txt"), "1").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "step 1"]).unwrap();
        std::fs::write(p.join("b.txt"), "2").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "step 2"]).unwrap();
        assert_eq!(commits_ahead_of_base(p, "main"), 2);
    }

    #[test]
    fn commits_ahead_of_base_returns_zero_on_bad_base_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]).unwrap();
        git(p, &["config", "user.email", "t@t"]).unwrap();
        git(p, &["config", "user.name", "t"]).unwrap();
        std::fs::write(p.join("f.txt"), "x").unwrap();
        git(p, &["add", "-A"]).unwrap();
        git(p, &["commit", "-q", "-m", "init"]).unwrap();
        // A base ref that doesn't exist must not panic — just report 0.
        assert_eq!(commits_ahead_of_base(p, "does-not-exist"), 0);
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

    /// The gate must not be able to hang the queue.
    ///
    /// This test existed once already. `fb87069` added the timeout and this
    /// test; `86fd517` — a commit about not deleting `.nerve/` — rewrote
    /// `worker.rs` wholesale (+112/−140 for a one-line change) and silently
    /// deleted both. The gate then ran unbounded for a full day while two
    /// later commits documented their own behaviour as resting on it.
    ///
    /// The assertion is on ELAPSED TIME, not just the message: a `sleep 30`
    /// under a 1s deadline returns in ~1s if the child is genuinely killed,
    /// and takes the full 30s if the deadline is decorative. A test that only
    /// checked the returned string would pass either way.
    #[test]
    fn run_verify_kills_a_hanging_command_promptly() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        let (ok, out) = run_verify_with_timeout(dir.path(), "sleep 30", 1);
        let elapsed = start.elapsed();
        assert!(!ok);
        assert!(out.contains("timed out"));
        assert!(
            elapsed < Duration::from_secs(10),
            "expected the hung command to be killed promptly, took {elapsed:?}"
        );
    }

    /// A timeout must kill the whole process GROUP, not just the shell.
    ///
    /// `sh -c "npm run build"` forks children that hold our stdout/stderr
    /// pipes. Killing only the immediate `sh` pid leaves them running and the
    /// reader thread blocked on pipes that never close — so the "successful"
    /// kill would hang exactly the code meant to prevent hangs. Here the
    /// grandchild outlives the shell by 30s; if it is not group-killed, the
    /// reader thread pins this test for the full 30s.
    #[cfg(unix)]
    #[test]
    fn run_verify_timeout_kills_the_whole_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        let (ok, _) = run_verify_with_timeout(dir.path(), "sleep 30 & sleep 30", 1);
        let elapsed = start.elapsed();
        assert!(!ok);
        assert!(
            elapsed < Duration::from_secs(10),
            "a forked grandchild survived the kill and held the pipes: {elapsed:?}"
        );
    }

    #[test]
    fn verify_gate_composes_project_extra_steps_when_opted_in() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".nerve")).unwrap();
        std::fs::write(
            dir.path().join(".nerve").join("verify.toml"),
            r#"extra = ["echo hi"]"#,
        )
        .unwrap();
        let pv = crate::verify_project::load_project_verify(dir.path());
        assert_eq!(
            crate::verify_project::compose_gate("base", &pv.extra),
            "base && echo hi"
        );
    }

    #[test]
    fn verify_gate_is_unchanged_when_project_has_not_opted_in() {
        let dir = tempfile::tempdir().unwrap();
        let pv = crate::verify_project::load_project_verify(dir.path());
        assert_eq!(
            crate::verify_project::compose_gate("base", &pv.extra),
            "base"
        );
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

    // ── is_generated_artifact ───────────────────────────────────────────
    // Real incident: a Python job left __pycache__/*.pyc dirty and it was
    // staged verbatim into a reviewed branch. Segment-matching (not
    // substring) is the entire point — a file/dir that merely CONTAINS an
    // excluded name as a substring (e.g. "my_target_notes.md", "src/building/")
    // must NOT be treated as generated output.

    #[test]
    fn is_generated_artifact_matches_python_cache_output() {
        assert!(is_generated_artifact("__pycache__/foo.cpython-311.pyc"));
        assert!(is_generated_artifact("a/b/__pycache__/c.pyc"));
        assert!(is_generated_artifact("pkg/mod.pyc"));
        assert!(is_generated_artifact("pkg/mod.pyo"));
        assert!(is_generated_artifact(".pytest_cache/v/cache/lastfailed"));
    }

    #[test]
    fn is_generated_artifact_matches_rust_target_dir() {
        assert!(is_generated_artifact("target/debug/deps/nerve-abc123"));
        assert!(is_generated_artifact("crate/target/release/nerve"));
    }

    #[test]
    fn is_generated_artifact_matches_node_output() {
        assert!(is_generated_artifact("node_modules/lodash/index.js"));
        assert!(is_generated_artifact("frontend/.next/cache/x"));
        assert!(is_generated_artifact("dist/bundle.js"));
        assert!(is_generated_artifact("app/build/main.js"));
    }

    #[test]
    fn is_generated_artifact_matches_editor_and_os_noise() {
        assert!(is_generated_artifact(".DS_Store"));
        assert!(is_generated_artifact("a/b/.DS_Store"));
    }

    #[test]
    fn is_generated_artifact_rejects_substring_near_misses() {
        // Contains "target" as a substring but is not the segment "target".
        assert!(!is_generated_artifact("my_target_notes.md"));
        assert!(!is_generated_artifact("src/my_target_notes.md"));
        // A directory named "building", not "build".
        assert!(!is_generated_artifact("src/building/plan.md"));
        assert!(!is_generated_artifact("src/building/mod.rs"));
        // Contains "dist" as a substring.
        assert!(!is_generated_artifact("src/distance_calc.rs"));
        // Contains "node_modules" as a substring but isn't the segment.
        assert!(!is_generated_artifact("my_node_modules_doc.md"));
    }

    #[test]
    fn is_generated_artifact_rejects_ordinary_source_files() {
        assert!(!is_generated_artifact("src/main.rs"));
        assert!(!is_generated_artifact("README.md"));
        assert!(!is_generated_artifact("lib/foo.ts"));
        assert!(!is_generated_artifact("a file.ts"));
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
