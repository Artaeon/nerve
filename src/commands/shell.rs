//! Shell commands: /run, /pipe, /diff, /test, /build, /git

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

    if trimmed == "/diff" || trimmed.starts_with("/diff ") {
        return handle_diff(app, trimmed);
    }

    if trimmed == "/test" {
        return handle_test(app);
    }

    if trimmed == "/build" {
        return handle_build(app);
    }

    if trimmed == "/git" || trimmed.starts_with("/git ") {
        return handle_git(app, trimmed);
    }

    if trimmed == "/commit" || trimmed.starts_with("/commit ") {
        return handle_commit(app, trimmed);
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

fn handle_commit(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/commit").unwrap_or("").trim();

    // First, stage all changes if nothing is staged.
    let staged_check = shell::run_command("git diff --cached --quiet");
    let nothing_staged = staged_check.as_ref().map(|r| r.success).unwrap_or(false);

    if nothing_staged {
        if let Ok(result) = shell::run_command("git add -A") {
            if !result.success {
                app.set_status(format!("git add failed: {}", result.stderr));
                return true;
            }
        }
        // Check again if there's anything to commit.
        let recheck = shell::run_command("git diff --cached --quiet");
        if recheck.as_ref().map(|r| r.success).unwrap_or(false) {
            app.set_status("Nothing to commit (working tree clean)");
            return true;
        }
    }

    let message = if rest.is_empty() {
        "Changes from Nerve".to_string()
    } else {
        rest.to_string()
    };

    let escaped_msg = shell::shell_escape(&message);
    let author_flag = build_git_author_flag(&app.git_user_name, &app.git_user_email);
    let cmd = if author_flag.is_empty() {
        format!("git commit -m {escaped_msg}")
    } else {
        format!("git commit {author_flag} -m {escaped_msg}")
    };

    match shell::run_command(&cmd) {
        Ok(result) => {
            if result.success {
                app.add_assistant_message(format!(
                    "Committed: {}\n\n```\n{}\n```",
                    message,
                    result.stdout.trim()
                ));
                app.set_status("Commit successful");
            } else {
                app.set_status(format!("Commit failed: {}", result.stderr));
            }
        }
        Err(e) => app.report_error(e),
    }
    app.scroll_offset = 0;
    true
}

/// Build a `--author='Name <email>'` flag for git commit.
///
/// Returns an empty string if either name or email is empty.
/// Both fields are required — git requires `Name <email>` format.
pub(crate) fn build_git_author_flag(name: &str, email: &str) -> String {
    if name.is_empty() || email.is_empty() {
        return String::new();
    }
    // Use shell_escape for the combined author string.
    let author = format!("{} <{}>", name, email);
    format!("--author={}", shell::shell_escape(&author))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_author_flag_both_set() {
        let flag = build_git_author_flag("Jane Doe", "jane@example.com");
        assert_eq!(flag, "--author='Jane Doe <jane@example.com>'");
    }

    #[test]
    fn git_author_flag_escapes_quotes() {
        let flag = build_git_author_flag("O'Brien", "ob@test.com");
        assert_eq!(flag, "--author='O'\\''Brien <ob@test.com>'");
    }

    #[test]
    fn git_author_flag_empty_name() {
        let flag = build_git_author_flag("", "jane@example.com");
        assert!(flag.is_empty());
    }

    #[test]
    fn git_author_flag_empty_email() {
        let flag = build_git_author_flag("Jane Doe", "");
        assert!(flag.is_empty());
    }

    #[test]
    fn git_author_flag_both_empty() {
        let flag = build_git_author_flag("", "");
        assert!(flag.is_empty());
    }
}
