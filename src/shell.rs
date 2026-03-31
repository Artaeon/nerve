use std::process::Command;
use anyhow::Context;

/// Maximum number of output lines before truncation.
const MAX_OUTPUT_LINES: usize = 5000;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Truncate text to at most `max_lines` lines, appending a notice if trimmed.
fn truncate_output(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let mut truncated: String = lines[..max_lines].join("\n");
    truncated.push_str(&format!(
        "\n\n... ({} lines truncated, {} total)",
        lines.len() - max_lines,
        lines.len()
    ));
    truncated
}

/// Run a shell command and capture its output.
pub fn run_command(cmd: &str) -> anyhow::Result<CommandResult> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .with_context(|| format!("Failed to execute: {cmd}"))?;

    let stdout = truncate_output(
        &String::from_utf8_lossy(&output.stdout),
        MAX_OUTPUT_LINES,
    );
    let stderr = truncate_output(
        &String::from_utf8_lossy(&output.stderr),
        MAX_OUTPUT_LINES,
    );

    Ok(CommandResult {
        command: cmd.to_string(),
        stdout,
        stderr,
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
    })
}

/// Format command output for display in conversation.
pub fn format_command_output(result: &CommandResult) -> String {
    let mut output = format!("$ {}\n", result.command);

    if !result.stdout.is_empty() {
        output.push_str(&result.stdout);
        if !result.stdout.ends_with('\n') {
            output.push('\n');
        }
    }

    if !result.stderr.is_empty() {
        output.push_str(&format!("\nstderr:\n{}", result.stderr));
    }

    if !result.success {
        output.push_str(&format!("\nExit code: {}", result.exit_code));
    }

    output
}

/// Format command output as context for AI (in a code block).
pub fn format_command_for_context(result: &CommandResult) -> String {
    let status = if result.success {
        "success".to_string()
    } else {
        format!("failed (exit {})", result.exit_code)
    };
    let mut content = format!("Command: `{}` ({})\n\n```\n", result.command, status);

    if !result.stdout.is_empty() {
        content.push_str(&result.stdout);
    }
    if !result.stderr.is_empty() {
        if !result.stdout.is_empty() {
            content.push('\n');
        }
        content.push_str("--- stderr ---\n");
        content.push_str(&result.stderr);
    }

    content.push_str("\n```");
    content
}

/// Detect the project's test command based on what files exist.
pub fn detect_test_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo test";
    }
    if cwd.join("package.json").exists() {
        return "npm test";
    }
    if cwd.join("go.mod").exists() {
        return "go test ./...";
    }
    if cwd.join("pyproject.toml").exists() {
        return "pytest";
    }
    if cwd.join("Gemfile").exists() {
        return "bundle exec rspec";
    }
    if cwd.join("mix.exs").exists() {
        return "mix test";
    }
    if cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
        return "gradle test";
    }
    if cwd.join("pom.xml").exists() {
        return "mvn test";
    }
    if cwd.join("Makefile").exists() {
        return "make test";
    }

    "echo 'No test command detected'"
}

/// Detect the project's build command.
pub fn detect_build_command() -> &'static str {
    let cwd = std::env::current_dir().unwrap_or_default();

    if cwd.join("Cargo.toml").exists() {
        return "cargo build";
    }
    if cwd.join("package.json").exists() {
        return "npm run build";
    }
    if cwd.join("go.mod").exists() {
        return "go build ./...";
    }
    if cwd.join("Makefile").exists() {
        return "make";
    }
    if cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
        return "gradle build";
    }
    if cwd.join("pom.xml").exists() {
        return "mvn package";
    }
    if cwd.join("CMakeLists.txt").exists() {
        return "cmake --build build";
    }

    "echo 'No build command detected'"
}

/// Get the git diff.
pub fn git_diff(args: &str) -> anyhow::Result<CommandResult> {
    let cmd = if args.is_empty() {
        "git diff".to_string()
    } else {
        format!("git diff {args}")
    };
    run_command(&cmd)
}

/// Get git status.
#[allow(dead_code)]
pub fn git_status() -> anyhow::Result<CommandResult> {
    run_command("git status --short")
}

/// Get git log.
#[allow(dead_code)]
pub fn git_log(count: usize) -> anyhow::Result<CommandResult> {
    run_command(&format!("git log --oneline -{count}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo() {
        let result = run_command("echo hello").unwrap();
        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn run_failing_command() {
        let result = run_command("false").unwrap();
        assert!(!result.success);
    }

    #[test]
    fn format_output_includes_command() {
        let result = run_command("echo test").unwrap();
        let formatted = format_command_output(&result);
        assert!(formatted.contains("$ echo test"));
        assert!(formatted.contains("test"));
    }

    #[test]
    fn format_context_wraps_in_code_block() {
        let result = run_command("echo test").unwrap();
        let formatted = format_command_for_context(&result);
        assert!(formatted.contains("```"));
        assert!(formatted.contains("success"));
    }

    #[test]
    fn detect_test_command_for_rust() {
        // We're in a Rust project
        let cmd = detect_test_command();
        assert_eq!(cmd, "cargo test");
    }

    #[test]
    fn detect_build_command_for_rust() {
        let cmd = detect_build_command();
        assert_eq!(cmd, "cargo build");
    }

    #[test]
    fn truncate_long_output() {
        // Build a string with more than MAX_OUTPUT_LINES lines
        let long_text: String = (0..MAX_OUTPUT_LINES + 100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let truncated = truncate_output(&long_text, MAX_OUTPUT_LINES);
        assert!(truncated.contains("truncated"));
        assert!(truncated.lines().count() <= MAX_OUTPUT_LINES + 3); // +3 for the notice
    }

    #[test]
    fn short_output_not_truncated() {
        let short = "line 1\nline 2\nline 3";
        let result = truncate_output(short, MAX_OUTPUT_LINES);
        assert_eq!(result, short);
    }

    #[test]
    fn run_command_captures_stderr() {
        let result = run_command("echo error >&2").unwrap();
        assert!(result.stderr.contains("error"));
    }

    #[test]
    fn run_command_with_pipe() {
        let result = run_command("echo 'hello world' | tr 'h' 'H'").unwrap();
        assert!(result.stdout.contains("Hello"));
    }

    #[test]
    fn run_command_exit_code() {
        let result = run_command("exit 42").unwrap();
        assert_eq!(result.exit_code, 42);
        assert!(!result.success);
    }

    #[test]
    fn format_command_output_shows_stderr() {
        let result = CommandResult {
            command: "test".into(),
            stdout: "out".into(),
            stderr: "err".into(),
            exit_code: 1,
            success: false,
        };
        let output = format_command_output(&result);
        assert!(output.contains("stderr:"));
        assert!(output.contains("err"));
        assert!(output.contains("Exit code: 1"));
    }

    #[test]
    fn format_context_shows_status() {
        let result = CommandResult {
            command: "test".into(),
            stdout: "output".into(),
            stderr: String::new(),
            exit_code: 0,
            success: true,
        };
        let ctx = format_command_for_context(&result);
        assert!(ctx.contains("success"));
        assert!(ctx.contains("```"));
    }
}
