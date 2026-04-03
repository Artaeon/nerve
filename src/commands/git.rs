//! Git write commands: /commit, /stage, /unstage, /gitbranch, /stash, /log,
//! and enhanced /gitstatus.

use std::sync::Arc;

use crate::ai::provider::{AiProvider, ChatMessage};
use crate::app::App;
use crate::shell;

/// Handle git-related write commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) -> bool {
    if trimmed == "/commit" || trimmed.starts_with("/commit ") {
        return handle_commit(app, trimmed, provider).await;
    }

    if trimmed == "/stage" || trimmed.starts_with("/stage ") {
        return handle_stage(app, trimmed);
    }

    if trimmed == "/unstage" || trimmed.starts_with("/unstage ") {
        return handle_unstage(app, trimmed);
    }

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

// ── /stage ─────────────────────────────────────────────────────────────────

fn handle_stage(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/stage").unwrap_or("").trim();

    if rest.is_empty() {
        // Stage all changes
        match shell::run_command("git add -A") {
            Ok(result) => {
                if result.success {
                    // Show what was staged
                    match shell::run_command("git diff --cached --stat") {
                        Ok(stat) => {
                            if stat.stdout.trim().is_empty() {
                                app.set_status("Nothing to stage");
                            } else {
                                app.add_assistant_message(format!(
                                    "Staged all changes:\n\n```\n{}\n```",
                                    stat.stdout.trim()
                                ));
                                app.set_status("All changes staged");
                            }
                        }
                        Err(_) => app.set_status("All changes staged"),
                    }
                } else {
                    app.set_status(format!("git add failed: {}", result.stderr));
                }
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        }
    } else {
        // Stage specific files
        let files: Vec<&str> = rest.split_whitespace().collect();
        let escaped: Vec<String> = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "'\\''")))
            .collect();
        let cmd = format!("git add {}", escaped.join(" "));

        match shell::run_command(&cmd) {
            Ok(result) => {
                if result.success {
                    app.set_status(format!("Staged {} file(s)", files.len()));
                } else {
                    app.set_status(format!("git add failed: {}", result.stderr));
                }
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        }
    }
    app.scroll_offset = 0;
    true
}

// ── /unstage ───────────────────────────────────────────────────────────────

fn handle_unstage(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/unstage").unwrap_or("").trim();

    let cmd = if rest.is_empty() {
        "git reset HEAD".to_string()
    } else {
        let files: Vec<&str> = rest.split_whitespace().collect();
        let escaped: Vec<String> = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "'\\''")))
            .collect();
        format!("git reset HEAD {}", escaped.join(" "))
    };

    match shell::run_command(&cmd) {
        Ok(result) => {
            if result.success {
                if rest.is_empty() {
                    app.set_status("All files unstaged");
                } else {
                    app.set_status("File(s) unstaged");
                }
            } else {
                app.set_status(format!("git reset failed: {}", result.stderr));
            }
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.scroll_offset = 0;
    true
}

// ── /commit ────────────────────────────────────────────────────────────────

async fn handle_commit(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) -> bool {
    let rest = trimmed.strip_prefix("/commit").unwrap_or("").trim();

    // First, stage all changes if nothing is staged
    let staged_check = shell::run_command("git diff --cached --quiet");
    let nothing_staged = staged_check.as_ref().map(|r| r.success).unwrap_or(false);

    if nothing_staged {
        // Nothing staged, stage everything
        if let Ok(result) = shell::run_command("git add -A") {
            if !result.success {
                app.set_status(format!("git add failed: {}", result.stderr));
                return true;
            }
        }

        // Check again if there's anything to commit
        let recheck = shell::run_command("git diff --cached --quiet");
        if recheck.as_ref().map(|r| r.success).unwrap_or(false) {
            app.set_status("Nothing to commit (working tree clean)");
            return true;
        }
    }

    let message = if rest.is_empty() {
        // Generate commit message from AI
        app.set_status("Generating commit message...");
        match generate_commit_message(app, provider).await {
            Ok(msg) => msg,
            Err(e) => {
                app.set_status(format!("AI commit message failed: {e}"));
                return true;
            }
        }
    } else {
        rest.to_string()
    };

    // Perform the commit
    let escaped_msg = message.replace('\'', "'\\''");
    let cmd = format!("git commit -m '{escaped_msg}'");

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
        Err(e) => app.set_status(format!("Error: {e}")),
    }
    app.scroll_offset = 0;
    true
}

async fn generate_commit_message(
    app: &App,
    provider: &Arc<dyn AiProvider>,
) -> anyhow::Result<String> {
    let diff = shell::run_command("git diff --cached --stat")?;
    let diff_detail = shell::run_command("git diff --cached")?;

    // Truncate large diffs to avoid exceeding context
    let diff_text = if diff_detail.stdout.len() > 4000 {
        format!("{}\n\n... (diff truncated)", &diff_detail.stdout[..4000])
    } else {
        diff_detail.stdout.clone()
    };

    let prompt = format!(
        "Generate a concise git commit message for these changes. \
         Return ONLY the commit message, nothing else. No quotes, no prefix. \
         Use conventional commit format (e.g. 'feat: ...', 'fix: ...', 'refactor: ...').\n\n\
         Files changed:\n{}\n\nDiff:\n{}",
        diff.stdout, diff_text
    );

    let messages = vec![
        ChatMessage::system(
            "You are a git commit message generator. Output only the commit message, nothing else.",
        ),
        ChatMessage::user(prompt),
    ];

    let model = &app.selected_model;
    let response = provider.chat(&messages, model).await?;

    // Clean up the response - remove quotes, trim whitespace
    let cleaned = response
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();

    if cleaned.is_empty() {
        anyhow::bail!("AI returned empty commit message");
    }

    Ok(cleaned)
}

// ── Argument parsing helpers (used by tests) ───────────────────────────────

/// Extract the commit message from a `/commit ...` input.
pub(crate) fn parse_commit_message(input: &str) -> Option<&str> {
    let rest = input.strip_prefix("/commit")?.trim();
    if rest.is_empty() { None } else { Some(rest) }
}

/// Parse `/log ...` input into the count.
pub(crate) fn parse_log_count(input: &str) -> usize {
    let rest = input.strip_prefix("/log").unwrap_or("").trim();
    rest.parse::<usize>().unwrap_or(10).clamp(1, 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Commit message parsing ──────────────────────────────────────────

    #[test]
    fn commit_message_parsing() {
        assert_eq!(parse_commit_message("/commit fix bug"), Some("fix bug"));
        assert_eq!(
            parse_commit_message("/commit feat: add new feature"),
            Some("feat: add new feature")
        );
        assert_eq!(parse_commit_message("/commit"), None);
        assert_eq!(parse_commit_message("/commit "), None);
    }

    #[test]
    fn commit_message_with_special_chars() {
        assert_eq!(
            parse_commit_message("/commit fix: handle 'quoted' strings"),
            Some("fix: handle 'quoted' strings")
        );
        assert_eq!(
            parse_commit_message("/commit refactor: move A -> B"),
            Some("refactor: move A -> B")
        );
    }

    #[test]
    fn commit_empty_after_trim() {
        assert_eq!(parse_commit_message("/commit   "), None);
    }

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
