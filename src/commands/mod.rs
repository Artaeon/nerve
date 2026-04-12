//! Slash-command dispatch and handlers.
//!
//! Each sub-module groups related commands. The top-level [`handle`] function
//! dispatches to the appropriate handler, returning `true` if the command was
//! recognised (so the caller should *not* forward the text to the AI).

pub mod ai;
pub mod chat;
pub mod files;
pub mod git;
pub mod info;
pub mod knowledge;
pub mod settings;
pub mod shell;

use std::sync::Arc;

use crate::ai::provider::AiProvider;
use crate::app::App;

/// Handle a slash command.  Returns `true` if recognised and handled.
pub async fn handle(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) -> bool {
    let trimmed = text.trim();

    // Try each command group in turn.  The first handler that recognises the
    // command returns `true` and short-circuits the rest.

    if info::handle(app, trimmed).await {
        return true;
    }

    if chat::handle(app, trimmed, provider).await {
        return true;
    }

    if ai::handle(app, trimmed).await {
        return true;
    }

    if shell::handle(app, trimmed).await {
        return true;
    }

    if git::handle(app, trimmed, provider).await {
        return true;
    }

    if files::handle(app, trimmed, provider).await {
        return true;
    }

    if knowledge::handle(app, trimmed, provider).await {
        return true;
    }

    if settings::handle(app, trimmed).await {
        return true;
    }

    // ── Plugin dispatch ────────────────────────────────────────────────
    {
        let cmd = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let (plugin_cmd, plugin_args) = match cmd.find(char::is_whitespace) {
            Some(pos) => (&cmd[..pos], cmd[pos..].trim()),
            None => (cmd, ""),
        };
        for plugin in &app.plugins {
            if plugin.manifest.command == plugin_cmd {
                match plugin.execute(plugin_args, "") {
                    Ok(output) => {
                        app.add_assistant_message(output);
                    }
                    Err(e) => {
                        app.set_status(format!("Plugin error: {e}"));
                    }
                }
                return true;
            }
        }
    }

    // ── Alias expansion (check before returning false) ─────────────────
    {
        let cmd_name = trimmed
            .strip_prefix('/')
            .unwrap_or("")
            .split_whitespace()
            .next()
            .unwrap_or("");
        let remaining_args = trimmed
            .strip_prefix('/')
            .unwrap_or("")
            .strip_prefix(cmd_name)
            .unwrap_or("")
            .trim();

        if let Some(aliased) = app.aliases.get(cmd_name).cloned() {
            let full = if remaining_args.is_empty() {
                aliased
            } else {
                format!("{aliased} {remaining_args}")
            };
            app.input = full;
            app.cursor_position = app.input.len();
            app.input_mode = crate::app::InputMode::Insert;
            app.set_status(format!("Expanded alias: /{cmd_name}"));
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{AiProvider, ChatMessage, ModelInfo, StreamEvent};
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc;

    struct DummyProvider;
    impl AiProvider for DummyProvider {
        fn chat_stream(
            &self,
            _: &[ChatMessage],
            _: &str,
            _: mpsc::UnboundedSender<StreamEvent>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
        fn chat(
            &self,
            _: &[ChatMessage],
            _: &str,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
            Box::pin(async { Ok(String::new()) })
        }
        fn list_models(
            &self,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn name(&self) -> &str {
            "dummy"
        }
    }

    fn provider() -> Arc<dyn AiProvider> {
        Arc::new(DummyProvider)
    }

    // ── Command routing ─────────────────────────────────────────────

    #[tokio::test]
    async fn dispatches_to_info_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/help", &provider()).await);
    }

    #[tokio::test]
    async fn dispatches_to_chat_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/new", &provider()).await);
    }

    #[tokio::test]
    async fn dispatches_to_ai_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/models", &provider()).await);
    }

    #[tokio::test]
    async fn dispatches_to_shell_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/run echo hi", &provider()).await);
    }

    #[tokio::test]
    async fn dispatches_to_git_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/diff", &provider()).await);
    }

    #[tokio::test]
    async fn dispatches_to_settings_handler() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme list", &provider()).await);
    }

    #[tokio::test]
    async fn unrecognized_command_returns_false() {
        let mut app = App::new();
        assert!(!handle(&mut app, "/totally_unknown_cmd", &provider()).await);
    }

    #[tokio::test]
    async fn plain_text_returns_false() {
        let mut app = App::new();
        assert!(!handle(&mut app, "hello world", &provider()).await);
    }

    #[tokio::test]
    async fn empty_input_returns_false() {
        let mut app = App::new();
        assert!(!handle(&mut app, "", &provider()).await);
    }

    #[tokio::test]
    async fn alias_expansion_works() {
        let mut app = App::new();
        app.aliases
            .insert("myalias".into(), "/run echo aliased".into());
        assert!(handle(&mut app, "/myalias", &provider()).await);
        assert!(app.input.contains("/run echo aliased"));
    }

    #[tokio::test]
    async fn alias_with_args_appended() {
        let mut app = App::new();
        app.aliases.insert("greet".into(), "/run echo hello".into());
        assert!(handle(&mut app, "/greet world", &provider()).await);
        assert!(app.input.contains("/run echo hello world"));
    }

    #[tokio::test]
    async fn whitespace_trimmed() {
        let mut app = App::new();
        assert!(handle(&mut app, "  /help  ", &provider()).await);
    }
}
