use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    ]
}

/// Generate the tools description for the system prompt
pub fn tools_system_prompt() -> String {
    let mut prompt = String::from(
        "You have access to the following tools. To use a tool, respond with a tool call in this EXACT format:\n\n\
         <tool_call>\n\
         tool: tool_name\n\
         arg_name: arg_value\n\
         arg_name2: arg_value2\n\
         </tool_call>\n\n\
         You can use multiple tools in a single response. After each tool call, you will receive the result.\n\
         Continue using tools until the task is complete, then provide your final response.\n\n\
         Available tools:\n\n",
    );

    for tool in available_tools() {
        prompt.push_str(&format!(
            "- **{}**: {}\n  Parameters: {}\n\n",
            tool.name, tool.description, tool.parameters
        ));
    }

    prompt.push_str(
        "Guidelines:\n\
         - Read files before editing them to understand the context\n\
         - After writing/editing files, verify with read_file if needed\n\
         - Run tests after making code changes\n\
         - Keep changes minimal and focused\n\
         - Explain your reasoning before and after tool use\n",
    );

    prompt
}

/// Parse tool calls from AI response text
pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<tool_call>") {
        if let Some(end) = remaining[start..].find("</tool_call>") {
            let block = &remaining[start + 11..start + end];
            if let Some(call) = parse_single_tool_call(block) {
                calls.push(call);
            }
            remaining = &remaining[start + end + 12..];
        } else {
            break;
        }
    }

    calls
}

fn parse_single_tool_call(block: &str) -> Option<ToolCall> {
    let mut tool_name = None;
    let mut args = std::collections::HashMap::new();
    let lines: Vec<&str> = block.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() {
            i += 1;
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            if key == "tool" {
                tool_name = Some(value.to_string());
                i += 1;
            } else if is_multiline_arg(key) {
                // Collect all lines until the next known key or end
                let mut full_value = value.to_string();
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    // Check if this is a new argument
                    if next_line
                        .split_once(':')
                        .map(|(k, _)| is_known_arg(k.trim()))
                        .unwrap_or(false)
                    {
                        break;
                    }
                    full_value.push('\n');
                    full_value.push_str(lines[i]); // Keep original indentation
                    i += 1;
                }
                args.insert(key.to_string(), full_value);
            } else {
                args.insert(key.to_string(), value.to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    tool_name.map(|name| ToolCall { tool: name, args })
}

fn is_multiline_arg(key: &str) -> bool {
    matches!(key, "content" | "old_text" | "new_text")
}

fn is_known_arg(key: &str) -> bool {
    matches!(
        key,
        "tool" | "path" | "content" | "old_text" | "new_text" | "command" | "pattern"
    )
}

/// Reset the per-session tool execution counter.
pub fn reset_tool_counter() {
    TOOL_EXEC_COUNT.store(0, Ordering::Relaxed);
}

/// Execute a tool call and return the result, enforcing a per-session
/// execution limit.
pub fn execute_tool(call: &ToolCall) -> ToolResult {
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
        "read_file" => execute_read_file(call),
        "write_file" => execute_write_file(call),
        "edit_file" => execute_edit_file(call),
        "run_command" => execute_run_command(call),
        "list_files" => execute_list_files(call),
        "search_code" => execute_search_code(call),
        "create_directory" => execute_create_dir(call),
        _ => ToolResult {
            tool: call.tool.clone(),
            success: false,
            output: format!("Unknown tool: {}", call.tool),
        },
    }
}

fn execute_read_file(call: &ToolCall) -> ToolResult {
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");

    // Security: block reading sensitive files
    if crate::shell::is_sensitive_file(path) {
        return ToolResult {
            tool: "read_file".into(),
            success: false,
            output: "Blocked: this file may contain secrets".into(),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Truncate very large files
            let truncated = if content.len() > 50_000 {
                format!(
                    "{}...\n\n[Truncated: file is {} bytes]",
                    &content[..50_000],
                    content.len()
                )
            } else {
                content
            };
            ToolResult {
                tool: "read_file".into(),
                success: true,
                output: truncated,
            }
        }
        Err(e) => ToolResult {
            tool: "read_file".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

fn execute_write_file(call: &ToolCall) -> ToolResult {
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");
    let content = call.args.get("content").map(|s| s.as_str()).unwrap_or("");

    // Security: block writing to protected system paths
    if crate::shell::is_protected_path(path) {
        return ToolResult {
            tool: "write_file".into(),
            success: false,
            output: format!("Blocked: cannot write to protected path: {path}"),
        };
    }

    // Create parent directories
    if let Some(parent) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(path, content) {
        Ok(()) => ToolResult {
            tool: "write_file".into(),
            success: true,
            output: format!("Written {} bytes to {path}", content.len()),
        },
        Err(e) => ToolResult {
            tool: "write_file".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

fn execute_edit_file(call: &ToolCall) -> ToolResult {
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");
    let old_text = call.args.get("old_text").map(|s| s.as_str()).unwrap_or("");
    let new_text = call.args.get("new_text").map(|s| s.as_str()).unwrap_or("");

    // Security: block editing protected system paths
    if crate::shell::is_protected_path(path) {
        return ToolResult {
            tool: "edit_file".into(),
            success: false,
            output: format!("Blocked: cannot edit protected path: {path}"),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            if !content.contains(old_text) {
                return ToolResult {
                    tool: "edit_file".into(),
                    success: false,
                    output: "old_text not found in file".into(),
                };
            }
            let new_content = content.replacen(old_text, new_text, 1);
            match std::fs::write(path, &new_content) {
                Ok(()) => ToolResult {
                    tool: "edit_file".into(),
                    success: true,
                    output: format!("Edited {path}"),
                },
                Err(e) => ToolResult {
                    tool: "edit_file".into(),
                    success: false,
                    output: format!("Write error: {e}"),
                },
            }
        }
        Err(e) => ToolResult {
            tool: "edit_file".into(),
            success: false,
            output: format!("Read error: {e}"),
        },
    }
}

fn execute_run_command(call: &ToolCall) -> ToolResult {
    let cmd = call.args.get("command").map(|s| s.as_str()).unwrap_or("");

    // Security: block dangerous commands from agent
    if crate::shell::is_dangerous_command(cmd) {
        return ToolResult {
            tool: "run_command".into(),
            success: false,
            output: "Blocked: this command is potentially destructive".into(),
        };
    }

    match crate::shell::run_command(cmd) {
        Ok(result) => ToolResult {
            tool: "run_command".into(),
            success: result.success,
            output: format!(
                "{}{}",
                result.stdout,
                if result.stderr.is_empty() {
                    String::new()
                } else {
                    format!("\nstderr: {}", result.stderr)
                }
            ),
        },
        Err(e) => ToolResult {
            tool: "run_command".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

fn execute_list_files(call: &ToolCall) -> ToolResult {
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or(".");
    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect();
            items.sort();
            ToolResult {
                tool: "list_files".into(),
                success: true,
                output: items.join("\n"),
            }
        }
        Err(e) => ToolResult {
            tool: "list_files".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

fn execute_search_code(call: &ToolCall) -> ToolResult {
    let pattern = call.args.get("pattern").map(|s| s.as_str()).unwrap_or("");
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or(".");

    let cmd = format!(
        "grep -rn --include='*.rs' --include='*.py' --include='*.js' --include='*.ts' \
         --include='*.go' --include='*.java' --include='*.toml' --include='*.json' \
         --include='*.yaml' --include='*.md' '{}' {} | head -50",
        pattern.replace('\'', "'\\''"),
        path
    );
    match crate::shell::run_command(&cmd) {
        Ok(result) => ToolResult {
            tool: "search_code".into(),
            success: true,
            output: if result.stdout.is_empty() {
                "No matches found".into()
            } else {
                result.stdout
            },
        },
        Err(e) => ToolResult {
            tool: "search_code".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

fn execute_create_dir(call: &ToolCall) -> ToolResult {
    let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");

    // Security: block creating directories in protected system paths
    if crate::shell::is_protected_path(path) {
        return ToolResult {
            tool: "create_directory".into(),
            success: false,
            output: format!("Blocked: cannot create directory in protected path: {path}"),
        };
    }

    match std::fs::create_dir_all(path) {
        Ok(()) => ToolResult {
            tool: "create_directory".into(),
            success: true,
            output: format!("Created {path}"),
        },
        Err(e) => ToolResult {
            tool: "create_directory".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_tool_call_test() {
        let text = "Let me read that file.\n\n<tool_call>\ntool: read_file\npath: src/main.rs\n</tool_call>\n\nDone.";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "read_file");
        assert_eq!(calls[0].args.get("path").unwrap(), "src/main.rs");
    }

    #[test]
    fn parse_multiple_tool_calls() {
        let text = "<tool_call>\ntool: read_file\npath: a.rs\n</tool_call>\n\n<tool_call>\ntool: read_file\npath: b.rs\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn parse_no_tool_calls() {
        let text = "Just a regular response with no tools.";
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_tool_call_with_multiword_args() {
        let text =
            "<tool_call>\ntool: run_command\ncommand: cargo test --release\n</tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(
            calls[0].args.get("command").unwrap(),
            "cargo test --release"
        );
    }

    #[test]
    fn available_tools_not_empty() {
        assert!(available_tools().len() >= 7);
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
    fn execute_list_files_current_dir() {
        let call = ToolCall {
            tool: "list_files".into(),
            args: [("path".into(), ".".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);
        assert!(result.output.contains("Cargo.toml"));
    }

    #[test]
    fn execute_unknown_tool() {
        let call = ToolCall {
            tool: "nonexistent".into(),
            args: Default::default(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Unknown tool"));
    }

    #[test]
    fn execute_read_file_not_found() {
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/nonexistent/file.txt".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
    }

    #[test]
    fn execute_create_and_write_file() {
        let dir = std::env::temp_dir().join("nerve_agent_test");
        let file_path = dir.join("test.txt");
        let _ = std::fs::remove_dir_all(&dir);

        // Create directory
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), dir.to_string_lossy().into())].into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);

        // Write file
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), file_path.to_string_lossy().into()),
                ("content".into(), "hello world".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);

        // Read file
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), file_path.to_string_lossy().into())].into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);
        assert_eq!(result.output, "hello world");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_search_code_finds_pattern() {
        let call = ToolCall {
            tool: "search_code".into(),
            args: [
                ("pattern".into(), "fn main".into()),
                ("path".into(), "src/".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);
        assert!(result.output.contains("main"));
    }

    #[test]
    fn parse_tool_call_with_multiline_content() {
        let text = r#"<tool_call>
tool: write_file
path: src/hello.rs
content: fn main() {
    println!("hello");
}
</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "write_file");
        assert!(calls[0].args.get("content").unwrap().contains("println"));
    }

    #[test]
    fn parse_edit_file_with_multiline() {
        let text = r#"<tool_call>
tool: edit_file
path: src/main.rs
old_text: fn old() {
    old_code();
}
new_text: fn new() {
    new_code();
}
</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].args.get("old_text").unwrap().contains("old_code"));
        assert!(calls[0].args.get("new_text").unwrap().contains("new_code"));
    }

    #[test]
    fn execute_edit_file_not_found() {
        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "/nonexistent.rs".into()),
                ("old_text".into(), "foo".into()),
                ("new_text".into(), "bar".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
    }

    #[test]
    fn execute_edit_file_text_not_found() {
        let dir = std::env::temp_dir().join("nerve_edit_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello world").unwrap();

        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), path.to_string_lossy().into()),
                ("old_text".into(), "nonexistent".into()),
                ("new_text".into(), "replacement".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("not found"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_edit_file_success() {
        let dir = std::env::temp_dir().join("nerve_edit_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello world").unwrap();

        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), path.to_string_lossy().into()),
                ("old_text".into(), "hello".into()),
                ("new_text".into(), "goodbye".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "goodbye world");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Security tests ──────────────────────────────────────────────────

    #[test]
    fn agent_blocks_dangerous_commands() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "run_command".into(),
            args: [("command".into(), "rm -rf /".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    #[test]
    fn agent_allows_safe_commands() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "run_command".into(),
            args: [("command".into(), "echo hello".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(result.success);
    }

    #[test]
    fn agent_blocks_write_to_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), "/etc/passwd".into()),
                ("content".into(), "bad".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
        assert!(result.output.contains("protected"));
    }

    #[test]
    fn agent_blocks_edit_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "/usr/bin/something".into()),
                ("old_text".into(), "a".into()),
                ("new_text".into(), "b".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    #[test]
    fn agent_blocks_create_dir_in_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), "/etc/nerve_test".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    #[test]
    fn agent_blocks_reading_sensitive_files() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/home/user/.ssh/id_rsa".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
        assert!(result.output.contains("secrets"));
    }

    #[test]
    fn agent_blocks_reading_env_file() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), ".env".into())].into(),
        };
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
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
        let result = execute_tool(&call);
        assert!(!result.success);
        assert!(result.output.contains("limit"));
        reset_tool_counter(); // Clean up for other tests
    }
}
