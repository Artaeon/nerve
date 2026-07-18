//! Headless agent runner — drives the agent loop to completion with no TUI.
//!
//! This is what the server's queue worker uses to actually execute a job. It
//! reuses the SAME primitives as the interactive loop — `tools_system_prompt`,
//! `parse_tool_calls`, `execute_tool`, and the `TOOL_RESULTS_PREFIX` feedback
//! format — so an autonomous run behaves identically to a run you'd drive by
//! hand in the TUI. No `App`, no terminal, no event loop.

use std::sync::Arc;

use crate::agent::pipeline::ToolPolicy;
use crate::agent::tools::{ToolResult, execute_tool, parse_tool_calls, tools_system_prompt};
use crate::ai::provider::{AiProvider, ChatMessage};

/// System prompt that frames the model as an autonomous worker. The tool
/// protocol itself comes from `tools_system_prompt()`, appended after this.
const AGENT_SYSTEM: &str = "You are Nerve, running autonomously as a background worker on a coding task. \
Be DECISIVE and EFFICIENT: read only the few files you genuinely need to match the project's \
conventions, then START WRITING code. Do NOT re-read files you have already read, and do NOT explore \
indefinitely — once you understand the pattern, implement it. Prefer making the change over gathering \
more context. Verify your work where you can (build/tests). \
You have a generous tool budget for this task — do not ration it, and do not stop early to save \
calls: keep going until the task is actually complete. The ONLY valid reasons to stop are (a) the \
task is genuinely done, or (b) you are truly blocked by missing information or a broken environment \
that no further tool call could resolve — and in case (b) you must first have actually attempted the \
change. When the task is complete, reply with a short plain-text summary of what you changed and \
STOP (emit no further tool calls).";

/// Tools that mutate the workspace — used to flag whether a run edited files.
const WRITE_TOOLS: &[&str] = &[
    "write_file",
    "edit_file",
    "run_command",
    "create_directory",
    "remember",
    "update_tasks",
];

/// Default safety cap on agent iterations for an unattended run. Real multi-file
/// features need room to read a few files, write several, and self-verify.
pub const DEFAULT_MAX_ITERATIONS: usize = 40;

/// Token budget above which the running history is compacted. Generous headroom
/// under Claude's ~200k window so a long autonomous job stays token-efficient
/// and never blows the context window, while keeping recent detail intact.
const CONTEXT_BUDGET_TOKENS: usize = 100_000;

/// Messages always kept verbatim at the head: the two system prompts + the
/// original task (the agent must never lose sight of what it was asked to do).
const HEAD_KEEP: usize = 3;

/// Most-recent messages always kept verbatim at the tail.
const TAIL_KEEP: usize = 6;

/// How many times to nudge a full-tool agent that replied with prose but made
/// no changes, before accepting it as genuinely done.
const MAX_NUDGES: usize = 2;

/// After this many tool iterations with no edit yet, nudge a full-tool agent to
/// stop exploring and start implementing. The single biggest token sink observed
/// in practice is the model reading/searching a dozen-plus files before it makes
/// its first change — and because each iteration re-sends the growing context,
/// exploration cost is roughly quadratic. Deliberately framed as "you have
/// enough context, proceed" — never as a *limit*, which makes the model
/// confabulate a tool cap and quit (see the anti-confabulation nudge below).
///
/// This nudge USED to fire only once, ever (a `bool` latch). That was too
/// weak: job e2915f05 — a trivial one-utility-class CSS fix — got exactly one
/// gentle nudge at iteration 8, then explored the REMAINING 32 iterations
/// under no further pressure and finished the full 40-iteration run having
/// made ZERO edits (tool census: run_command 17, search_code 9, read_lines 7,
/// read_file 6, edit_file 0, write_file 0). The nudge now RE-FIRES every
/// `EXPLORE_NUDGE_INTERVAL` iterations for as long as the agent keeps
/// exploring without editing, escalating its wording each time (see the call
/// site below) — while still never forcing action or naming a limit.
const EXPLORE_NUDGE_AFTER: usize = 8;

/// How often the explore-nudge RE-FIRES after its first trigger, as long as
/// the agent is still exploring without having edited anything. See
/// `EXPLORE_NUDGE_AFTER` for why a one-shot nudge was not enough (job
/// e2915f05).
const EXPLORE_NUDGE_INTERVAL: usize = 6;

/// Bound the running message history so a long job stays token-efficient.
///
/// Strategy: keep the system prompts + task (head) and the most recent
/// exchanges (tail) verbatim; replace the *content* of older tool-RESULT
/// messages (the big file/command dumps — role `"user"`) with a short stub once
/// the total is over `budget_tokens`. The model keeps its own reasoning (every
/// `assistant` turn is untouched) and can cheaply re-read a file if it truly
/// needs it again — so we shed the expensive raw bytes without losing the plot.
/// Idempotent: re-stubbing an already-stubbed message is a no-op.
/// Returns true when it actually stubbed something away — the caller uses that to
/// drop its re-read cache, because content that is no longer in the conversation
/// must be sent in full if the model asks for it again.
fn compact_context(messages: &mut [ChatMessage], budget_tokens: usize) -> bool {
    const STUB: &str =
        "[earlier tool output compacted to save context — re-read the file if you need it again]";

    let total = |msgs: &[ChatMessage]| -> usize {
        msgs.iter()
            .map(|m| crate::agent::context::ContextManager::estimate_tokens(&m.content))
            .sum()
    };
    if total(messages) <= budget_tokens {
        return false;
    }

    let mut stubbed = false;
    let shrinkable = |m: &ChatMessage| m.role == "user" && m.content.len() > STUB.len() + 100;

    // Pass 1 — the cheap win: stub OLD tool output (between the head and the
    // recent tail). The model keeps its own reasoning (assistant turns are never
    // touched) and can re-read a file if it truly needs it again.
    if messages.len() > HEAD_KEEP + TAIL_KEEP {
        let cut_end = messages.len() - TAIL_KEEP;
        for msg in &mut messages[HEAD_KEEP..cut_end] {
            if shrinkable(msg) {
                msg.content = STUB.to_string();
                stubbed = true;
            }
        }
        if total(messages) <= budget_tokens {
            return stubbed;
        }
    }

    // Pass 2 — the tail alone can blow the budget: a single tool result may be
    // MAX_TOOL_OUTPUT_CHARS (50k), so six of them in the "protected" tail is ~100k
    // tokens by itself. Passing 1 only was enough to return having freed nothing,
    // and the very next provider call would overflow the window and kill the job.
    // Shrink tail tool output oldest-first, keeping the MOST RECENT result intact
    // (that's the one the model is actually reasoning about right now), and stop
    // as soon as we're under budget.
    let last = messages.len().saturating_sub(1);
    for i in HEAD_KEEP..last {
        if total(messages) <= budget_tokens {
            return stubbed;
        }
        if shrinkable(&messages[i]) {
            messages[i].content = STUB.to_string();
            stubbed = true;
        }
    }

    // Pass 3 — still over: even the newest result is too big on its own. Truncate
    // it rather than let the request overflow; a clipped result the model can act
    // on beats a hard provider error that fails the whole job.
    if total(messages) > budget_tokens && shrinkable(&messages[last]) {
        const NOTE: &str = "\n[truncated to fit the context budget — re-read if you need more]";
        let est = crate::agent::context::ContextManager::estimate_tokens;
        // Budget the NOTE itself, or we'd land just over and still overflow.
        let allowance = budget_tokens.saturating_sub(total(&messages[..last]) + est(NOTE));
        // `estimate_tokens` is chars/3 + 1 — invert it conservatively.
        let keep_chars = allowance.saturating_sub(1).saturating_mul(3);
        if messages[last].content.chars().count() > keep_chars {
            let head: String = messages[last].content.chars().take(keep_chars).collect();
            messages[last].content = format!("{head}{NOTE}");
            stubbed = true;
        }
    }

    // The HEAD (system prompts + the original task) is never touched: losing the
    // goal to save space would defeat the whole run.
    stubbed
}

/// Tools whose output is a verbatim slice of a file — the only ones worth
/// de-duplicating, since an identical repeat is provably redundant.
const CACHEABLE_READS: &[&str] = &["read_file", "read_lines"];

/// Fingerprint of a tool call (tool + its args), so an identical repeat of the
/// SAME read can be recognised.
fn call_fingerprint(call: &crate::agent::tools::ToolCall) -> String {
    let mut args: Vec<(&String, &String)> = call.args.iter().collect();
    args.sort();
    format!("{}|{args:?}", call.tool)
}

fn content_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Read-shaped tools whose PATH argument is worth tracking for the
/// stuck-on-one-file detector below. Deliberately broader than
/// `CACHEABLE_READS`: that dedupe only helps on a byte-identical repeat
/// (same tool, same args), so `read_file(a.rs)` then `read_lines(a.rs, 10,
/// 30)` then `read_lines(a.rs, 40, 60)` — three DIFFERENT fingerprints —
/// never trips it, yet is exactly the "reading the same file three ways"
/// loop observed in job 5029e587. `run_command` is deliberately excluded:
/// extracting a path out of an arbitrary shell command (e.g. `sed -n
/// '10,30p' src/remote.rs`) would need a real shell parser to do safely, and
/// a wrong guess (flagging the wrong file, or crashing on quoting) is worse
/// than the gap of simply not counting it. So a run that reads a file only
/// via `run_command` will not trip this — a known, accepted blind spot.
const PATH_TRACKED_READS: &[&str] = &["read_file", "read_lines", "search_code", "find_files"];

/// After a single file has been accessed (by path, across ANY read-shaped
/// tool) this many times in one run, the agent is almost certainly stuck
/// re-reading it rather than acting on it — job 5029e587 read `src/remote.rs`
/// four different ways (read_file, read_lines twice, a `sed` run_command)
/// across 20+ iterations and never opened the third file it was asked to
/// change, shipping a half-wired fix. Small enough to catch the loop while
/// it is still cheap to correct, large enough that a legitimate "read the
/// whole file, then re-check two different line ranges" pattern doesn't
/// false-fire.
const STUCK_ON_FILE_THRESHOLD: usize = 4;

/// How often the stuck-on-file nudge RE-FIRES once tripped, mirroring
/// `EXPLORE_NUDGE_INTERVAL`: prod, don't nag. Without spacing, a run that
/// keeps re-reading the same file past the threshold would get the nudge
/// appended every single iteration, drowning out the rest of the tool
/// feedback.
const STUCK_ON_FILE_NUDGE_INTERVAL: usize = 6;

/// Pure decision function: given how many times each file path has been
/// accessed so far in this run (via any [`PATH_TRACKED_READS`] tool), decide
/// whether to nudge about the single most-read path, and what to say. Kept
/// free of any provider/agent-loop dependency so it is unit-testable without
/// a mock provider — the counting and the trip decision are the whole
/// behaviour under test.
///
/// Returns `None` when nothing has crossed the threshold. Otherwise returns
/// the offending path and a message that NAMES it — a generic "stop
/// exploring" already exists as the explore-nudge and, per job 5029e587,
/// does not help once `edited` is already true (see the call site).
fn stuck_on_file_nudge(
    accesses: &std::collections::HashMap<String, usize>,
) -> Option<(String, String)> {
    let (path, &count) = accesses
        .iter()
        .filter(|&(_, &c)| c >= STUCK_ON_FILE_THRESHOLD)
        .max_by_key(|&(_, &c)| c)?;
    Some((
        path.clone(),
        format!(
            "You have now accessed `{path}` {count} times in this run. The information you need \
             from it is almost certainly already in front of you — either make the edit to \
             `{path}` now, or if you already have, move on to another file this task needs. Do \
             not read `{path}` again without a specific new reason."
        ),
    ))
}

/// Assemble the repo's persisted `.nerve/` knowledge into a compact block so a
/// headless job honors the project's conventions, design system, and prior
/// decisions — AND, unlike before, actually recalls memory relevant to this
/// specific task. Reads the store rooted at the current working directory
/// (the worker sets CWD to the repo before the run). Returns `None` when there
/// is no `.nerve/` memory.
///
/// This used to build its own context block by hand and never called
/// `memory_recall::recall` at all (`grep -c recall src/agent/headless.rs` →
/// 0 — the `recall` tool this worker exposed was called 0 times in 2,362 real
/// tool calls). It now goes through `project_context::build`, the single
/// shared implementation also used by the interactive TUI, so the two can
/// never silently diverge again.
fn project_memory_context(task: &str) -> Option<String> {
    let root = std::env::current_dir().ok()?;
    project_memory_context_from(&crate::project::ProjectStore::for_workspace(&root), task)
}

/// Testable core of [`project_memory_context`]: assemble from an explicit
/// store. Thin wrapper around `project_context::build` — always recalls
/// memory relevant to `task` and always includes design principles (a
/// headless job has no single "current message" to gate design guidance on,
/// and design work is common enough in autonomous jobs that omitting it
/// silently is the wrong default).
pub(crate) fn project_memory_context_from(
    store: &crate::project::ProjectStore,
    task: &str,
) -> Option<String> {
    let opts = crate::project_context::ContextOptions {
        recall_query: Some(task),
        include_design: true,
    };
    let sections = crate::project_context::build(store, &opts);
    if sections.is_empty() {
        return None;
    }
    Some(format!(
        "PROJECT KNOWLEDGE (from this repo's .nerve/ memory — honor it so your work stays \
         consistent with the project's conventions, design system, and decisions):\n\n{}",
        sections.join("\n\n")
    ))
}

/// The result of a headless agent run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeadlessOutcome {
    /// Number of tool-executing iterations that ran.
    pub iterations: usize,
    /// Whether any write/mutating tool was invoked.
    pub edited: bool,
    /// The model's final plain-text response (its summary).
    pub final_response: String,
    /// Whether the run stopped because it hit the iteration cap (vs. finished).
    pub hit_max_iterations: bool,
    /// True when the run executed several tool rounds but the TOOL LAYER
    /// never once proved itself working — the signature of a wedged worker
    /// process (every tool, even `read_file`, failing to run at all). A
    /// `run_command` whose command merely exited non-zero (a failing
    /// build/test — a completely normal opening move) does NOT count against
    /// this: the tool ran and reported a real result, which proves the layer
    /// is fine (see `tool_layer_worked`). Only a tool that never truly ran is
    /// evidence of a wedge. The worker uses this to self-heal via a fresh
    /// restart.
    pub all_tools_failed: bool,
}

/// Request handed to a fresh `nerve --exec-agent` child process: run ONE
/// full-tool agent for `task` in `cwd`, then exit. Serialized over the child's
/// stdin; the child writes a [`HeadlessOutcome`] JSON back on stdout.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecAgentRequest {
    pub task: String,
    pub model: String,
    pub max_iterations: usize,
    pub timeout: u64,
    pub cwd: String,
}

/// Result of trying to run a decompose step in an isolated child process.
///
/// Distinguishing "never started" from "started then failed" matters: if the
/// child already ran, it may have half-applied edits to the repo, and running
/// the SAME step again in-process on top of that half-finished state would
/// double-run it — duplicate imports/functions, or an agent thrashing against
/// its own unfinished work. Only a step that truly never started is safe to
/// retry in-process.
enum StepExec {
    /// The child ran and returned its outcome.
    Ran(HeadlessOutcome),
    /// The child STARTED (and may have already edited the repo) but failed.
    /// The step must NOT be retried in-process — that would run it twice.
    Failed,
    /// The child never started; nothing ran, so it is safe to run in-process.
    NotSpawned,
}

/// Run one agent step in a FRESH child process (`nerve --exec-agent`), so the
/// long-running worker's "wedge" (accumulated in-process state that eventually
/// makes every tool fail) cannot build up across a decompose job's many steps —
/// each step gets a pristine process.
///
/// Returns [`StepExec::NotSpawned`] only for failures BEFORE the child process
/// actually started (nothing ran yet, so the caller can safely fall back to
/// running the step in-process). Once `spawn()` has succeeded, any later
/// failure returns [`StepExec::Failed`] instead — the child may already have
/// edited the repo, so re-running the step would risk running it twice on top
/// of its own half-applied changes.
async fn exec_step_subprocess(
    task: &str,
    model: &str,
    max_iterations: usize,
    timeout: u64,
) -> StepExec {
    use tokio::io::AsyncWriteExt;

    // Nothing has started yet — these can all fall back to in-process safely.
    let Some(cwd) = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
    else {
        return StepExec::NotSpawned;
    };
    let req = ExecAgentRequest {
        task: task.to_string(),
        model: model.to_string(),
        max_iterations,
        timeout,
        cwd,
    };
    let Ok(req_json) = serde_json::to_string(&req) else {
        return StepExec::NotSpawned;
    };
    let Ok(exe) = std::env::current_exe() else {
        return StepExec::NotSpawned;
    };

    let Ok(mut child) = tokio::process::Command::new(exe)
        .arg("--exec-agent")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit()) // logs still flow to journald
        // If we bail out below, don't leave the child running unsupervised
        // against the repo — kill it rather than let it keep editing behind
        // our back once we've decided not to use its result.
        .kill_on_drop(true)
        .spawn()
    else {
        return StepExec::NotSpawned;
    };

    // From here on, the child HAS started and may already be touching the
    // repo — any failure below must map to `Failed`, not `NotSpawned`, so the
    // caller never re-runs this step in-process on top of a half-edited tree.

    // The request is small (fits the pipe buffer), so write-then-wait can't
    // deadlock. Drop stdin to signal EOF before collecting output.
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(req_json.as_bytes()).await.is_err() || stdin.shutdown().await.is_err() {
            tracing::warn!(
                "exec-agent child failed to receive its request after starting; NOT retrying \
                 in-process — the child may already have modified the repo"
            );
            return StepExec::Failed;
        }
    }
    let output = match child.wait_with_output().await {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                "exec-agent child failed while awaiting output ({e}); NOT retrying in-process — \
                 the child may already have modified the repo"
            );
            return StepExec::Failed;
        }
    };
    if !output.status.success() {
        tracing::warn!(
            "exec-agent child exited unsuccessfully; NOT retrying in-process — the child may \
             already have modified the repo"
        );
        return StepExec::Failed;
    }
    match serde_json::from_slice::<HeadlessOutcome>(&output.stdout) {
        Ok(outcome) => StepExec::Ran(outcome),
        Err(e) => {
            tracing::warn!(
                "exec-agent child's output failed to parse ({e}); NOT retrying in-process — the \
                 child may already have modified the repo"
            );
            StepExec::Failed
        }
    }
}

/// Char-safe truncation of tool output, matching the interactive runner's cap.
/// The cap equals `read_file`'s own cap (`MAX_TOOL_OUTPUT_CHARS`) so a file the
/// model just read is passed back whole rather than clipped to a smaller window.
fn truncate_output(output: &str) -> String {
    use crate::agent::tools::fs::MAX_TOOL_OUTPUT_CHARS;
    if output.len() > MAX_TOOL_OUTPUT_CHARS {
        let head: String = output.chars().take(MAX_TOOL_OUTPUT_CHARS).collect();
        format!(
            "{head}...\n[Output truncated: {} bytes total]",
            output.len()
        )
    } else {
        output.to_string()
    }
}

/// Short single-line description of one tool call for the per-iteration log:
/// the tool name plus a short hint of its most identifying argument.
///
/// Before this, the per-iteration log showed only tool NAMES (`c.tool`), with
/// no indication of what actually ran. In job e2915f05 — a trivial
/// one-utility-class CSS fix that ran the full 40 iterations and made ZERO
/// edits — the log showed "run_command" 17 times with no way to tell what any
/// of those 17 commands actually were, so the flailing could not be
/// diagnosed. This fixes that: `run_command(npm run -s lint)`,
/// `edit_file(app/globals.css)`, etc.
///
/// Deliberately shows only the command/path/pattern — NEVER full file or
/// command-output CONTENT, which would flood the log and could echo secrets.
fn tool_call_hint(call: &crate::agent::tools::ToolCall) -> String {
    let arg = |name: &str| call.args.get(name).map(String::as_str);
    let hint = match call.tool.as_str() {
        "run_command" => arg("command").map(|c| {
            // Flatten to one line first, THEN truncate, so a multi-line
            // command doesn't break the single-line log format.
            let flat = c.replace('\n', " ");
            crate::agent::context::smart_truncate(&flat, 60)
        }),
        "read_file" | "read_lines" | "write_file" | "edit_file" | "create_directory" => {
            arg("path").map(str::to_string)
        }
        "search_code" | "find_files" => arg("pattern").map(str::to_string),
        _ => None,
    };
    match hint {
        Some(h) => format!("{}({h})", call.tool),
        None => call.tool.clone(),
    }
}

/// Format a batch of executed tool results into the feedback message the model
/// reads next turn — identical shape to the interactive agent runner.
fn format_results(results: &[ToolResult]) -> String {
    let mut out = format!("{}\n\n", crate::conversation::TOOL_RESULTS_PREFIX);
    let mut all_success = true;
    for (idx, result) in results.iter().enumerate() {
        if !result.success {
            all_success = false;
        }
        out.push_str(&format!(
            "### Tool {}: {} [{}]\n```\n{}\n```\n\n",
            idx + 1,
            result.tool,
            if result.success { "OK" } else { "ERROR" },
            truncate_output(&result.output),
        ));
    }
    if !all_success {
        out.push_str(
            "Some tools failed. Please review the errors above and adjust your approach.\n",
        );
    }
    out
}

/// Run the agent loop to completion for `task` with full tool access — the
/// normal single-agent path used by the worker.
pub async fn run_headless_agent(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    task: &str,
    max_iterations: usize,
    command_timeout_secs: u64,
) -> anyhow::Result<HeadlessOutcome> {
    run_role(
        provider,
        model,
        AGENT_SYSTEM,
        ToolPolicy::Full,
        task,
        max_iterations,
        command_timeout_secs,
    )
    .await
}

/// Whether a tool result proves the TOOL LAYER worked, regardless of what
/// the underlying command reported. A `run_command` that ran and exited
/// non-zero (a failing build/test) is a normal, healthy result -- only a tool
/// that never ran at all is evidence of a broken tool layer.
/// `execute_run_command` stamps the "[completed in " marker only on the path
/// where the command actually executed (not on the blocked/timeout/spawn-error
/// paths), so that marker is what distinguishes "the shell ran and reported
/// the truth" from "the tool layer itself never ran".
fn tool_layer_worked(result: &ToolResult) -> bool {
    result.success || (result.tool == "run_command" && result.output.contains("[completed in "))
}

/// Core agent loop, parameterized by the role's `system_prompt` and its
/// `policy`. Under [`ToolPolicy::ReadOnly`] any write/command tool is blocked
/// (returned as a failed result) so planner/reviewer roles can inspect but not
/// mutate — mirroring the interactive pipeline's tool-layer enforcement.
///
/// The loop: ask the model → parse `<tool_call>`s → if none, we're done →
/// otherwise execute each (mutating tools run on the blocking pool) → feed the
/// results back → repeat, up to `max_iterations`.
async fn run_role(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    system_prompt: &str,
    policy: ToolPolicy,
    task: &str,
    max_iterations: usize,
    command_timeout_secs: u64,
) -> anyhow::Result<HeadlessOutcome> {
    // THE WEDGE FIX. The tool-execution counter is process-global and monotonic,
    // and nothing on this path ever reset it (only the TUI's `/agent off` did) —
    // so a long-running worker accumulated tool calls across jobs until it
    // crossed MAX_TOOL_EXECUTIONS and then failed EVERY tool call, read_file
    // included. That is exactly the "wedge" that RESTART_AFTER_JOBS, the
    // all_tools_failed self-heal, the requeue, and subprocess isolation were all
    // built to work around (a fresh process happened to get a fresh counter).
    // Each agent run gets its own budget.
    crate::agent::tools::reset_tool_counter();

    // Fold the project's persisted `.nerve/` knowledge (brief, conventions,
    // design principles, recent decisions) into the tools system message so a
    // headless job follows the project's conventions and design system instead
    // of starting amnesiac — the same "don't forget what this project is" the
    // interactive loop gets. Riding in an existing HEAD-kept system message
    // keeps it verbatim through compaction without changing HEAD_KEEP.
    let tools_prompt = match project_memory_context(task) {
        Some(ctx) => format!("{}\n\n{ctx}", tools_system_prompt()),
        None => tools_system_prompt(),
    };
    let mut messages = vec![
        ChatMessage::system(system_prompt),
        ChatMessage::system(tools_prompt),
        ChatMessage::user(task),
    ];

    let mut iterations = 0usize;
    let mut edited = false;
    let mut any_tool_succeeded = false;
    let mut nudges = 0usize;
    // Iteration at which the explore-nudge last fired (0 = never fired), and
    // how many times it has fired — replaces a one-shot `bool` latch so the
    // nudge can RE-FIRE (with escalating wording) while the agent keeps
    // exploring without editing. See `EXPLORE_NUDGE_AFTER` for why: a
    // one-shot nudge let job e2915f05 explore 32 unpressured iterations after
    // its single nudge and finish having made zero edits.
    let mut last_explore_nudge = 0usize;
    let mut explore_nudge_count = 0usize;
    // Fingerprint → content hash of reads already sent verbatim in THIS run, so
    // an identical re-read can be collapsed to a pointer instead of re-sending
    // the file. Dropped whenever compaction stubs old output away.
    let mut read_cache: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut deduped_reads = 0usize;
    // Per-PATH access counter across EVERY read-shaped tool in `PATH_TRACKED_READS`
    // — not per-tool, not per-argument. See `stuck_on_file_nudge`: this is what
    // catches job 5029e587, where `read_cache` above (keyed on tool+args) never
    // deduped `read_file(a.rs)` against `read_lines(a.rs, 10, 30)` against
    // `read_lines(a.rs, 40, 60)` because each is a distinct fingerprint.
    let mut file_access_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    // Iteration at which the stuck-on-file nudge last fired (0 = never), spaced
    // out the same way as `last_explore_nudge` so it prods once per interval
    // rather than nagging every iteration once tripped.
    let mut last_stuck_file_nudge = 0usize;
    // Assigned on every loop iteration before any break, so it is always set by
    // the time we read it after the loop.
    let mut final_response;
    let mut hit_max_iterations = false;

    loop {
        // Keep the history within budget before each model call, so a long job
        // stays efficient and never overflows the context window.
        // If compaction just stubbed earlier tool output away, a cached read is
        // no longer visible to the model — forget it, so asking again returns the
        // real content rather than a pointer to something that is gone.
        if compact_context(&mut messages, CONTEXT_BUDGET_TOKENS) {
            read_cache.clear();
        }

        let response = provider.chat(&messages, model).await?;
        messages.push(ChatMessage::assistant(&response));
        final_response = response.clone();

        let tool_calls = parse_tool_calls(&response);
        if tool_calls.is_empty() {
            // A full-tool agent that replied with prose but changed nothing has
            // usually just *described* the work (models sometimes plan first).
            // Nudge it to actually act before accepting it as done — this is the
            // difference between a job that builds the feature and one that
            // "finishes in 0 iterations" having done nothing. Read-only roles
            // (planner/reviewer) legitimately finish with prose, so never nudge
            // them.
            if policy == ToolPolicy::Full && !edited && nudges < MAX_NUDGES {
                nudges += 1;
                tracing::info!(
                    "headless: replied without acting — nudging ({nudges}/{MAX_NUDGES})"
                );
                // A model that quits citing a "tool execution limit" was, for a
                // long time, telling the TRUTH: the global tool counter was never
                // reset on this path, so it really did hit one (see the wedge fix
                // in `reset_tool_counter` above). Now each run gets its own
                // generous budget, so a bail this early is genuinely premature —
                // say so WITHOUT claiming the limit doesn't exist (it does; the
                // agent should never be told its own observations are false).
                let bailed = {
                    let r = response.to_lowercase();
                    (r.contains("limit") || r.contains("cannot") || r.contains("can't"))
                        && (r.contains("session")
                            || r.contains("tool")
                            || r.contains("continue")
                            || r.contains("progress"))
                };
                let msg = if bailed {
                    "You have plenty of tool budget left for this task — this run has barely \
                     started, so that is not a reason to stop. You have made no changes yet. Stop \
                     explaining and IMPLEMENT the task NOW with write_file/edit_file/run_command. \
                     Only stop once the change is actually written."
                } else {
                    "You have not used any tools or changed anything yet. If this task needs code \
                     changes, implement them NOW with write_file/edit_file/run_command — do not \
                     just describe them. If it genuinely needs no changes, say so briefly and stop."
                };
                messages.push(ChatMessage::user(msg));
                continue;
            }
            break; // no more tools requested → the agent is finished
        }

        iterations += 1;
        // Enriched with a short arg hint (see `tool_call_hint`) — a bare tool
        // NAME told us nothing when job e2915f05 ran 17 run_commands with no
        // record of what any of them were.
        let tools_summary: Vec<String> = tool_calls.iter().map(tool_call_hint).collect();
        tracing::info!(
            "headless iter {iterations}/{max_iterations}: {} tool(s) [{}] (ctx ~{} msgs)",
            tool_calls.len(),
            tools_summary.join(", "),
            messages.len(),
        );

        let mut results = Vec::with_capacity(tool_calls.len());
        for call in &tool_calls {
            let is_write = WRITE_TOOLS.contains(&call.tool.as_str());
            let result = if is_write && policy == ToolPolicy::ReadOnly {
                ToolResult {
                    tool: call.tool.clone(),
                    success: false,
                    output: format!(
                        "Blocked: `{}` is not permitted for a read-only role. Use read_file, \
                         read_lines, search_code, list_files or find_files and report findings.",
                        call.tool
                    ),
                }
            } else {
                // Capture what the dedupe below needs BEFORE the clone is moved
                // into the blocking task.
                let fingerprint = call_fingerprint(call);
                let tool_name = call.tool.clone();
                let path_arg = call.args.get("path").cloned();
                let call = call.clone();
                let mut result =
                    tokio::task::spawn_blocking(move || execute_tool(&call, command_timeout_secs))
                        .await
                        .unwrap_or_else(|e| ToolResult {
                            tool: "<panicked>".into(),
                            success: false,
                            output: format!("tool task panicked: {e}"),
                        });
                // Count the run as having edited ONLY when a mutating tool
                // actually SUCCEEDED. Flagging on mere invocation was a real bug:
                // a job whose every write_file failed still set `edited`, so the
                // verify gate ran against an unchanged tree, "passed", and the
                // job was logged/journaled as a success that wrote nothing (and
                // the worker skips the commit when nothing truly changed anyway).
                if result.success && is_write {
                    edited = true;
                }

                // TOKEN EFFICIENCY: models re-read the same file repeatedly, and
                // every repeat re-sent the whole thing (up to MAX_TOOL_OUTPUT_CHARS)
                // — and since each iteration re-sends the accumulated history, one
                // wasteful re-read keeps costing on every later turn. If this exact
                // read returns byte-identical content to one already in this
                // conversation, replace it with a one-line pointer. Safe because the
                // real content is still verbatim above; the moment compaction stubs
                // that away, the cache is dropped (see the loop head) so a re-read
                // returns the file in full again.
                if result.success && CACHEABLE_READS.contains(&tool_name.as_str()) {
                    let hash = content_hash(&result.output);
                    if read_cache.get(&fingerprint) == Some(&hash) {
                        deduped_reads += 1;
                        result.output = format!(
                            "[identical to your earlier {tool_name} of this exact path in this run \
                             — the content is unchanged and already shown above; omitted to save \
                             context]"
                        );
                    } else {
                        read_cache.insert(fingerprint, hash);
                    }
                }

                // Count this access against its PATH (not tool+args) so
                // `read_file(a.rs)`, `read_lines(a.rs, 10, 30)` and
                // `read_lines(a.rs, 40, 60)` all count toward the SAME file —
                // see `PATH_TRACKED_READS` for why this is a separate, broader
                // set than `CACHEABLE_READS` above. `run_command` reads (e.g. a
                // `sed`/`cat` one-liner) are a known, accepted gap — see the
                // comment on `PATH_TRACKED_READS`.
                if result.success
                    && PATH_TRACKED_READS.contains(&tool_name.as_str())
                    && let Some(path) = &path_arg
                {
                    *file_access_counts.entry(path.clone()).or_insert(0) += 1;
                }
                // Separately, track whether the TOOL LAYER worked at all -- this is
                // deliberately NOT the same condition as `edited` above. A
                // `run_command` that ran and merely exited non-zero (a failing
                // build or test suite -- the normal opening move of "run the
                // tests, see them fail, fix them") is proof the tool layer is
                // fine, even though `result.success` is false. Counting it as
                // evidence of a wedge made the detector false-fire on ordinary
                // failing tests and restart a perfectly healthy worker.
                if tool_layer_worked(&result) {
                    any_tool_succeeded = true;
                }
                result
            };
            results.push(result);
        }

        messages.push(ChatMessage::user(format_results(&results)));

        // Token-efficiency AND anti-flailing: if a full-tool agent has spent
        // many iterations exploring and still hasn't made a single change, nudge
        // it to proceed — and if it KEEPS exploring, nudge again, with
        // increasingly directive wording. This trims the long read-only prefix
        // that dominates token cost (each iteration re-sends the growing
        // context). It never FORCES action on the first couple of nudges: the
        // model can keep reading if it truly needs to. Positive framing only —
        // NEVER mention any "limit" or an iteration count (naming a limit makes
        // the model confabulate a cap and quit — see the anti-confabulation
        // nudge above, which learned that the hard way).
        //
        // This escalation exists BECAUSE a one-shot nudge (the old `bool`
        // latch) was proven too weak: job e2915f05, a trivial one-utility-class
        // CSS fix, got exactly one gentle nudge at iteration 8, explored the
        // remaining 32 iterations completely unpressured, and finished the
        // full 40-iteration run having made ZERO edits (run_command 17,
        // search_code 9, read_lines 7, read_file 6, edit_file 0, write_file 0).
        if policy == ToolPolicy::Full
            && !edited
            && iterations >= EXPLORE_NUDGE_AFTER
            && iterations - last_explore_nudge >= EXPLORE_NUDGE_INTERVAL
        {
            last_explore_nudge = iterations;
            explore_nudge_count += 1;
            tracing::info!(
                "headless: {iterations} iterations without an edit — nudging to implement \
                 (nudge #{explore_nudge_count})"
            );
            let msg = match explore_nudge_count {
                1 => {
                    "You have now gathered plenty of context and have made no changes yet. You \
                     almost certainly have enough to proceed — start IMPLEMENTING now with \
                     write_file/edit_file. Read more only if a specific edit truly requires it."
                }
                2 => {
                    "You are still exploring and have written nothing. Make your FIRST edit now \
                     with write_file or edit_file. A small, correct change you can refine beats \
                     continued reading."
                }
                _ => {
                    "Stop reading. Based on what you already know, write the single most \
                     important file for this task NOW with write_file. If unsure of the exact \
                     final form, write your best version — a partial correct edit is far better \
                     than none."
                }
            };
            messages.push(ChatMessage::user(msg));
        }

        // Stuck-on-one-file detector (job 5029e587): deliberately INDEPENDENT of
        // `edited` — unlike the explore-nudge above, this must keep firing AFTER
        // the first edit, because that is exactly when job 5029e587 got stuck:
        // it edited one file at iteration 14, then spent its ENTIRE remaining
        // budget re-reading a SECOND file four different ways (read_file,
        // read_lines twice, a `sed` run_command) and never opened the third
        // file it was asked to change, shipping a half-wired fix. Spaced out
        // like the explore-nudge (`STUCK_ON_FILE_NUDGE_INTERVAL`) so it prods
        // rather than nags once tripped.
        if let Some((path, msg)) = stuck_on_file_nudge(&file_access_counts)
            && iterations - last_stuck_file_nudge >= STUCK_ON_FILE_NUDGE_INTERVAL
        {
            last_stuck_file_nudge = iterations;
            tracing::info!("headless: stuck re-reading {path} — nudging to act or move on");
            messages.push(ChatMessage::user(msg));
        }

        if iterations >= max_iterations {
            hit_max_iterations = true;
            break;
        }
    }

    if deduped_reads > 0 {
        tracing::info!(
            "headless: collapsed {deduped_reads} redundant re-read(s) to a pointer — the model \
             re-read files it already had"
        );
    }

    Ok(HeadlessOutcome {
        iterations,
        edited,
        final_response,
        hit_max_iterations,
        // Several tool rounds ran but the tool LAYER never once proved itself
        // working -- the signature of a wedged process (every tool, even
        // read_file, failing to run at all). A `run_command` whose command
        // merely exited non-zero (a failing build/test, a completely normal
        // opening move) still counts as the layer working -- see
        // `tool_layer_worked` -- so this must not false-fire on ordinary
        // failing tests and gratuitously restart a healthy worker.
        all_tools_failed: iterations >= 3 && !any_tool_succeeded,
    })
}

// ── Multi-agent workflow (planner → coder → reviewer) ────────────────────────

const PLANNER_SYSTEM: &str = "You are the PLANNER in a plan → code → review pipeline. Explore the \
codebase with READ-ONLY file tools (read_file, read_lines, search_code, list_files, find_files) \
and produce a concise, numbered implementation plan grounded in the ACTUAL code and its \
conventions: which files to add or change, and how. The write_file/edit_file/run_command tools are \
DISABLED for you and will be rejected — planning only, do not attempt them. Be efficient: read only \
what you need to plan well. End your reply with the final plan as a numbered list.";

const REVIEWER_SYSTEM: &str = "You are the REVIEWER in a plan → code → review pipeline. You are given \
the DIFF of what was just implemented; judge whether it correctly and cleanly implements the task \
and matches the project's conventions. You may use READ-ONLY file tools (read_file, read_lines, \
search_code, list_files, find_files) for extra context. The run_command tool is DISABLED for you \
and will be rejected — do not attempt it. Call out concrete problems if there are any. End your \
reply with EXACTLY one final line: `VERDICT: APPROVED` or `VERDICT: NEEDS FIXES: <short reason>`.";

/// Outcome of a multi-agent workflow run.
#[derive(Debug, Clone)]
pub struct WorkflowOutcome {
    /// Whether the coder edited files.
    pub edited: bool,
    /// Total coder (+ fix) iterations.
    pub coder_iterations: usize,
    /// The planner's numbered plan.
    pub plan: String,
    /// The reviewer's verdict text.
    pub review: String,
    /// Whether any coding phase hit the iteration cap.
    pub hit_max_iterations: bool,
}

/// Run a **planner → coder → reviewer** workflow for `task`, the headless
/// equivalent of the interactive `/workflow` pipeline. The planner (read-only)
/// grounds a plan in the real code; the coder (full tools) implements it; the
/// reviewer (read-only) judges the result and, if it flags fixes, the coder
/// gets one corrective round. Returns the combined outcome.
pub async fn run_workflow(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    task: &str,
    max_iterations: usize,
    command_timeout_secs: u64,
) -> anyhow::Result<WorkflowOutcome> {
    // 1. Plan — read-only.
    let planner = run_role(
        provider,
        model,
        PLANNER_SYSTEM,
        ToolPolicy::ReadOnly,
        task,
        max_iterations,
        command_timeout_secs,
    )
    .await?;
    let plan = planner.final_response.clone();
    tracing::info!(
        "workflow: plan ready after {} planning iteration(s)",
        planner.iterations
    );

    // 2. Code — full tools, guided by the plan.
    let coder_task =
        format!("{task}\n\n## Approved implementation plan\n{plan}\n\nImplement this plan now.");
    let mut coder = run_role(
        provider,
        model,
        AGENT_SYSTEM,
        ToolPolicy::Full,
        &coder_task,
        max_iterations,
        command_timeout_secs,
    )
    .await?;

    // 3. Review — read-only, given the actual diff of the coder's work so it
    // reviews what changed rather than having to reconstruct it.
    let review_task = match working_diff() {
        Some(diff) => format!(
            "The task was:\n{task}\n\nThe plan was:\n{plan}\n\nHere is the diff of what was just \
             implemented:\n\n```diff\n{diff}\n```\n\nReview it for correctness, quality and \
             conventions; read files for extra context if needed."
        ),
        None => format!(
            "The task was:\n{task}\n\nThe plan was:\n{plan}\n\nReview the code that now exists in \
             the repository for correctness, quality and conventions."
        ),
    };
    let reviewer = run_role(
        provider,
        model,
        REVIEWER_SYSTEM,
        ToolPolicy::ReadOnly,
        &review_task,
        max_iterations,
        command_timeout_secs,
    )
    .await?;
    let review = reviewer.final_response.clone();
    tracing::info!("workflow: review done — {}", first_review_line(&review));

    // 4. One corrective round — but ONLY if the reviewer actually reached a
    // conclusion. A reviewer that ran out of iterations produces an unreliable
    // "NEEDS FIXES" (it never finished inspecting); treating that as a real
    // verdict just burns another coding round for nothing.
    if !reviewer.hit_max_iterations && review.to_uppercase().contains("NEEDS FIXES") {
        tracing::info!("workflow: reviewer requested fixes; running one coder correction round");
        let fix_task = format!(
            "A reviewer flagged issues with your implementation:\n\n{review}\n\nFix them. The \
             original task was: {task}"
        );
        let fixo = run_role(
            provider,
            model,
            AGENT_SYSTEM,
            ToolPolicy::Full,
            &fix_task,
            max_iterations,
            command_timeout_secs,
        )
        .await?;
        coder.edited = coder.edited || fixo.edited;
        coder.iterations += fixo.iterations;
        coder.hit_max_iterations = coder.hit_max_iterations || fixo.hit_max_iterations;
    }

    Ok(WorkflowOutcome {
        edited: coder.edited,
        coder_iterations: coder.iterations,
        plan,
        review,
        hit_max_iterations: coder.hit_max_iterations,
    })
}

// ── Self-decomposing agent (plan → split → execute each step) ────────────────

/// Hard cap on how many sub-tasks a decomposition may produce, so a runaway
/// planner can't spawn an unbounded number of agent runs.
const MAX_SUBTASKS: usize = 12;

const DECOMPOSER_SYSTEM: &str = "You are the PLANNER in a decompose → execute pipeline. Your job is to \
break ONE coding task into the SMALLEST sequence of self-contained sub-tasks that another agent will \
implement one at a time. This matters because a single agent reliably completes small, focused, \
prescriptive changes but THRASHES on a big cross-cutting change done all at once. \
Explore first with READ-ONLY tools (read_file, read_lines, search_code, list_files, find_files) — \
write_file/edit_file/run_command are DISABLED for you and will be rejected. \
Rules for a GOOD decomposition: (1) Each sub-task should touch AT MOST ~2 files and be independently \
implementable and verifiable. (2) Order them so earlier steps don't depend on later ones — put \
NEW-FILE and PURE-LOGIC steps first, then the WIRING steps that call them. (3) CHECK whether the \
types/plumbing a step needs ALREADY EXIST (they often do) — if so, that step collapses to a single \
file; say so. (4) Make each instruction PRESCRIPTIVE: name the exact file(s), the exact function \
signature or JSX to add, and a concrete acceptance criterion. (5) Prefer 2-5 steps; never exceed \
about 8. Do NOT write any code yourself. \
End your reply with ONLY a fenced JSON array (```json ... ```), each element an object with two \
string fields: \"title\" (a few words) and \"instruction\" (the full prescriptive sub-task, written \
as if handed directly to the implementing agent). Output nothing after the closing fence.";

/// Extract a JSON array from planner text: prefer a ```json fenced block, else
/// the widest `[ … ]` slice. Char-boundary safe (uses byte indices from `find`,
/// which land on valid boundaries for the ASCII delimiters we search for).
fn extract_json_array(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after = &text[start + "```json".len()..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    let lb = text.find('[')?;
    let rb = text.rfind(']')?;
    (rb > lb).then(|| text[lb..=rb].to_string())
}

/// Parse the decomposer's output into ordered `(title, instruction)` sub-tasks.
/// Returns empty on any parse failure so the caller can fall back safely.
fn parse_subtasks(text: &str) -> Vec<(String, String)> {
    let Some(json) = extract_json_array(text) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Vec::new();
    };
    let Some(arr) = val.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            let instruction = item
                .get("instruction")
                .and_then(|v| v.as_str())?
                .trim()
                .to_string();
            (!instruction.is_empty()).then_some((title, instruction))
        })
        .take(MAX_SUBTASKS)
        .collect()
}

/// Run a task by DECOMPOSING it into small sub-tasks and executing each in turn.
///
/// This is the systematic form of the "decompose cross-cutting work" rule that
/// makes edit-existing tasks succeed: a read-only planner splits the task into
/// the smallest ordered, prescriptive steps, then each step runs through the
/// normal full-tool agent against the accumulating repo state. If the planner
/// produces no usable steps, it falls back to a single ordinary agent run — so
/// decomposition can never do WORSE than running the task whole.
pub async fn run_decomposed_agent(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    task: &str,
    max_iterations: usize,
    command_timeout_secs: u64,
) -> anyhow::Result<HeadlessOutcome> {
    let planner = run_role(
        provider,
        model,
        DECOMPOSER_SYSTEM,
        ToolPolicy::ReadOnly,
        task,
        max_iterations,
        command_timeout_secs,
    )
    .await?;
    let mut subtasks = parse_subtasks(&planner.final_response);

    // Seen live: the planner explores fine, then answers with prose (or a plan
    // in the wrong shape) and no parseable JSON — and we'd silently fall back to
    // running the whole task in one agent, which is exactly what decomposition
    // exists to avoid. Before giving up, ask ONCE for just the JSON, handing the
    // model its own analysis back so it only has to reformat. A tiny iteration
    // budget keeps it from exploring again.
    if subtasks.is_empty() {
        tracing::info!("decompose: planner returned no parseable plan — asking once for the JSON");
        let retry_task = format!(
            "The task was:\n{task}\n\nYour analysis so far:\n{}\n\nOutput ONLY the decomposition \
             now, as a fenced JSON array (```json ... ```) where each element is an object with \
             exactly two string fields: \"title\" and \"instruction\". No prose, no tool calls — \
             just the JSON array.",
            crate::agent::context::smart_truncate(&planner.final_response, 4000)
        );
        if let Ok(retry) = run_role(
            provider,
            model,
            DECOMPOSER_SYSTEM,
            ToolPolicy::ReadOnly,
            &retry_task,
            2,
            command_timeout_secs,
        )
        .await
        {
            subtasks = parse_subtasks(&retry.final_response);
        }
    }

    if subtasks.is_empty() {
        tracing::warn!("decompose: no sub-tasks parsed — falling back to a single agent run");
        return run_headless_agent(provider, model, task, max_iterations, command_timeout_secs)
            .await;
    }
    let n = subtasks.len();
    tracing::info!("decompose: {n} sub-task(s) planned");

    let mut edited = false;
    let mut total_iters = planner.iterations;
    let mut hit_cap = false;
    let mut wedged = false;
    let mut summary = String::new();

    for (i, (title, instruction)) in subtasks.iter().enumerate() {
        let step_task = format!(
            "This is step {step}/{n} of a larger goal — implement ONLY this step.\n\n\
             OVERALL GOAL: {task}\n\nSTEP {step}: {title}\n\n{instruction}",
            step = i + 1,
        );
        tracing::info!("decompose: step {}/{n}: {title}", i + 1);
        // Run each step in a FRESH process so the wedge can't accumulate across
        // the job's many steps. `NotSpawned` means the child never ran at all,
        // so it is safe to fall back to running the step in-process. `Failed`
        // means the child STARTED and may already have half-applied edits to
        // the repo — re-running the SAME step in-process on top of that would
        // run it TWICE (duplicate imports/functions, or the agent thrashing
        // against its own unfinished work), so that case must NOT retry; it is
        // handled below as a genuine step failure instead.
        let out =
            match exec_step_subprocess(&step_task, model, max_iterations, command_timeout_secs)
                .await
            {
                StepExec::Ran(out) => out,
                StepExec::NotSpawned => {
                    run_role(
                        provider,
                        model,
                        AGENT_SYSTEM,
                        ToolPolicy::Full,
                        &step_task,
                        max_iterations,
                        command_timeout_secs,
                    )
                    .await?
                }
                StepExec::Failed => {
                    // The child may have already modified the repo — do NOT re-run
                    // this step (that would be the double-run hazard this fix
                    // exists to avoid). Record it as failed and stop: later steps
                    // likely depend on this one having actually completed. Leave
                    // `edited` as-is; the worker commits whatever is actually in
                    // the tree.
                    summary.push_str(&format!("- Step {}/{n} [FAILED]: {title}\n", i + 1));
                    break;
                }
            };
        total_iters += out.iterations;
        edited = edited || out.edited;
        hit_cap = hit_cap || out.hit_max_iterations;

        // Commit each finished step to the job branch so progress is DURABLE: a
        // decompose job runs many sub-agents in one process and can hit the
        // worker wedge mid-way — committing per step means a wedge-triggered
        // requeue resumes with the completed steps intact instead of starting
        // over (the requeue path keeps the branch's commits). Best-effort.
        if out.edited {
            commit_step(i + 1, n, title);
        }

        // A wedged environment (every tool failing) won't recover within this
        // process — stop and let the worker self-heal via a restart.
        if out.all_tools_failed {
            wedged = true;
            summary.push_str(&format!("- Step {}/{n} [WEDGED]: {title}\n", i + 1));
            break;
        }
        let status = if out.hit_max_iterations {
            "INCOMPLETE (hit cap)"
        } else if out.edited {
            "done"
        } else {
            "no changes"
        };
        summary.push_str(&format!("- Step {}/{n} [{status}]: {title}\n", i + 1));
    }

    Ok(HeadlessOutcome {
        iterations: total_iters,
        edited,
        final_response: format!("Decomposed the task into {n} step(s):\n{summary}"),
        hit_max_iterations: hit_cap,
        all_tools_failed: wedged,
    })
}

/// Commit the current working-tree changes to the job branch as one decompose
/// step, so progress survives a mid-run wedge + requeue. Best-effort: any git
/// failure is ignored (the worker still commits any remaining dirt at the end).
/// Assumes CWD is the repo (the worker sets it before the run).
///
/// Stages explicit paths rather than `git add -A` so generated build/cache
/// output (`__pycache__`, `target/`, `node_modules/`, etc. — see
/// `worker::is_generated_artifact`) never reaches the commit, even though a
/// step may legitimately have written such files as a side effect (e.g.
/// running a Python test suite). Skipped paths are logged so a reviewer
/// never wonders why a file a step touched is absent from the commit.
fn commit_step(step: usize, total: usize, title: &str) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(["-c", "safe.directory=*"])
            .args(args)
            .output()
    };
    let status = run(&["status", "--porcelain"])
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for line in status.lines() {
        let Some(rest) = line.get(3..).map(str::trim) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        // For a rename/copy the porcelain is "old -> new"; keep the new path.
        let path = rest.rsplit(" -> ").next().unwrap_or(rest).trim_matches('"');
        if crate::worker::is_generated_artifact(path) {
            skipped.push(path.to_string());
        } else {
            kept.push(path.to_string());
        }
    }
    if !skipped.is_empty() {
        tracing::info!(
            "decompose step {step}/{total} ({title}): excluded {} generated artifact \
             path(s) from the commit: {}",
            skipped.len(),
            skipped.join(", ")
        );
    }
    if kept.is_empty() {
        // Nothing real to commit — e.g. the step's only side effect was
        // writing to a cache dir. Leave the tree as-is; a step with no real
        // commit here simply contributes no commit to `commits_ahead_of_base`.
        return;
    }
    let mut add = vec!["add", "--"];
    add.extend(kept.iter().map(String::as_str));
    let _ = run(&add);
    let msg = format!("decompose step {step}/{total}: {title}");
    // --no-verify so a repo's pre-commit hook can't block the autonomous run.
    let _ = run(&["commit", "--no-verify", "-m", &msg]);
}

/// The uncommitted diff in the current directory (the coder's changes, which
/// the worker commits *after* the workflow), truncated for the reviewer's
/// context. Best-effort: `None` if git isn't available or there's no diff.
/// Assumes CWD is the repo (the worker sets it before running the workflow).
fn working_diff() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-c", "safe.directory=*", "diff", "HEAD"])
        .output()
        .ok()?;
    let diff = String::from_utf8_lossy(&output.stdout);
    let diff = diff.trim();
    if diff.is_empty() {
        None
    } else {
        Some(crate::agent::context::smart_truncate(diff, 12_000))
    }
}

/// The reviewer's `VERDICT:` line (or a short prefix) for logging.
fn first_review_line(review: &str) -> String {
    review
        .lines()
        .rev()
        .find(|l| l.to_uppercase().contains("VERDICT"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| crate::agent::context::smart_truncate(review.trim(), 80))
}

#[cfg(test)]
#[path = "headless_tests.rs"]
mod tests;
