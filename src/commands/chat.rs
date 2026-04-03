//! Chat commands: /new, /clear, /delete, /export, /copy, /system, /rename,
//! /session, /branch, /!!, /repeat

use std::sync::Arc;

use crate::ai::provider::AiProvider;
use crate::app::{self, App, InputMode};
use crate::clipboard;
use crate::session;

/// Handle chat-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str, _provider: &Arc<dyn AiProvider>) -> bool {
    if trimmed == "/clear" {
        return handle_clear(app);
    }

    if trimmed == "/new" {
        app.new_conversation();
        return true;
    }

    if trimmed == "/export" {
        return handle_export(app);
    }

    if crate::shell::matches_command(trimmed, "/system") {
        return handle_system(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/rename") {
        return handle_rename(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/delete") {
        return handle_delete(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/copy") {
        return handle_copy(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/session") {
        return handle_session(app, trimmed);
    }

    if trimmed == "/branch"
        || trimmed == "/br"
        || trimmed.starts_with("/branch ")
        || trimmed.starts_with("/br ")
    {
        return handle_branch(app, trimmed);
    }

    if trimmed == "/!!" || trimmed == "/repeat" {
        return handle_repeat(app);
    }

    false
}

fn handle_clear(app: &mut App) -> bool {
    app.current_conversation_mut().messages.clear();
    app.streaming_response.clear();
    app.is_streaming = false;
    app.stream_rx = None;
    app.scroll_offset = 0;
    app.set_status("Conversation cleared");
    true
}

fn handle_export(app: &mut App) -> bool {
    let conv = app.current_conversation();
    let export_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nerve")
        .join("exports");
    std::fs::create_dir_all(&export_dir).ok();

    let filename = format!(
        "conversation_{}.md",
        conv.id.chars().take(8).collect::<String>()
    );
    let path = export_dir.join(&filename);

    let mut content = format!(
        "# {}\nModel: {} | Provider: {}\nDate: {}\n\n---\n\n",
        conv.title,
        app.selected_model,
        app.selected_provider,
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    );

    for (role, msg) in &conv.messages {
        let label = match role.as_str() {
            "user" => "You",
            "assistant" => "AI",
            "system" => "System",
            _ => role,
        };
        content.push_str(&format!("## {}\n{}\n\n---\n\n", label, msg));
    }

    match std::fs::write(&path, &content) {
        Ok(()) => app.set_status(format!("Exported to {}", path.display())),
        Err(e) => app.set_status(format!("Export error: {e}")),
    }
    true
}

fn handle_system(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/system").unwrap_or("").trim();
    if rest.is_empty() {
        let sys = app
            .current_conversation()
            .messages
            .iter()
            .find(|(r, _)| r == "system")
            .map(|(_, c)| c.clone());
        match sys {
            Some(prompt) => {
                app.add_assistant_message(format!("Current system prompt:\n\n{prompt}"))
            }
            None => app.add_assistant_message(
                "No system prompt set. Use /system <prompt> to set one.".into(),
            ),
        }
    } else if rest == "clear" {
        app.current_conversation_mut()
            .messages
            .retain(|(r, _)| r != "system");
        app.set_status("System prompt cleared");
    } else {
        let prompt = rest.to_string();
        app.current_conversation_mut()
            .messages
            .retain(|(r, _)| r != "system");
        app.current_conversation_mut()
            .messages
            .insert(0, ("system".into(), prompt));
        app.set_status("System prompt set");
    }
    app.scroll_offset = 0;
    true
}

fn handle_rename(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/rename").unwrap_or("").trim();
    if rest.is_empty() {
        app.add_assistant_message("Usage: /rename <new title>".into());
    } else {
        let new_title = rest.to_string();
        app.current_conversation_mut().title = new_title.clone();
        app.set_status(format!("Renamed to: {new_title}"));
    }
    app.scroll_offset = 0;
    true
}

fn handle_delete(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/delete").unwrap_or("").trim();
    if rest == "all" {
        app.conversations.clear();
        app.conversations.push(app::Conversation::new());
        app.active_conversation = 0;
        app.scroll_offset = 0;
        app.set_status("All conversations deleted");
    } else if app.conversations.len() <= 1 {
        app.current_conversation_mut().messages.clear();
        app.current_conversation_mut().title = "New Conversation".into();
        app.set_status("Conversation cleared");
    } else {
        app.conversations.remove(app.active_conversation);
        if app.active_conversation >= app.conversations.len() {
            app.active_conversation = app.conversations.len() - 1;
        }
        app.scroll_offset = 0;
        app.set_status("Conversation deleted");
    }
    true
}

fn handle_copy(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/copy").unwrap_or("").trim();
    let conv = app.current_conversation();
    let text = match rest {
        "all" => conv
            .messages
            .iter()
            .map(|(role, content)| format!("{}: {}", role, content))
            .collect::<Vec<_>>()
            .join("\n\n"),
        "last" => conv
            .messages
            .last()
            .map(|(_, c)| c.clone())
            .unwrap_or_default(),
        "code" => conv
            .messages
            .iter()
            .rev()
            .filter(|(r, _)| r == "assistant")
            .find_map(|(_, content)| {
                let mut in_block = false;
                let mut code = String::new();
                let mut last_code: Option<String> = None;
                for line in content.lines() {
                    if line.starts_with("```") {
                        if in_block {
                            last_code = Some(code.clone());
                            code.clear();
                            in_block = false;
                        } else {
                            in_block = true;
                            code.clear();
                        }
                    } else if in_block {
                        if !code.is_empty() {
                            code.push('\n');
                        }
                        code.push_str(line);
                    }
                }
                last_code
            })
            .unwrap_or_default(),
        other => {
            if let Ok(num) = other.parse::<usize>() {
                let idx = conv.messages.len().saturating_sub(num);
                conv.messages
                    .get(idx)
                    .map(|(_, c)| c.clone())
                    .unwrap_or_default()
            } else {
                conv.messages
                    .iter()
                    .rev()
                    .find(|(r, _)| r == "assistant")
                    .map(|(_, c)| c.clone())
                    .unwrap_or_default()
            }
        }
    };
    if text.is_empty() {
        app.set_status("Nothing to copy");
    } else {
        match clipboard::copy_to_clipboard(&text) {
            Ok(()) => app.set_status("Copied to clipboard"),
            Err(e) => app.set_status(format!("Clipboard error: {e}")),
        }
    }
    true
}

fn handle_session(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/session").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();
    let subcmd = args.first().copied().unwrap_or("info");
    match subcmd {
        "save" => {
            let sess = session::session_from_app(app);
            match session::save_session(&sess) {
                Ok(()) => app.set_status("Session saved"),
                Err(e) => app.report_error(e),
            }
        }
        "list" => match session::list_sessions() {
            Ok(sessions) => {
                if sessions.is_empty() {
                    app.add_assistant_message("No saved sessions.".into());
                } else {
                    let mut msg = "Saved sessions:\n\n".to_string();
                    for (id, date, count) in &sessions {
                        msg.push_str(&format!(
                            "  {} \u{2014} {} ({} conv)\n",
                            &id[..8],
                            date.format("%Y-%m-%d %H:%M"),
                            count
                        ));
                    }
                    msg.push_str("\nResume with: nerve --continue");
                    app.add_assistant_message(msg);
                }
            }
            Err(e) => app.report_error(e),
        },
        "restore" => match session::load_last_session() {
            Ok(sess) => {
                session::restore_session_to_app(&sess, app);
                app.set_status(format!(
                    "Session restored ({} conversations)",
                    app.conversations.len()
                ));
            }
            Err(e) => app.report_error(e),
        },
        _ => {
            let sess_info = format!(
                "Session Info:\n  Conversations: {}\n  Active: {}\n  Model: {}\n  Provider: {}\n\nCommands:\n  /session save     Save current session\n  /session list     List saved sessions\n  /session restore  Restore last session\n  nerve --continue  Resume on startup",
                app.conversations.len(),
                app.current_conversation().title,
                app.selected_model,
                app.selected_provider
            );
            app.add_assistant_message(sess_info);
        }
    }
    true
}

fn handle_branch(app: &mut App, trimmed: &str) -> bool {
    let rest = if trimmed.starts_with("/branch") {
        trimmed.strip_prefix("/branch").unwrap_or("").trim()
    } else {
        trimmed.strip_prefix("/br").unwrap_or("").trim()
    };
    let args: Vec<&str> = rest.split_whitespace().collect();
    let subcmd = args.first().copied().unwrap_or("list");
    match subcmd {
        "save" | "create" => {
            let name = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                format!("Branch {}", app.branches.len() + 1)
            };
            app.create_branch(name.clone());
            app.set_status(format!("Branch saved: {name}"));
        }
        "list" => {
            if app.branches.is_empty() {
                app.add_assistant_message(
                    "No branches saved.\n\nUsage:\n  /branch save [name]  Save current state\n  /branch restore <n>  Restore a branch\n  /branch delete <n>   Delete a branch\n  /branch diff <n>     Compare with a branch".into()
                );
            } else {
                let mut msg = "Saved branches:\n\n".to_string();
                for (i, branch) in app.branches.iter().enumerate() {
                    let msg_count = branch.messages.len();
                    let time = branch.created_at.format("%H:%M:%S");
                    msg.push_str(&format!(
                        "  {}. {} ({} messages, saved at {})\n",
                        i + 1,
                        branch.name,
                        msg_count,
                        time
                    ));
                }
                msg.push_str("\nUsage: /branch restore <number> | /branch delete <number>");
                app.add_assistant_message(msg);
            }
        }
        "restore" | "load" => {
            if let Some(idx_str) = args.get(1) {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    let idx = idx.saturating_sub(1);
                    if idx < app.branches.len() {
                        let name = app.branches[idx].name.clone();
                        app.restore_branch(idx);
                        app.set_status(format!("Restored branch: {name}"));
                    } else {
                        app.set_status("Invalid branch number");
                    }
                } else {
                    app.set_status("Usage: /branch restore <number>");
                }
            } else {
                app.set_status("Usage: /branch restore <number>");
            }
        }
        "delete" | "rm" => {
            if let Some(idx_str) = args.get(1)
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                let idx = idx.saturating_sub(1);
                if idx < app.branches.len() {
                    let name = app.branches[idx].name.clone();
                    app.delete_branch(idx);
                    app.set_status(format!("Deleted branch: {name}"));
                } else {
                    app.set_status("Invalid branch number");
                }
            }
        }
        "diff" => {
            if let Some(idx_str) = args.get(1)
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                let idx = idx.saturating_sub(1);
                if let Some(branch) = app.branches.get(idx) {
                    let current = &app.current_conversation().messages;
                    let branched = &branch.messages;

                    let common = current
                        .iter()
                        .zip(branched.iter())
                        .take_while(|(a, b)| a == b)
                        .count();

                    let mut msg =
                        format!("Diff with branch '{}'\n{}\n\n", branch.name, "=".repeat(30));
                    msg.push_str(&format!("Common messages: {}\n", common));
                    msg.push_str(&format!(
                        "Current has {} more message(s)\n",
                        current.len().saturating_sub(common)
                    ));
                    msg.push_str(&format!(
                        "Branch has {} more message(s)\n\n",
                        branched.len().saturating_sub(common)
                    ));

                    if current.len() > common {
                        msg.push_str("Current (diverged):\n");
                        for (role, content) in &current[common..] {
                            let brief: String = content.chars().take(80).collect();
                            msg.push_str(&format!("  [{role}] {brief}...\n"));
                        }
                    }
                    if branched.len() > common {
                        msg.push_str("\nBranch (diverged):\n");
                        for (role, content) in &branched[common..] {
                            let brief: String = content.chars().take(80).collect();
                            msg.push_str(&format!("  [{role}] {brief}...\n"));
                        }
                    }

                    app.add_assistant_message(msg);
                }
            }
        }
        _ => {
            app.add_assistant_message("Usage: /branch save|list|restore|delete|diff".into());
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_repeat(app: &mut App) -> bool {
    if let Some(last) = app.input_history.last().cloned() {
        app.input = last;
        app.cursor_position = app.input.len();
        app.input_mode = InputMode::Insert;
        app.set_status("Loaded last input \u{2014} press Enter to send");
    } else {
        app.set_status("No previous input");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{AiProvider, ChatMessage, ModelInfo, StreamEvent};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    /// Minimal mock provider that does nothing (we only test parsing/routing).
    struct MockProvider;

    impl AiProvider for MockProvider {
        fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _model: &str,
            _tx: mpsc::UnboundedSender<StreamEvent>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }

        fn chat(
            &self,
            _messages: &[ChatMessage],
            _model: &str,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
            Box::pin(async { Ok(String::new()) })
        }

        fn list_models(
            &self,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
            Box::pin(async { Ok(vec![]) })
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    fn mock_provider() -> Arc<dyn AiProvider> {
        Arc::new(MockProvider)
    }

    // ── /clear ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn clear_is_handled() {
        let mut app = App::new();
        app.current_conversation_mut()
            .messages
            .push(("user".into(), "hello".into()));
        let provider = mock_provider();
        assert!(handle(&mut app, "/clear", &provider).await);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[tokio::test]
    async fn clearall_does_not_match_clear() {
        let mut app = App::new();
        let provider = mock_provider();
        // /clearall should NOT be handled by this module
        assert!(!handle(&mut app, "/clearall", &provider).await);
    }

    // ── /new ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn new_creates_conversation() {
        let mut app = App::new();
        assert_eq!(app.conversations.len(), 1);
        let provider = mock_provider();
        assert!(handle(&mut app, "/new", &provider).await);
        assert_eq!(app.conversations.len(), 2);
    }

    // ── /copy ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn copy_bare_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/copy", &provider).await);
    }

    #[tokio::test]
    async fn copy_all_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/copy all", &provider).await);
    }

    #[tokio::test]
    async fn copy_code_is_handled() {
        let mut app = App::new();
        app.current_conversation_mut().messages.push((
            "assistant".into(),
            "Here:\n```rust\nfn main() {}\n```".into(),
        ));
        let provider = mock_provider();
        assert!(handle(&mut app, "/copy code", &provider).await);
    }

    #[tokio::test]
    async fn copy_last_is_handled() {
        let mut app = App::new();
        app.current_conversation_mut()
            .messages
            .push(("assistant".into(), "hi".into()));
        let provider = mock_provider();
        assert!(handle(&mut app, "/copy last", &provider).await);
    }

    // ── /session ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn session_bare_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/session", &provider).await);
        // Bare /session shows info — should add an assistant message
        let last = app.current_conversation().messages.last().unwrap();
        assert_eq!(last.0, "assistant");
        assert!(last.1.contains("Session Info"));
    }

    #[tokio::test]
    async fn session_save_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/session save", &provider).await);
    }

    #[tokio::test]
    async fn session_list_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/session list", &provider).await);
    }

    #[tokio::test]
    async fn session_restore_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        // This will likely fail to find a session, but should still return true
        assert!(handle(&mut app, "/session restore", &provider).await);
    }

    // ── /branch ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn branch_list_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/branch list", &provider).await);
    }

    #[tokio::test]
    async fn branch_save_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/branch save my branch", &provider).await);
        assert_eq!(app.branches.len(), 1);
        assert_eq!(app.branches[0].name, "my branch");
    }

    #[tokio::test]
    async fn branch_restore_is_handled() {
        let mut app = App::new();
        app.create_branch("test".into());
        let provider = mock_provider();
        assert!(handle(&mut app, "/branch restore 1", &provider).await);
    }

    #[tokio::test]
    async fn br_shorthand_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/br", &provider).await);
        assert!(handle(&mut app, "/br save shorthand", &provider).await);
        assert_eq!(app.branches.len(), 1);
        assert_eq!(app.branches[0].name, "shorthand");
    }

    // ── /rename ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn rename_extracts_new_title() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/rename new name here", &provider).await);
        assert_eq!(app.current_conversation().title, "new name here");
    }

    #[tokio::test]
    async fn rename_bare_shows_usage() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/rename", &provider).await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Usage"));
    }

    // ── /system ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn system_set_extracts_prompt() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/system set You are a pirate", &provider).await);
        let sys = app
            .current_conversation()
            .messages
            .iter()
            .find(|(r, _)| r == "system");
        assert!(sys.is_some());
        assert_eq!(sys.unwrap().1, "set You are a pirate");
    }

    #[tokio::test]
    async fn system_bare_shows_current() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/system", &provider).await);
        // Should add an assistant message about no system prompt
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("No system prompt set"));
    }

    #[tokio::test]
    async fn system_clear_removes_prompt() {
        let mut app = App::new();
        app.current_conversation_mut()
            .messages
            .push(("system".into(), "old prompt".into()));
        let provider = mock_provider();
        assert!(handle(&mut app, "/system clear", &provider).await);
        assert!(
            app.current_conversation()
                .messages
                .iter()
                .all(|(r, _)| r != "system")
        );
    }

    // ── /delete ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_all_clears_conversations() {
        let mut app = App::new();
        app.new_conversation();
        assert_eq!(app.conversations.len(), 2);
        let provider = mock_provider();
        assert!(handle(&mut app, "/delete all", &provider).await);
        assert_eq!(app.conversations.len(), 1);
    }

    // ── /export ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn export_is_handled() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/export", &provider).await);
    }

    // ── /!! and /repeat ─────────────────────────────────────────────────

    #[tokio::test]
    async fn repeat_with_history() {
        let mut app = App::new();
        app.input_history.push("hello world".into());
        let provider = mock_provider();
        assert!(handle(&mut app, "/!!", &provider).await);
        assert_eq!(app.input, "hello world");
    }

    #[tokio::test]
    async fn repeat_alias_works() {
        let mut app = App::new();
        app.input_history.push("test".into());
        let provider = mock_provider();
        assert!(handle(&mut app, "/repeat", &provider).await);
        assert_eq!(app.input, "test");
    }

    #[tokio::test]
    async fn repeat_no_history() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(handle(&mut app, "/!!", &provider).await);
        assert_eq!(app.status_message.as_deref(), Some("No previous input"));
    }

    // ── Unrecognised commands ───────────────────────────────────────────

    #[tokio::test]
    async fn unrecognised_returns_false() {
        let mut app = App::new();
        let provider = mock_provider();
        assert!(!handle(&mut app, "/unknown", &provider).await);
        assert!(!handle(&mut app, "/copying", &provider).await);
        assert!(!handle(&mut app, "/newx", &provider).await);
    }
}
