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
IMPORTANT: there is NO limit on the number of tool calls or 'tool executions' you may make in this \
session — no such per-session cap exists. Never stop, and never claim you have 'hit a tool execution \
limit' or 'cannot continue this session': keep going until the task is actually complete. The ONLY \
valid reasons to stop are (a) the task is genuinely done, or (b) you are truly blocked by missing \
information or a broken environment that no further tool call could resolve — and in case (b) you \
must first have actually attempted the change. When the task is complete, reply with a short \
plain-text summary of what you changed and STOP (emit no further tool calls).";

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

/// Bound the running message history so a long job stays token-efficient.
///
/// Strategy: keep the system prompts + task (head) and the most recent
/// exchanges (tail) verbatim; replace the *content* of older tool-RESULT
/// messages (the big file/command dumps — role `"user"`) with a short stub once
/// the total is over `budget_tokens`. The model keeps its own reasoning (every
/// `assistant` turn is untouched) and can cheaply re-read a file if it truly
/// needs it again — so we shed the expensive raw bytes without losing the plot.
/// Idempotent: re-stubbing an already-stubbed message is a no-op.
fn compact_context(messages: &mut [ChatMessage], budget_tokens: usize) {
    if messages.len() <= HEAD_KEEP + TAIL_KEEP {
        return;
    }
    let total: usize = messages
        .iter()
        .map(|m| crate::agent::context::ContextManager::estimate_tokens(&m.content))
        .sum();
    if total <= budget_tokens {
        return;
    }

    const STUB: &str =
        "[earlier tool output compacted to save context — re-read the file if you need it again]";
    let cut_end = messages.len() - TAIL_KEEP;
    for msg in &mut messages[HEAD_KEEP..cut_end] {
        // Only shrink big tool-result messages (role "user"); leave the model's
        // own assistant turns intact so its reasoning trail is preserved.
        if msg.role == "user" && msg.content.len() > STUB.len() + 100 {
            msg.content = STUB.to_string();
        }
    }
}

/// The result of a headless agent run.
#[derive(Debug, Clone)]
pub struct HeadlessOutcome {
    /// Number of tool-executing iterations that ran.
    pub iterations: usize,
    /// Whether any write/mutating tool was invoked.
    pub edited: bool,
    /// The model's final plain-text response (its summary).
    pub final_response: String,
    /// Whether the run stopped because it hit the iteration cap (vs. finished).
    pub hit_max_iterations: bool,
    /// True when the run executed several tool rounds but *not a single* tool
    /// call succeeded — the signature of a wedged worker process (every tool,
    /// even `read_file`, failing). A healthy run always has at least one
    /// successful read. The worker uses this to self-heal via a fresh restart.
    pub all_tools_failed: bool,
}

/// Char-safe truncation of tool output, matching the interactive runner's cap.
fn truncate_output(output: &str) -> String {
    if output.len() > 5000 {
        let head: String = output.chars().take(5000).collect();
        format!(
            "{head}...\n[Output truncated: {} bytes total]",
            output.len()
        )
    } else {
        output.to_string()
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
    let mut messages = vec![
        ChatMessage::system(system_prompt),
        ChatMessage::system(tools_system_prompt()),
        ChatMessage::user(task),
    ];

    let mut iterations = 0usize;
    let mut edited = false;
    let mut any_tool_succeeded = false;
    let mut nudges = 0usize;
    // Assigned on every loop iteration before any break, so it is always set by
    // the time we read it after the loop.
    let mut final_response;
    let mut hit_max_iterations = false;

    loop {
        // Keep the history within budget before each model call, so a long job
        // stays efficient and never overflows the context window.
        compact_context(&mut messages, CONTEXT_BUDGET_TOKENS);

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
                // Models sometimes give up mid-loop by confabulating a "tool
                // execution limit" / "cannot continue this session". There is no
                // such limit — rebut it directly so the agent resumes instead of
                // abandoning a task it never actually started.
                let bailed = {
                    let r = response.to_lowercase();
                    (r.contains("limit") || r.contains("cannot") || r.contains("can't"))
                        && (r.contains("session")
                            || r.contains("tool")
                            || r.contains("continue")
                            || r.contains("progress"))
                };
                let msg = if bailed {
                    "There is NO tool-execution or per-session limit — that constraint does not \
                     exist. You have made no changes yet. Stop explaining and IMPLEMENT the task \
                     NOW with write_file/edit_file/run_command. Only stop once the change is \
                     actually written."
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
        let tools_summary: Vec<&str> = tool_calls.iter().map(|c| c.tool.as_str()).collect();
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
                let call = call.clone();
                let result =
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
                if result.success {
                    any_tool_succeeded = true;
                    if is_write {
                        edited = true;
                    }
                }
                result
            };
            results.push(result);
        }

        messages.push(ChatMessage::user(format_results(&results)));

        if iterations >= max_iterations {
            hit_max_iterations = true;
            break;
        }
    }

    Ok(HeadlessOutcome {
        iterations,
        edited,
        final_response,
        hit_max_iterations,
        // Several tool rounds ran but nothing ever succeeded → the environment
        // is wedged (a healthy run always lands at least one successful read).
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
