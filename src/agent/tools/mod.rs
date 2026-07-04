use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

mod exec;
mod fs;
pub mod parse;

pub use parse::parse_tool_calls;

static TOOL_EXEC_COUNT: AtomicUsize = AtomicUsize::new(0);
const MAX_TOOL_EXECUTIONS: usize = 100;

/// A tool the AI agent can invoke
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: String, // Human-readable parameter description
}

/// A parsed tool call from AI output
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool: String,
    pub args: std::collections::HashMap<String, String>,
}

/// Result of executing a tool
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub output: String,
}

/// All tools available to the agent
pub fn available_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "read_file".into(),
            description: "Read the contents of a file".into(),
            parameters: "path: string (file path to read)".into(),
        },
        Tool {
            name: "write_file".into(),
            description: "Write content to a file (creates or overwrites)".into(),
            parameters: "path: string, content: string".into(),
        },
        Tool {
            name: "edit_file".into(),
            description: "Replace a specific string in a file".into(),
            parameters: "path: string, old_text: string, new_text: string".into(),
        },
        Tool {
            name: "run_command".into(),
            description: "Execute a shell command and return its output".into(),
            parameters: "command: string".into(),
        },
        Tool {
            name: "list_files".into(),
            description: "List files in a directory (non-recursive)".into(),
            parameters: "path: string (directory path)".into(),
        },
        Tool {
            name: "search_code".into(),
            description: "Search for a pattern in files (like grep)".into(),
            parameters: "pattern: string, path: string (optional, defaults to .)".into(),
        },
        Tool {
            name: "create_directory".into(),
            description: "Create a directory (including parents)".into(),
            parameters: "path: string".into(),
        },
        Tool {
            name: "find_files".into(),
            description: "Find files matching a glob pattern (recursive)".into(),
            parameters: "pattern: string (e.g. '*.rs', 'src/**/*.ts'), path: string (optional, defaults to .)".into(),
        },
        Tool {
            name: "read_lines".into(),
            description: "Read specific line range from a file".into(),
            parameters: "path: string, start: number (1-indexed), end: number".into(),
        },
        Tool {
            name: "web_search".into(),
            description: "Search the web for current information, documentation, or answers"
                .into(),
            parameters: "query: string (search query)".into(),
        },
        Tool {
            name: "remember".into(),
            description: "Persist a project fact/convention to .nerve/memory.md so it is \
                          known in all future sessions (use for conventions, gotchas, \
                          decisions the user confirms)"
                .into(),
            parameters: "fact: string (one concise sentence)".into(),
        },
        Tool {
            name: "update_tasks".into(),
            description: "Manage the persistent project task backlog (.nerve/tasks.json): \
                          add tasks or update their status. Use to track multi-step work — \
                          the backlog survives sessions."
                .into(),
            parameters: "action: string (add|done|start|fail), title: string (for add), \
                         id: number (for done/start/fail)"
                .into(),
        },
    ]
}

/// Generate the tools description for the system prompt
pub fn tools_system_prompt() -> String {
    let mut prompt = String::from(
"You are Nerve, an AI coding assistant with direct access to the user's filesystem and terminal. You can read files, write code, edit existing files, run commands, and search the codebase.

IMPORTANT — HOW TOOLS WORK HERE: Any built-in/native tool mechanism you may have is DISABLED in this session. Do NOT try to use native tools, do NOT ask the user to grant permissions, and NEVER claim you are blocked or lack write access — you are not. The ONLY way to read or modify files or run commands is the <tool_call> text format below; Nerve executes it and returns results.

TOOL FORMAT — You MUST use this exact format to call tools:

<tool_call>
tool: tool_name
arg_name: value
</tool_call>

You may use multiple tools per response. After tool execution, you will receive results in <tool_result> tags. Continue using tools until the task is fully complete.

AVAILABLE TOOLS:

");

    for tool in available_tools() {
        prompt.push_str(&format!(
            "### {}\n{}\nParameters: {}\n\n",
            tool.name, tool.description, tool.parameters
        ));
    }

    prompt.push_str(
"WORKFLOW — Follow this process for every task:

1. UNDERSTAND: Read relevant files first. Never edit a file you haven't read.
2. PLAN: Explain what you will change and why before making changes.
3. IMPLEMENT: Make changes using write_file or edit_file.
4. VERIFY: Read the changed file to confirm correctness.
5. TEST: Run tests if applicable (e.g., `cargo test`, `npm test`).

RULES:
- Always read a file before editing it.
- Use edit_file for small changes (replacing specific text). Use write_file for new files or complete rewrites.
- For edit_file, the old_text must match EXACTLY (including whitespace and indentation).
- Run commands to verify your changes work.
- If a command fails, read the error output and fix the issue.
- Keep changes minimal — don't refactor code that isn't related to the task.
- When creating new files, include proper error handling and follow the project's existing patterns.
- Explain what you did after completing the task.

IMPORTANT: When you are done with all changes and verification, respond with your final summary WITHOUT any tool calls. This signals that the task is complete.
");

    prompt
}

/// Reset the per-session tool execution counter.
pub fn reset_tool_counter() {
    TOOL_EXEC_COUNT.store(0, Ordering::Relaxed);
}

/// Extract a required argument from a tool call, returning an error result
/// if the argument is missing or empty.
pub(super) fn require_arg<'a>(call: &'a ToolCall, name: &str) -> Result<&'a str, ToolResult> {
    match call.args.get(name).map(std::string::String::as_str) {
        Some(v) if !v.is_empty() => Ok(v),
        _ => Err(ToolResult {
            tool: call.tool.clone(),
            success: false,
            output: format!("Missing required argument: {name}"),
        }),
    }
}

/// Execute a tool call and return the result, enforcing a per-session
/// execution limit. `command_timeout_secs` is applied to the `run_command`
/// tool (0 = no timeout).
pub fn execute_tool(call: &ToolCall, command_timeout_secs: u64) -> ToolResult {
    let count = TOOL_EXEC_COUNT.fetch_add(1, Ordering::Relaxed);
    if count >= MAX_TOOL_EXECUTIONS {
        return ToolResult {
            tool: call.tool.clone(),
            success: false,
            output: format!(
                "Tool execution limit reached ({MAX_TOOL_EXECUTIONS}). Start a new session."
            ),
        };
    }

    match call.tool.as_str() {
        "read_file" => fs::execute_read_file(call),
        "write_file" => fs::execute_write_file(call),
        "edit_file" => fs::execute_edit_file(call),
        "run_command" => exec::execute_run_command(call, command_timeout_secs),
        "list_files" => fs::execute_list_files(call),
        "search_code" => exec::execute_search_code(call),
        "create_directory" => fs::execute_create_dir(call),
        "find_files" => fs::execute_find_files(call),
        "read_lines" => fs::execute_read_lines(call),
        "web_search" => exec::execute_web_search(call),
        "remember" => exec::execute_remember(call),
        "update_tasks" => exec::execute_update_tasks(call),
        _ => ToolResult {
            tool: call.tool.clone(),
            success: false,
            output: format!("Unknown tool: {}", call.tool),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_tools_not_empty() {
        assert!(available_tools().len() >= 9);
    }

    #[test]
    fn tools_system_prompt_mentions_all_tools() {
        let prompt = tools_system_prompt();
        for tool in available_tools() {
            assert!(
                prompt.contains(&tool.name),
                "Missing tool {} in system prompt",
                tool.name
            );
        }
    }

    #[test]
    fn execute_unknown_tool() {
        let call = ToolCall {
            tool: "nonexistent".into(),
            args: Default::default(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Unknown tool"));
    }

    #[test]
    fn tool_execution_limit() {
        // Set the counter just below the limit, then verify the next call
        // is blocked. This avoids issues with parallel test execution
        // sharing the global counter.
        TOOL_EXEC_COUNT.store(MAX_TOOL_EXECUTIONS, Ordering::Relaxed);
        let call = ToolCall {
            tool: "list_files".into(),
            args: [("path".into(), ".".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("limit"));
        reset_tool_counter(); // Clean up for other tests
    }

    // ── New tool and prompt tests ──────────────────────────────────────

    #[test]
    fn tools_system_prompt_is_detailed() {
        let prompt = tools_system_prompt();
        assert!(prompt.contains("WORKFLOW"));
        assert!(prompt.contains("RULES"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("find_files"));
        assert!(prompt.contains("read_lines"));
        assert!(prompt.len() > 1000); // Should be substantial
    }

    #[test]
    fn exactly_twelve_tools() {
        assert_eq!(available_tools().len(), 12);
    }
}
