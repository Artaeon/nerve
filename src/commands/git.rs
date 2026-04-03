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

    if trimmed == "/gitbranch" || trimmed.starts_with("/gitbranch ") {
        return handle_gitbranch(app, trimmed);
    }

    if trimmed == "/stash" || trimmed.starts_with("/stash ") {
        return handle_stash(app, trimmed);
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

// ── /gitbranch ─────────────────────────────────────────────────────────────

fn handle_gitbranch(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/gitbranch").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();

    if args.is_empty() {
        // List branches
        match shell::run_command(
            "git branch -a --format='%(if)%(HEAD)%(then)* %(else)  %(end)%(refname:short)%(if)%(upstream)%(then) -> %(upstream:short)%(end)'",
        ) {
            Ok(result) => {
                if result.success {
                    app.add_assistant_message(format!(
                        "Git Branches\n{}\n\n{}\n\nUsage:\n  /gitbranch <name>          Create and switch to new branch\n  /gitbranch switch <name>   Switch to existing branch\n  /gitbranch delete <name>   Delete a branch",
                        "=".repeat(25),
                        result.stdout.trim()
                    ));
                } else {
                    app.set_status(format!("git branch failed: {}", result.stderr));
                }
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        }
        app.scroll_offset = 0;
        return true;
    }

    match args[0] {
        "switch" | "checkout" | "co" => {
            if args.len() < 2 {
                app.set_status("Usage: /gitbranch switch <name>");
                return true;
            }
            let name = args[1];
            let cmd = format!("git switch '{}'", name.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.set_status(format!("Switched to branch: {name}"));
                    } else {
                        app.set_status(format!("Switch failed: {}", result.stderr.trim()));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        "delete" | "rm" | "del" => {
            if args.len() < 2 {
                app.set_status("Usage: /gitbranch delete <name>");
                return true;
            }
            let name = args[1];
            let cmd = format!("git branch -d '{}'", name.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.set_status(format!("Deleted branch: {name}"));
                    } else {
                        // Check if the branch needs force-delete
                        if result.stderr.contains("not fully merged") {
                            app.add_assistant_message(format!(
                                "Branch '{}' is not fully merged.\n\
                                 Use `/run git branch -D {}` to force delete.",
                                name, name
                            ));
                        } else {
                            app.set_status(format!("Delete failed: {}", result.stderr.trim()));
                        }
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        _ => {
            // Create and switch to new branch
            let name = args[0];
            if !is_valid_branch_name(name) {
                app.set_status(format!("Invalid branch name: {name}"));
                return true;
            }
            let cmd = format!("git switch -c '{}'", name.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.set_status(format!("Created and switched to branch: {name}"));
                    } else {
                        app.set_status(format!("Branch creation failed: {}", result.stderr.trim()));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
    }
    app.scroll_offset = 0;
    true
}

/// Validate a git branch name (basic checks).
fn is_valid_branch_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Git branch names cannot contain these
    let invalid_chars = [' ', '~', '^', ':', '\\', '?', '*', '['];
    if name.chars().any(|c| invalid_chars.contains(&c)) {
        return false;
    }
    if name.starts_with('-') || name.starts_with('.') {
        return false;
    }
    if name.ends_with('.') || name.ends_with('/') || name.ends_with(".lock") {
        return false;
    }
    if name.contains("..") || name.contains("@{") {
        return false;
    }
    true
}

// ── /stash ─────────────────────────────────────────────────────────────────

fn handle_stash(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/stash").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();
    let subcmd = args.first().copied().unwrap_or("");

    match subcmd {
        "pop" => match shell::run_command("git stash pop") {
            Ok(result) => {
                if result.success {
                    app.add_assistant_message(format!(
                        "Stash popped:\n\n```\n{}\n```",
                        result.stdout.trim()
                    ));
                    app.set_status("Stash popped");
                } else {
                    app.set_status(format!("Stash pop failed: {}", result.stderr));
                }
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        },
        "list" => match shell::run_command("git stash list") {
            Ok(result) => {
                if result.success {
                    if result.stdout.trim().is_empty() {
                        app.add_assistant_message("No stashes saved.".into());
                    } else {
                        app.add_assistant_message(format!(
                            "Git Stashes\n{}\n\n```\n{}\n```",
                            "=".repeat(25),
                            result.stdout.trim()
                        ));
                    }
                } else {
                    app.set_status(format!("git stash list failed: {}", result.stderr));
                }
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        },
        "drop" => {
            let stash_ref = args.get(1).copied().unwrap_or("stash@{0}");
            let cmd = format!("git stash drop '{}'", stash_ref.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.set_status(format!("Dropped {stash_ref}"));
                    } else {
                        app.set_status(format!("Stash drop failed: {}", result.stderr));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        "show" => {
            let stash_ref = args.get(1).copied().unwrap_or("stash@{0}");
            let cmd = format!("git stash show -p '{}'", stash_ref.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.add_assistant_message(format!(
                            "Stash {stash_ref}:\n\n```diff\n{}\n```",
                            result.stdout.trim()
                        ));
                    } else {
                        app.set_status(format!("Stash show failed: {}", result.stderr));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        "apply" => {
            let stash_ref = args.get(1).copied().unwrap_or("stash@{0}");
            let cmd = format!("git stash apply '{}'", stash_ref.replace('\'', "'\\''"));
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.set_status(format!("Applied {stash_ref}"));
                    } else {
                        app.set_status(format!("Stash apply failed: {}", result.stderr));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        "" => {
            // Default: stash with no message
            match shell::run_command("git stash push --include-untracked") {
                Ok(result) => {
                    if result.success {
                        if result.stdout.contains("No local changes") {
                            app.set_status("Nothing to stash");
                        } else {
                            app.set_status("Changes stashed");
                        }
                    } else {
                        app.set_status(format!("Stash failed: {}", result.stderr));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        _ => {
            // Treat everything else as a stash message
            let message = rest;
            let escaped = message.replace('\'', "'\\''");
            let cmd = format!("git stash push --include-untracked -m '{escaped}'");
            match shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        if result.stdout.contains("No local changes") {
                            app.set_status("Nothing to stash");
                        } else {
                            app.set_status(format!("Stashed: {message}"));
                        }
                    } else {
                        app.set_status(format!("Stash failed: {}", result.stderr));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
    }
    app.scroll_offset = 0;
    true
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

/// Parse `/gitbranch ...` input into (subcommand, name).
pub(crate) fn parse_gitbranch_args(input: &str) -> (&str, Option<&str>) {
    let rest = input.strip_prefix("/gitbranch").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();

    match args.len() {
        0 => ("list", None),
        1 => match args[0] {
            "switch" | "checkout" | "co" | "delete" | "rm" | "del" => (args[0], None),
            _ => ("create", Some(args[0])),
        },
        _ => (args[0], Some(args[1])),
    }
}

/// Parse `/stash ...` input into the subcommand.
pub(crate) fn parse_stash_subcommand(input: &str) -> &str {
    let rest = input.strip_prefix("/stash").unwrap_or("").trim();
    let first = rest.split_whitespace().next().unwrap_or("");
    match first {
        "pop" | "list" | "drop" | "show" | "apply" => first,
        "" => "push",
        _ => "push_with_message",
    }
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

    // ── Branch command parsing ─────────────────────────────────────────

    #[test]
    fn branch_list_default() {
        let (sub, name) = parse_gitbranch_args("/gitbranch");
        assert_eq!(sub, "list");
        assert!(name.is_none());
    }

    #[test]
    fn branch_create() {
        let (sub, name) = parse_gitbranch_args("/gitbranch feature-x");
        assert_eq!(sub, "create");
        assert_eq!(name, Some("feature-x"));
    }

    #[test]
    fn branch_switch() {
        let (sub, name) = parse_gitbranch_args("/gitbranch switch main");
        assert_eq!(sub, "switch");
        assert_eq!(name, Some("main"));
    }

    #[test]
    fn branch_delete() {
        let (sub, name) = parse_gitbranch_args("/gitbranch delete old-branch");
        assert_eq!(sub, "delete");
        assert_eq!(name, Some("old-branch"));
    }

    #[test]
    fn branch_delete_no_name() {
        let (sub, name) = parse_gitbranch_args("/gitbranch delete");
        assert_eq!(sub, "delete");
        assert!(name.is_none());
    }

    #[test]
    fn branch_checkout_alias() {
        let (sub, name) = parse_gitbranch_args("/gitbranch co develop");
        assert_eq!(sub, "co");
        assert_eq!(name, Some("develop"));
    }

    #[test]
    fn branch_rm_alias() {
        let (sub, name) = parse_gitbranch_args("/gitbranch rm feature");
        assert_eq!(sub, "rm");
        assert_eq!(name, Some("feature"));
    }

    // ── Branch name validation ─────────────────────────────────────────

    #[test]
    fn valid_branch_names() {
        assert!(is_valid_branch_name("feature-x"));
        assert!(is_valid_branch_name("fix/login-bug"));
        assert!(is_valid_branch_name("v1.0.0"));
        assert!(is_valid_branch_name("my_branch"));
        assert!(is_valid_branch_name("feature/JIRA-123"));
    }

    #[test]
    fn invalid_branch_names() {
        assert!(!is_valid_branch_name(""));
        assert!(!is_valid_branch_name("-starts-with-dash"));
        assert!(!is_valid_branch_name(".starts-with-dot"));
        assert!(!is_valid_branch_name("ends-with-dot."));
        assert!(!is_valid_branch_name("ends-with-slash/"));
        assert!(!is_valid_branch_name("has space"));
        assert!(!is_valid_branch_name("has~tilde"));
        assert!(!is_valid_branch_name("has^caret"));
        assert!(!is_valid_branch_name("has:colon"));
        assert!(!is_valid_branch_name("has?question"));
        assert!(!is_valid_branch_name("has*star"));
        assert!(!is_valid_branch_name("has[bracket"));
        assert!(!is_valid_branch_name("has..double-dot"));
        assert!(!is_valid_branch_name("has@{at-brace"));
        assert!(!is_valid_branch_name("name.lock"));
    }

    #[test]
    fn branch_name_with_slashes() {
        assert!(is_valid_branch_name("feature/add-login"));
        assert!(is_valid_branch_name("bugfix/PROJ-123/fix-crash"));
    }

    #[test]
    fn branch_name_lock_suffix() {
        assert!(!is_valid_branch_name("my-branch.lock"));
        assert!(is_valid_branch_name("my-branch.locked")); // only ".lock" is invalid
    }

    #[test]
    fn branch_with_extra_whitespace() {
        let (sub, name) = parse_gitbranch_args("/gitbranch   switch   main");
        assert_eq!(sub, "switch");
        assert_eq!(name, Some("main"));
    }

    // ── Stash parsing ──────────────────────────────────────────────────

    #[test]
    fn stash_subcommand_parsing() {
        assert_eq!(parse_stash_subcommand("/stash"), "push");
        assert_eq!(parse_stash_subcommand("/stash pop"), "pop");
        assert_eq!(parse_stash_subcommand("/stash list"), "list");
        assert_eq!(parse_stash_subcommand("/stash drop"), "drop");
        assert_eq!(parse_stash_subcommand("/stash show"), "show");
        assert_eq!(parse_stash_subcommand("/stash apply"), "apply");
        assert_eq!(
            parse_stash_subcommand("/stash my work in progress"),
            "push_with_message"
        );
    }

    #[test]
    fn stash_with_multiword_message() {
        assert_eq!(
            parse_stash_subcommand("/stash WIP: fixing auth flow"),
            "push_with_message"
        );
    }

    // ── Comprehensive edge case tests ──────────────────────────────────

    #[test]
    fn commit_message_preserves_full_text() {
        // The entire rest after "/commit " should be the message
        assert_eq!(
            parse_commit_message("/commit fix: resolve race condition in worker pool"),
            Some("fix: resolve race condition in worker pool")
        );
    }

    #[test]
    fn commit_message_multiline_not_supported() {
        // Input is single-line from the TUI, but newlines in message are preserved
        assert_eq!(
            parse_commit_message("/commit first line\nsecond line"),
            Some("first line\nsecond line")
        );
    }

    #[test]
    fn log_negative_values() {
        // Negative values parse as error, fall back to default
        assert_eq!(parse_log_count("/log -5"), 10);
    }

    #[test]
    fn log_zero_clamped() {
        assert_eq!(parse_log_count("/log 0"), 1);
    }

    #[test]
    fn log_very_large_clamped() {
        assert_eq!(parse_log_count("/log 999999"), 100);
    }

    #[test]
    fn branch_empty_after_whitespace() {
        let (sub, name) = parse_gitbranch_args("/gitbranch   ");
        assert_eq!(sub, "list");
        assert!(name.is_none());
    }

    #[test]
    fn branch_del_alias() {
        let (sub, name) = parse_gitbranch_args("/gitbranch del feature");
        assert_eq!(sub, "del");
        assert_eq!(name, Some("feature"));
    }

    #[test]
    fn branch_name_with_numbers() {
        assert!(is_valid_branch_name("feature-123"));
        assert!(is_valid_branch_name("123"));
        assert!(is_valid_branch_name("v2.0.0-rc1"));
    }

    #[test]
    fn branch_name_backslash() {
        assert!(!is_valid_branch_name("has\\backslash"));
    }

    #[test]
    fn branch_name_double_dot_in_middle() {
        assert!(!is_valid_branch_name("a..b"));
    }

    #[test]
    fn branch_name_at_brace() {
        assert!(!is_valid_branch_name("ref@{1}"));
    }

    #[test]
    fn stash_empty_space() {
        assert_eq!(parse_stash_subcommand("/stash "), "push");
    }

    #[test]
    fn stash_drop_with_ref() {
        // "drop" is the subcommand; the ref after is handled by the handler
        assert_eq!(parse_stash_subcommand("/stash drop stash@{2}"), "drop");
    }

    #[test]
    fn stash_show_with_ref() {
        assert_eq!(parse_stash_subcommand("/stash show stash@{0}"), "show");
    }

    #[test]
    fn stash_apply_with_ref() {
        assert_eq!(parse_stash_subcommand("/stash apply stash@{1}"), "apply");
    }

    #[test]
    fn stash_unknown_becomes_message() {
        assert_eq!(
            parse_stash_subcommand("/stash save my changes"),
            "push_with_message"
        );
    }

    #[test]
    fn commit_message_with_unicode() {
        assert_eq!(
            parse_commit_message("/commit feat: add emoji support \u{2728}"),
            Some("feat: add emoji support \u{2728}")
        );
    }

    #[test]
    fn branch_name_unicode_letters() {
        // Git allows unicode in branch names
        assert!(is_valid_branch_name("feature/\u{00e4}\u{00f6}\u{00fc}"));
    }

    #[test]
    fn commit_dispatch_guard_prevents_prefix_match() {
        // The handle() dispatch checks `trimmed == "/commit" || trimmed.starts_with("/commit ")`
        // so "/committed" never reaches parse_commit_message. Verify the dispatch pattern:
        let input = "/committed something";
        let matches_dispatch = input == "/commit" || input.starts_with("/commit ");
        assert!(!matches_dispatch, "dispatch should not match '/committed'");
    }

    #[test]
    fn gitbranch_three_args() {
        // Only first two matter for parsing
        let (sub, name) = parse_gitbranch_args("/gitbranch switch feature extra");
        assert_eq!(sub, "switch");
        assert_eq!(name, Some("feature"));
    }

    #[test]
    fn log_float_value() {
        // "3.5" doesn't parse as usize, falls back to default
        assert_eq!(parse_log_count("/log 3.5"), 10);
    }

    #[test]
    fn branch_name_only_dots() {
        assert!(!is_valid_branch_name(".."));
        assert!(!is_valid_branch_name("."));
    }

    #[test]
    fn branch_name_single_char() {
        assert!(is_valid_branch_name("a"));
        assert!(is_valid_branch_name("X"));
    }
}
