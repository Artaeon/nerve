//! Git write commands: /commit, /stage, /unstage, /gitbranch, /stash, /log,
//! and enhanced /gitstatus.

use std::sync::Arc;

use crate::ai::provider::AiProvider;
use crate::app::App;
use crate::shell;

/// Handle git-related write commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str, _provider: &Arc<dyn AiProvider>) -> bool {
    if trimmed == "/log" || trimmed.starts_with("/log ") {
        return handle_log(app, trimmed);
    }

    if trimmed == "/gitstatus" {
        return handle_gitstatus(app);
    }

    false
}

// ── /gitstatus ─────────────────────────────────────────────────────────────

fn handle_gitstatus(app: &mut App) -> bool {
    match shell::run_command("git status") {
        Ok(result) => {
            if result.success {
                app.add_assistant_message(format!(
                    "Git Status\n{}\n\n{}",
                    "=".repeat(25),
                    result.stdout
                ));
            } else {
                app.set_status(format!("git status failed: {}", result.stderr));
            }
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.scroll_offset = 0;
    true
}

// ── /log ───────────────────────────────────────────────────────────────────

fn handle_log(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/log").unwrap_or("").trim();
    let count = rest.parse::<usize>().unwrap_or(10);
    let count = count.clamp(1, 100);

    let cmd = format!("git log --oneline --decorate --graph -{}", count);

    match shell::run_command(&cmd) {
        Ok(result) => {
            if result.success {
                if result.stdout.trim().is_empty() {
                    app.add_assistant_message("No commits found.".into());
                } else {
                    app.add_assistant_message(format!(
                        "Git Log (last {})\n{}\n\n```\n{}\n```",
                        count,
                        "=".repeat(25),
                        result.stdout.trim()
                    ));
                }
            } else {
                app.set_status(format!("git log failed: {}", result.stderr));
            }
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.scroll_offset = 0;
    true
}

// ── Argument parsing helpers (used by tests) ───────────────────────────────

/// Parse `/log ...` input into the count.
pub(crate) fn parse_log_count(input: &str) -> usize {
    let rest = input.strip_prefix("/log").unwrap_or("").trim();
    rest.parse::<usize>().unwrap_or(10).clamp(1, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Log parsing ────────────────────────────────────────────────────

    #[test]
    fn log_count_parsing() {
        assert_eq!(parse_log_count("/log"), 10);
        assert_eq!(parse_log_count("/log 5"), 5);
        assert_eq!(parse_log_count("/log 20"), 20);
        assert_eq!(parse_log_count("/log 0"), 1); // clamped to min 1
        assert_eq!(parse_log_count("/log 200"), 100); // clamped to max 100
        assert_eq!(parse_log_count("/log abc"), 10); // fallback to default
    }

    #[test]
    fn log_with_whitespace() {
        assert_eq!(parse_log_count("/log   15  "), 15);
    }
}
