//! Headless agent runner — drives the agent loop to completion with no TUI.
//!
//! This is what the server's queue worker uses to actually execute a job. It
//! reuses the SAME primitives as the interactive loop — `tools_system_prompt`,
//! `parse_tool_calls`, `execute_tool`, and the `TOOL_RESULTS_PREFIX` feedback
//! format — so an autonomous run behaves identically to a run you'd drive by
//! hand in the TUI. No `App`, no terminal, no event loop.

use std::sync::Arc;

use crate::agent::tools::{ToolResult, execute_tool, parse_tool_calls, tools_system_prompt};
use crate::ai::provider::{AiProvider, ChatMessage};

/// System prompt that frames the model as an autonomous worker. The tool
/// protocol itself comes from `tools_system_prompt()`, appended after this.
const AGENT_SYSTEM: &str = "You are Nerve, running autonomously as a background worker on a coding task. \
Work step by step using the tools until the task is fully done. Verify your work where you can \
(build/tests). When — and only when — the task is complete, reply with a short plain-text summary \
of what you changed and STOP (emit no further tool calls). If you cannot make progress, explain why \
and stop.";

/// Tools that mutate the workspace — used to flag whether a run edited files.
const WRITE_TOOLS: &[&str] = &[
    "write_file",
    "edit_file",
    "run_command",
    "create_directory",
    "remember",
    "update_tasks",
];

/// Default safety cap on agent iterations for an unattended run.
pub const DEFAULT_MAX_ITERATIONS: usize = 25;

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

/// Run the agent loop to completion for `task`, returning what happened.
///
/// The loop: ask the model → parse `<tool_call>`s → if none, we're done →
/// otherwise execute each (mutating tools run on the blocking pool) → feed the
/// results back → repeat, up to `max_iterations`.
pub async fn run_headless_agent(
    provider: &Arc<dyn AiProvider>,
    model: &str,
    task: &str,
    max_iterations: usize,
    command_timeout_secs: u64,
) -> anyhow::Result<HeadlessOutcome> {
    let mut messages = vec![
        ChatMessage::system(AGENT_SYSTEM),
        ChatMessage::system(tools_system_prompt()),
        ChatMessage::user(task),
    ];

    let mut iterations = 0usize;
    let mut edited = false;
    // Assigned on every loop iteration before any break, so it is always set by
    // the time we read it after the loop.
    let mut final_response;
    let mut hit_max_iterations = false;

    loop {
        let response = provider.chat(&messages, model).await?;
        messages.push(ChatMessage::assistant(&response));
        final_response = response.clone();

        let tool_calls = parse_tool_calls(&response);
        if tool_calls.is_empty() {
            break; // no more tools requested → the agent is finished
        }

        iterations += 1;

        let mut results = Vec::with_capacity(tool_calls.len());
        for call in &tool_calls {
            if WRITE_TOOLS.contains(&call.tool.as_str()) {
                edited = true;
            }
            let call = call.clone();
            let result =
                tokio::task::spawn_blocking(move || execute_tool(&call, command_timeout_secs))
                    .await
                    .unwrap_or_else(|e| ToolResult {
                        tool: "<panicked>".into(),
                        success: false,
                        output: format!("tool task panicked: {e}"),
                    });
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
    })
}

#[cfg(test)]
#[path = "headless_tests.rs"]
mod tests;
