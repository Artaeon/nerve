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
                aliased.clone()
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
