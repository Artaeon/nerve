//! Shell commands: /run, /pipe, /diff, /test, /build, /lint, /format, /search, /git

use crate::app::App;
use crate::shell;

/// Handle shell-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str) -> bool {
    if trimmed.starts_with("/run ") || trimmed.starts_with("/! ") {
        return handle_run(app, trimmed);
    }

    if trimmed.starts_with("/pipe ") {
        return handle_pipe(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/diff") {
        return handle_diff(app, trimmed);
    }

    if trimmed == "/test" {
        return handle_test(app);
    }

    if trimmed == "/build" {
        return handle_build(app);
    }

    if trimmed == "/lint" {
        return handle_lint(app);
    }

    if trimmed == "/format" || trimmed == "/fmt" {
        return handle_format(app);
    }

    if crate::shell::matches_command(trimmed, "/search") {
        return handle_search(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/git") {
        return handle_git(app, trimmed);
    }

    false
}

fn is_dangerous_command(cmd: &str) -> bool {
    shell::is_dangerous_command(cmd)
}

fn handle_run(app: &mut App, trimmed: &str) -> bool {
    let rest = if let Some(r) = trimmed.strip_prefix("/run ") {
        r.trim()
    } else {
        trimmed.strip_prefix("/! ").unwrap_or("").trim()
    };
    if rest.is_empty() {
        app.add_assistant_message(
            "Usage: /run <command>\nExecutes a shell command and shows the output.".into(),
        );
        return true;
    }
    let cmd = rest.to_string();
    if is_dangerous_command(&cmd) {
        app.set_status("Blocked: this command looks dangerous. Use your terminal directly.");
        return true;
    }
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(&cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            app.add_assistant_message(output);
        }
        Err(e) => {
            app.report_error(e);
        }
    }
    true
}

fn handle_pipe(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/pipe ").unwrap_or("").trim();
    if rest.is_empty() {
        app.add_assistant_message(
            "Usage: /pipe <command>\nRuns a command and adds its output as context.".into(),
        );
        return true;
    }
    let cmd = rest.to_string();
    if is_dangerous_command(&cmd) {
        app.set_status("Blocked: this command looks dangerous. Use your terminal directly.");
        return true;
    }
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(&cmd, app.command_timeout_secs) {
        Ok(result) => {
            let context = shell::format_command_for_context(&result);
            app.current_conversation_mut()
                .messages
                .push(("system".into(), context));
            app.set_status(format!(
                "Added output of '{}' as context ({} lines)",
                cmd,
                result.stdout.lines().count()
            ));
        }
        Err(e) => {
            app.report_error(e);
        }
    }
    true
}

fn handle_diff(app: &mut App, trimmed: &str) -> bool {
    let diff_args = trimmed
        .strip_prefix("/diff")
        .unwrap_or("")
        .trim()
        .to_string();
    match shell::git_diff(&diff_args) {
        Ok(result) => {
            if result.stdout.trim().is_empty() {
                app.add_assistant_message("No changes detected (git diff is empty).".into());
            } else {
                let label = if diff_args.is_empty() {
                    String::new()
                } else {
                    format!(" {diff_args}")
                };
                let context = format!("Git diff{}:\n\n```diff\n{}\n```", label, result.stdout);
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context));
                app.add_assistant_message(format!(
                    "Diff loaded ({} lines). Ask me anything about it.",
                    result.stdout.lines().count()
                ));
            }
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_test(app: &mut App) -> bool {
    let cmd = shell::detect_test_command();
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            let context = shell::format_command_for_context(&result);
            app.current_conversation_mut()
                .messages
                .push(("system".into(), context));
            app.add_assistant_message(output);
            if !result.success {
                app.set_status("Tests FAILED \u{2014} ask me to help fix them");
            } else {
                app.set_status("Tests passed");
            }
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_build(app: &mut App) -> bool {
    let cmd = shell::detect_build_command();
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            if !result.success {
                let context = shell::format_command_for_context(&result);
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context));
                app.add_assistant_message(output);
                app.set_status("Build FAILED \u{2014} ask me to help fix it");
            } else {
                app.add_assistant_message(output);
                app.set_status("Build succeeded");
            }
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_git(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/git").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();
    let subcmd = args.first().copied().unwrap_or("status");
    let cmd = match subcmd {
        "status" | "s" => "git status --short".to_string(),
        "log" | "l" => {
            let n = args
                .get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
            format!("git log --oneline -{n}")
        }
        "diff" | "d" => "git diff".to_string(),
        "branch" | "b" => "git branch -a".to_string(),
        _ => format!("git {rest}"),
    };
    if is_dangerous_command(&cmd) {
        app.set_status("Blocked: this command looks dangerous. Use your terminal directly.");
        return true;
    }
    match shell::run_command_with_timeout(&cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            app.add_assistant_message(output);
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_lint(app: &mut App) -> bool {
    let cmd = shell::detect_lint_command();
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            let context = shell::format_command_for_context(&result);
            app.current_conversation_mut()
                .messages
                .push(("system".into(), context));
            app.add_assistant_message(output);
            if !result.success {
                app.set_status("Lint FAILED \u{2014} ask me to help fix issues");
            } else {
                app.set_status("Lint passed");
            }
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_format(app: &mut App) -> bool {
    let cmd = shell::detect_format_command();
    app.set_status(format!("Running: {cmd}"));
    match shell::run_command_with_timeout(cmd, app.command_timeout_secs) {
        Ok(result) => {
            let output = shell::format_command_output(&result);
            app.add_assistant_message(output);
            if !result.success {
                app.set_status("Format failed");
            } else {
                app.set_status("Code formatted");
            }
        }
        Err(e) => app.report_error(e),
    }
    true
}

fn handle_search(app: &mut App, trimmed: &str) -> bool {
    let pattern = trimmed.strip_prefix("/search").unwrap_or("").trim();
    if pattern.is_empty() {
        app.set_status("Usage: /search <pattern>");
        return true;
    }

    let escaped = shell::shell_escape(pattern);
    let cmd = format!(
        "rg --line-number --color=never --max-count=5 --max-columns=200 {escaped} | head -50"
    );
    match shell::run_command_with_timeout(&cmd, app.command_timeout_secs) {
        Ok(result) => {
            if result.stdout.trim().is_empty() {
                app.add_assistant_message(format!("No matches found for: `{pattern}`"));
            } else {
                let match_count = result.stdout.lines().count();
                let context = format!(
                    "Search results for `{pattern}`:\n\n```\n{}\n```",
                    result.stdout
                );
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context));
                app.add_assistant_message(format!(
                    "Found {match_count} matches for `{pattern}`. Ask me to analyze or modify the matching code."
                ));
            }
            app.set_status(format!("Search: {pattern}"));
        }
        Err(e) => app.report_error(e),
    }
    true
}

#[cfg(test)]
mod tests {
    // Tests for shell commands are minimal since most logic is in shell.rs.
}
