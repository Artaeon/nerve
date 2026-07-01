use std::sync::Arc;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::ai::provider::AiProvider;
use crate::app::{App, AppMode, InputMode};
use crate::clipboard_manager::ClipboardSource;
use crate::config::Config;
use crate::{
    accept_autocomplete, clear_conversation, common_prefix, complete_file_path,
    copy_last_assistant_message, cycle_conversation, cycle_conversation_back, delete_last_exchange,
    edit_last_message, get_all_commands, list_file_matches, regenerate_response, submit_message,
    update_autocomplete,
};
use crate::{clipboard, history};

mod history_search;
mod overlays;
mod settings;

// Re-exported so the test module in `main.rs` can reach it at
// `crate::input::update_search_results`.
#[cfg(test)]
pub(crate) use history_search::update_search_results;

// ─── Key event handling ─────────────────────────────────────────────────────

pub(crate) async fn handle_key_event(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    // Escape while streaming = stop generation. Also cancels any active
    // workflow so the next user turn doesn't accidentally resume as a
    // pipeline role.
    if app.is_streaming && code == KeyCode::Esc {
        let was_auto_agent_active = app.auto_agent_active;
        app.finish_streaming();
        let was_in_pipeline = app.pipeline.take().is_some();
        let msg = if was_in_pipeline {
            // A workflow's Coder/Reviewer role turns agent mode on; clear it
            // on cancel so the next plain message isn't silently in agent mode.
            app.agent_mode = false;
            "Workflow cancelled"
        } else {
            // Cancelling a silently-activated auto-agent turn must revert it too.
            app.revert_auto_agent_activation(was_auto_agent_active);
            "Generation stopped"
        };
        app.set_status(msg);
        return Ok(());
    }

    // ── Global keys (always active) ─────────────────────────────────────
    if mods.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('c') | KeyCode::Char('d') => {
                // Graceful shutdown: save state before quitting.
                let _ = app.clipboard_manager.save();
                // Save current conversation if it has messages.
                if !app.current_conversation().messages.is_empty() {
                    let conv = app.current_conversation();
                    let record = history::ConversationRecord {
                        id: conv.id.clone(),
                        title: conv.title.clone(),
                        messages: conv
                            .messages
                            .iter()
                            .map(|(role, content)| history::MessageRecord {
                                role: role.clone(),
                                content: content.clone(),
                                timestamp: chrono::Utc::now(),
                            })
                            .collect(),
                        model: app.selected_model.clone(),
                        provider: app.selected_provider.clone(),
                        created_at: conv.created_at,
                        updated_at: chrono::Utc::now(),
                    };
                    let _ = history::save_conversation(&record);
                }
                // Save any in-progress generation before quitting.
                if app.is_streaming {
                    app.finish_streaming();
                }
                app.should_quit = true;
                return Ok(());
            }
            KeyCode::Char('h') => {
                if app.mode == AppMode::Help {
                    app.mode = AppMode::Normal;
                } else {
                    app.mode = AppMode::Help;
                    app.help_scroll = 0;
                }
                return Ok(());
            }
            KeyCode::Char('f') => {
                app.mode = AppMode::SearchOverlay;
                app.search_query.clear();
                app.search_results.clear();
                app.search_current = 0;
                return Ok(());
            }
            _ => {}
        }
    }

    // ── Dispatch by mode ────────────────────────────────────────────────
    match app.mode {
        AppMode::Normal => handle_normal_mode(app, key, provider, config).await?,
        AppMode::CommandBar => overlays::handle_command_bar(app, key),
        AppMode::PromptPicker => overlays::handle_prompt_picker(app, key),
        AppMode::ModelSelect => overlays::handle_model_select(app, key),
        AppMode::ProviderSelect => overlays::handle_provider_select(app, key),
        AppMode::Help => {
            // Scroll the help content; Esc (or q) closes.
            match code {
                KeyCode::Esc | KeyCode::Char('q') => app.mode = AppMode::Normal,
                KeyCode::Char('j') | KeyCode::Down => {
                    app.help_scroll = (app.help_scroll + 1).min(app.help_max_scroll.get());
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                }
                KeyCode::Char('g') | KeyCode::Home => app.help_scroll = 0,
                KeyCode::Char('G') | KeyCode::End => {
                    app.help_scroll = app.help_max_scroll.get();
                }
                KeyCode::PageDown => {
                    app.help_scroll = (app.help_scroll + 10).min(app.help_max_scroll.get());
                }
                KeyCode::PageUp => app.help_scroll = app.help_scroll.saturating_sub(10),
                _ => {}
            }
        }
        AppMode::Settings => settings::handle_settings(app, key),
        AppMode::ClipboardManager => overlays::handle_clipboard_manager(app, key),
        AppMode::HistoryBrowser => history_search::handle_history_browser(app, key),
        AppMode::SearchOverlay => history_search::handle_search(app, key),
    }

    Ok(())
}

// ── Common Ctrl handler ────────────────────────────────────────────────────

/// Handle Ctrl+<key> commands that work in both Normal and Insert modes.
/// Returns `true` if the key was handled.
async fn handle_common_ctrl(
    app: &mut App,
    code: KeyCode,
    provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<bool> {
    match code {
        KeyCode::Char('k') => {
            app.mode = AppMode::CommandBar;
            app.command_bar_input.clear();
            app.command_bar_select_index = 0;
            app.command_bar_category = 0;
            Ok(true)
        }
        KeyCode::Char('n') => {
            app.new_conversation();
            Ok(true)
        }
        KeyCode::Char('p') => {
            app.mode = AppMode::PromptPicker;
            app.prompt_filter.clear();
            app.prompt_select_index = 0;
            app.prompt_category_index = 0;
            app.prompt_focus_right = false;
            Ok(true)
        }
        KeyCode::Char('m') => {
            app.mode = AppMode::ModelSelect;
            app.model_select_index = app
                .available_models
                .iter()
                .position(|m| m == &app.selected_model)
                .unwrap_or(0);
            Ok(true)
        }
        KeyCode::Char('t') => {
            app.mode = AppMode::ProviderSelect;
            app.provider_select_index = app
                .available_providers
                .iter()
                .position(|p| p == &app.selected_provider)
                .unwrap_or(0);
            Ok(true)
        }
        KeyCode::Char('y') => {
            copy_last_assistant_message(app);
            Ok(true)
        }
        KeyCode::Char('l') => {
            clear_conversation(app);
            Ok(true)
        }
        KeyCode::Char('b') => {
            app.mode = AppMode::ClipboardManager;
            app.clipboard_search.clear();
            app.clipboard_select_index = 0;
            Ok(true)
        }
        KeyCode::Char('o') => {
            app.history_entries = history::list_conversations().unwrap_or_default();
            app.history_select_index = 0;
            app.history_search.clear();
            app.history_delete_pending = false;
            app.history_sort = 0;
            app.mode = AppMode::HistoryBrowser;
            Ok(true)
        }
        KeyCode::Char('r') => {
            regenerate_response(app, provider, config).await;
            Ok(true)
        }
        KeyCode::Char('e') => {
            edit_last_message(app);
            Ok(true)
        }
        KeyCode::Char(',') => {
            app.mode = AppMode::Settings;
            app.settings_tab = 0;
            app.settings_select = 0;
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Normal mode ─────────────────────────────────────────────────────────────

async fn handle_normal_mode(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    match app.input_mode {
        // ── Normal / vim-navigation ─────────────────────────────────────
        InputMode::Normal => {
            if mods.contains(KeyModifiers::CONTROL) {
                if handle_common_ctrl(app, code, provider, config).await? {
                    return Ok(());
                }
                // No mode-specific Ctrl keys in Normal mode.
                return Ok(());
            }

            match code {
                KeyCode::Char('i') => app.input_mode = InputMode::Insert,
                KeyCode::Char('?') => {
                    // `?` opens help (advertised in the status bar/hints) —
                    // a reliable alternative to Ctrl+H, which some terminals
                    // swallow as Backspace.
                    app.mode = AppMode::Help;
                    app.help_scroll = 0;
                }
                KeyCode::Char('/') => {
                    // Switch to Insert mode and insert '/' so the user can
                    // type slash commands directly (e.g. /help, /agent on).
                    app.input_mode = InputMode::Insert;
                    app.insert_char('/');
                    update_autocomplete(app);
                }
                KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
                KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
                KeyCode::Char('G') => app.scroll_to_bottom(),
                KeyCode::Char('g') => app.scroll_to_top(),
                KeyCode::PageUp => {
                    for _ in 0..10 {
                        app.scroll_up();
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..10 {
                        app.scroll_down();
                    }
                }
                KeyCode::Tab => cycle_conversation(app),
                KeyCode::BackTab => cycle_conversation_back(app),
                KeyCode::Char('q') => app.should_quit = true,
                KeyCode::Char('x') => delete_last_exchange(app),
                KeyCode::Char(c @ '1'..='9') => {
                    let n = c.to_digit(10).expect("char matched '1'..='9'") as usize;
                    let conv = app.current_conversation();
                    // #n counts back from the newest message (#1 == last). Use
                    // checked_sub so that when the conversation has fewer than n
                    // messages we fall through to "No message #n" instead of
                    // saturating to index 0 and copying/mislabelling the oldest.
                    let idx = conv.messages.len().checked_sub(n);
                    if let Some((role, content)) = idx.and_then(|i| conv.messages.get(i)) {
                        let role = role.clone();
                        let content = content.clone();
                        match clipboard::copy_to_clipboard(&content) {
                            Ok(()) => {
                                app.clipboard_manager
                                    .add(content, ClipboardSource::ManualCopy);
                                let _ = app.clipboard_manager.save();
                                app.set_status(format!(
                                    "Copied message #{n} ({role}) to clipboard"
                                ));
                            }
                            Err(e) => app.set_status(format!("Clipboard error: {e}")),
                        }
                    } else {
                        app.set_status(format!("No message #{n}"));
                    }
                }
                _ => {}
            }
        }

        // ── Insert / typing mode ────────────────────────────────────────
        InputMode::Insert => {
            if mods.contains(KeyModifiers::CONTROL) {
                if handle_common_ctrl(app, code, provider, config).await? {
                    return Ok(());
                }
                // Insert-mode-specific Ctrl keys.
                match code {
                    KeyCode::Char('v') => {
                        if let Ok(text) = clipboard::paste_from_clipboard() {
                            for ch in text.chars() {
                                app.insert_char(ch);
                            }
                            update_autocomplete(app);
                        }
                    }
                    KeyCode::Char('w') => {
                        // Delete word before cursor. `word_delete_start`
                        // returns a char-boundary offset so multi-byte
                        // whitespace can't cause a slicing panic.
                        let pos = app.cursor_position.min(app.input.len());
                        let new_pos = crate::app::word_delete_start(&app.input[..pos]);
                        app.input.drain(new_pos..pos);
                        app.cursor_position = new_pos;
                        update_autocomplete(app);
                    }
                    _ => {}
                }
                return Ok(());
            }

            // ── Autocomplete interception ──────────────────────────────
            // When the autocomplete popup is visible, certain keys navigate
            // or accept the selection instead of their normal behaviour.
            if app.autocomplete_visible {
                match code {
                    KeyCode::Up => {
                        app.autocomplete_index = app.autocomplete_index.saturating_sub(1);
                        return Ok(());
                    }
                    KeyCode::Down => {
                        if app.autocomplete_index + 1 < app.autocomplete_items.len() {
                            app.autocomplete_index += 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Tab => {
                        accept_autocomplete(app);
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        // Accept the selection, then fall through to the
                        // normal Enter handler which will submit the message.
                        accept_autocomplete(app);
                    }
                    KeyCode::Esc => {
                        app.autocomplete_visible = false;
                        // Also switch to Normal mode (standard Esc behaviour).
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                    _ => {
                        // Fall through to normal handling; autocomplete will
                        // be refreshed after the keystroke is processed below.
                    }
                }
            }

            match code {
                KeyCode::Enter => {
                    if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT) {
                        // Insert newline for multi-line input
                        app.insert_char('\n');
                    } else {
                        // Submit message
                        if app.is_streaming {
                            return Ok(());
                        }
                        if let Some(text) = app.submit_input() {
                            app.autocomplete_visible = false;
                            submit_message(app, &text, provider).await;
                        }
                    }
                }
                KeyCode::Esc => {
                    app.autocomplete_visible = false;
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    app.delete_char();
                    update_autocomplete(app);
                }
                KeyCode::Left => app.move_cursor_left(),
                KeyCode::Right => app.move_cursor_right(),
                KeyCode::Tab => {
                    if app.input.starts_with('/') {
                        // Check if this is a file command with a path to complete
                        let parts: Vec<&str> = app.input.splitn(3, ' ').collect();
                        if parts.len() >= 2
                            && (parts[0] == "/file" || parts[0] == "/files" || parts[0] == "/cd")
                        {
                            let partial = parts.last().unwrap_or(&"");
                            if let Some(completed) = complete_file_path(partial) {
                                let prefix = if parts.len() == 3 {
                                    format!("{} {} ", parts[0], parts[1])
                                } else {
                                    format!("{} ", parts[0])
                                };
                                app.input = format!("{prefix}{completed}");
                                app.cursor_position = app.input.len();
                            } else {
                                // Show multiple matches in status bar if any exist
                                let file_matches = list_file_matches(partial);
                                if file_matches.len() > 1 {
                                    let display: Vec<String> =
                                        file_matches.iter().take(10).cloned().collect();
                                    let suffix = if file_matches.len() > 10 {
                                        format!(" (+{})", file_matches.len() - 10)
                                    } else {
                                        String::new()
                                    };
                                    app.set_status(format!("{}{}", display.join("  "), suffix));
                                }
                            }
                        } else {
                            // Existing slash command completion
                            let partial = &app.input[1..]; // strip the /
                            let commands = get_all_commands();
                            let matches: Vec<&str> = commands
                                .iter()
                                .filter(|(cmd, _)| cmd.starts_with(partial))
                                .map(|(cmd, _)| *cmd)
                                .collect();

                            if matches.len() == 1 {
                                app.input = format!("/{} ", matches[0]);
                                app.cursor_position = app.input.len();
                            } else if matches.len() > 1 {
                                let options = matches
                                    .iter()
                                    .map(|c| format!("/{c}"))
                                    .collect::<Vec<_>>()
                                    .join("  ");
                                app.status_message = Some(options);

                                let common = common_prefix(&matches);
                                if common.len() > partial.len() {
                                    app.input = format!("/{common}");
                                    app.cursor_position = app.input.len();
                                }
                            }
                        }
                    } else if app.input.contains('@') {
                        // Complete @file references
                        if let Some(at_pos) = app.input.rfind('@') {
                            let pos = app.cursor_position.min(app.input.len());
                            if at_pos < pos {
                                let partial = &app.input[at_pos + 1..pos];
                                if partial.contains('.') || partial.contains('/') {
                                    if let Some(completed) = complete_file_path(partial) {
                                        let before = app.input[..=at_pos].to_string();
                                        let after = app.input[pos..].to_string();
                                        app.input = format!("{before}{completed}{after}");
                                        app.cursor_position = at_pos + 1 + completed.len();
                                    } else {
                                        let file_matches = list_file_matches(partial);
                                        if file_matches.len() > 1 {
                                            let display: Vec<String> =
                                                file_matches.iter().take(10).cloned().collect();
                                            let suffix = if file_matches.len() > 10 {
                                                format!(" (+{})", file_matches.len() - 10)
                                            } else {
                                                String::new()
                                            };
                                            app.set_status(format!(
                                                "{}{}",
                                                display.join("  "),
                                                suffix
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                KeyCode::Up
                    // Browse input history (older)
                    if !app.input_history.is_empty() => {
                        match app.input_history_index {
                            None => {
                                // Save current input and go to most recent history
                                app.input_saved = app.input.clone();
                                app.input_history_index = Some(app.input_history.len() - 1);
                                if let Some(last) = app.input_history.last() {
                                    app.input = last.clone();
                                }
                                app.cursor_position = app.input.len();
                            }
                            Some(idx) if idx > 0 => {
                                app.input_history_index = Some(idx - 1);
                                app.input = app.input_history[idx - 1].clone();
                                app.cursor_position = app.input.len();
                            }
                            _ => {} // At oldest entry, do nothing
                        }
                    }
                KeyCode::Down => {
                    // Browse input history (newer)
                    if let Some(idx) = app.input_history_index {
                        if idx + 1 < app.input_history.len() {
                            app.input_history_index = Some(idx + 1);
                            app.input = app.input_history[idx + 1].clone();
                        } else {
                            // Back to current input
                            app.input_history_index = None;
                            app.input = app.input_saved.clone();
                        }
                        app.cursor_position = app.input.len();
                    }
                }
                KeyCode::Home => {
                    app.cursor_position = 0;
                }
                KeyCode::End => {
                    app.cursor_position = app.input.len();
                }
                KeyCode::Char(c) => {
                    app.insert_char(c);
                    update_autocomplete(app);
                }
                _ => {}
            }
        }
    }

    Ok(())
}
