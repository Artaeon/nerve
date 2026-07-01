use std::sync::Arc;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::ai::provider::AiProvider;
use crate::app::{self, App, AppMode, InputMode};
use crate::clipboard_manager::ClipboardSource;
use crate::config::{self, Config};
use crate::{
    accept_autocomplete, clear_conversation, common_prefix, complete_file_path,
    copy_last_assistant_message, cycle_conversation, cycle_conversation_back,
    default_model_for_provider, delete_last_exchange, edit_last_message, get_all_commands,
    list_file_matches, models_for_provider, regenerate_response, submit_message,
    update_autocomplete,
};
use crate::{clipboard, history, prompts, ui};

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
        AppMode::CommandBar => handle_command_bar(app, key),
        AppMode::PromptPicker => handle_prompt_picker(app, key),
        AppMode::ModelSelect => handle_model_select(app, key),
        AppMode::ProviderSelect => handle_provider_select(app, key),
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
        AppMode::Settings => handle_settings(app, key),
        AppMode::ClipboardManager => handle_clipboard_manager(app, key),
        AppMode::HistoryBrowser => handle_history_browser(app, key),
        AppMode::SearchOverlay => handle_search(app, key),
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

// ── Command bar ─────────────────────────────────────────────────────────────

fn handle_command_bar(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Use the helper from the UI module to get the selected prompt.
            if let Some(prompt) = ui::command_bar::selected_prompt(app) {
                if prompt.template.starts_with("@action:") {
                    // Quick action — perform immediately.
                    match prompt.template.as_str() {
                        "@action:settings" => {
                            app.mode = AppMode::Settings;
                            app.set_status("Opened settings");
                            return;
                        }
                        "@action:theme" => {
                            let presets = config::theme_presets();
                            app.theme_index = (app.theme_index + 1) % presets.len();
                            if let Some((name, theme)) = presets.get(app.theme_index) {
                                let mut cfg = Config::load().unwrap_or_default();
                                cfg.theme = theme.clone();
                                let _ = cfg.save();
                                app.set_status(format!("Theme: {name}"));
                            }
                        }
                        "@action:agent_toggle" => {
                            app.agent_mode = !app.agent_mode;
                            let state = if app.agent_mode { "ON" } else { "OFF" };
                            app.set_status(format!("Agent mode: {state}"));
                        }
                        "@action:code_toggle" => {
                            app.code_mode = !app.code_mode;
                            let state = if app.code_mode { "ON" } else { "OFF" };
                            app.set_status(format!("Code mode: {state}"));
                        }
                        "@action:help" => {
                            app.mode = AppMode::Help;
                            app.help_scroll = 0;
                            app.set_status("Opened help");
                            return;
                        }
                        "@action:history" => {
                            app.mode = AppMode::HistoryBrowser;
                            app.set_status("Opened history browser");
                            return;
                        }
                        "@action:clipboard" => {
                            app.mode = AppMode::ClipboardManager;
                            app.set_status("Opened clipboard manager");
                            return;
                        }
                        _ => {}
                    }
                } else if prompt.template.starts_with('/') {
                    // Slash command — queue it for immediate execution.
                    app.pending_command = Some(prompt.template.clone());
                    app.set_status(format!("Running: {}", prompt.name));
                } else {
                    // SmartPrompt — load the template into the input field.
                    let template = if app.input.is_empty() {
                        prompt.template.replace("{{input}}", "")
                    } else {
                        prompt.template.replace("{{input}}", &app.input)
                    };
                    app.input = template;
                    app.cursor_position = app.input.len();
                    app.input_mode = InputMode::Insert;
                    app.set_status(format!("Loaded prompt: {}", prompt.name));
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            let cat_count = ui::command_bar::category_tabs().len();
            if cat_count > 0 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    app.command_bar_category = if app.command_bar_category == 0 {
                        cat_count - 1
                    } else {
                        app.command_bar_category - 1
                    };
                } else {
                    app.command_bar_category = (app.command_bar_category + 1) % cat_count;
                }
                app.command_bar_select_index = 0;
            }
        }
        KeyCode::BackTab => {
            // Shift+Tab also reported as BackTab on some terminals.
            let cat_count = ui::command_bar::category_tabs().len();
            app.command_bar_category = if app.command_bar_category == 0 {
                cat_count - 1
            } else {
                app.command_bar_category - 1
            };
            app.command_bar_select_index = 0;
        }
        KeyCode::Backspace => {
            app.command_bar_input.pop();
            app.command_bar_select_index = 0;
        }
        KeyCode::Up => {
            app.command_bar_select_index = app.command_bar_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::command_bar::matched_prompt_count(app);
            if app.command_bar_select_index + 1 < count {
                app.command_bar_select_index += 1;
            }
        }
        KeyCode::Char(c) => {
            app.command_bar_input.push(c);
            app.command_bar_select_index = 0;
        }
        _ => {}
    }
}

// ── Prompt picker ───────────────────────────────────────────────────────────

fn handle_prompt_picker(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            app.prompt_focus_right = !app.prompt_focus_right;
            if app.prompt_focus_right {
                app.prompt_select_index = 0;
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.prompt_focus_right {
                let count = ui::prompt_picker::visible_prompt_count(app);
                if app.prompt_select_index + 1 < count {
                    app.prompt_select_index += 1;
                }
            } else {
                let cat_count = prompts::categories().len();
                if app.prompt_category_index + 1 < cat_count {
                    app.prompt_category_index += 1;
                    app.prompt_select_index = 0;
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.prompt_focus_right {
                app.prompt_select_index = app.prompt_select_index.saturating_sub(1);
            } else if app.prompt_category_index > 0 {
                app.prompt_category_index -= 1;
                app.prompt_select_index = 0;
            }
        }
        KeyCode::Enter => {
            let all = prompts::all_prompts();
            let cats = prompts::categories();
            let selected_cat = cats
                .get(app.prompt_category_index)
                .cloned()
                .unwrap_or_default();
            let filtered: Vec<&prompts::SmartPrompt> =
                all.iter().filter(|p| p.category == selected_cat).collect();

            if let Some(prompt) = filtered.get(app.prompt_select_index) {
                let template = if app.input.is_empty() {
                    prompt.template.replace("{{input}}", "")
                } else {
                    prompt.template.replace("{{input}}", &app.input)
                };
                app.input = template;
                app.cursor_position = app.input.len();
                app.input_mode = InputMode::Insert;
                app.set_status(format!("Loaded prompt: {}", prompt.name));
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(c) => {
            // Characters typed while in the prompt picker go to the filter.
            app.prompt_filter.push(c);
            app.prompt_select_index = 0;
        }
        KeyCode::Backspace => {
            app.prompt_filter.pop();
            app.prompt_select_index = 0;
        }
        _ => {}
    }
}

// ── Model selection ─────────────────────────────────────────────────────────

fn handle_model_select(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('j') | KeyCode::Down
            if app.model_select_index + 1 < app.available_models.len() =>
        {
            app.model_select_index += 1;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.model_select_index = app.model_select_index.saturating_sub(1);
        }
        KeyCode::Enter => {
            if let Some(model) = app.available_models.get(app.model_select_index) {
                app.selected_model = model.clone();
                app.set_status(format!("Model set to {model}"));
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Provider selection ─────────────────────────────────────────────────────

fn handle_provider_select(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up | KeyCode::Char('k') => {
            app.provider_select_index = app.provider_select_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if app.provider_select_index + 1 < app.available_providers.len() =>
        {
            app.provider_select_index += 1;
        }
        KeyCode::Enter => {
            if let Some(provider_name) = app.available_providers.get(app.provider_select_index) {
                app.selected_provider = provider_name.clone();
                app.provider_changed = true;
                app.selected_model = default_model_for_provider(&app.selected_provider).into();
                app.available_models = models_for_provider(&app.selected_provider);
                // Show available models in status
                let model_list = app.available_models.join(", ");
                app.set_status(format!(
                    "Switched to {} | Model: {} | Available: {}",
                    provider_name, app.selected_model, model_list
                ));
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Clipboard manager ──────────────────────────────────────────────────────

fn handle_clipboard_manager(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let filtered = app.clipboard_manager.search(&app.clipboard_search);
            if let Some(&(original_idx, _)) = filtered.get(app.clipboard_select_index) {
                match app.clipboard_manager.copy_to_system(original_idx) {
                    Ok(()) => {
                        app.set_status("Copied to clipboard");
                    }
                    Err(e) => {
                        app.set_status(format!("Clipboard error: {e}"));
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('d') if app.clipboard_search.is_empty() => {
            let filtered = app.clipboard_manager.search(&app.clipboard_search);
            if let Some(&(original_idx, _)) = filtered.get(app.clipboard_select_index) {
                app.clipboard_manager.remove(original_idx);
                let new_count = app.clipboard_manager.search(&app.clipboard_search).len();
                if app.clipboard_select_index >= new_count && new_count > 0 {
                    app.clipboard_select_index = new_count - 1;
                } else if new_count == 0 {
                    app.clipboard_select_index = 0;
                }
                let _ = app.clipboard_manager.save();
            }
        }
        KeyCode::Up => {
            app.clipboard_select_index = app.clipboard_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::clipboard_manager::matched_entry_count(app);
            if app.clipboard_select_index + 1 < count {
                app.clipboard_select_index += 1;
            }
        }
        KeyCode::Backspace => {
            app.clipboard_search.pop();
            app.clipboard_select_index = 0;
        }
        KeyCode::Char(c) => {
            app.clipboard_search.push(c);
            app.clipboard_select_index = 0;
        }
        _ => {}
    }
}

// ── History browser ────────────────────────────────────────────────────────

fn handle_history_browser(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
            } else {
                app.mode = AppMode::Normal;
            }
        }
        KeyCode::Enter => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
                return;
            }
            let filtered = filtered_history_entries(app);
            if let Some(record) = filtered.get(app.history_select_index).cloned() {
                let conv = app::Conversation {
                    id: record.id.clone(),
                    title: record.title.clone(),
                    messages: record
                        .messages
                        .iter()
                        .map(|m| (m.role.clone(), m.content.clone()))
                        .collect(),
                    created_at: record.created_at,
                };
                // Abort any in-flight stream before switching conversations,
                // else its tokens keep buffering into an orphaned receiver and
                // the spawned task leaks (mirrors new_conversation()).
                app.cancel_active_stream();
                app.conversations.push(conv);
                app.active_conversation = app.conversations.len() - 1;
                app.scroll_offset = 0;
                app.streaming_response.clear();
                app.is_streaming = false;
                app.stream_rx = None;
                app.streaming_start = None;

                // Restore the model and provider from the history record.
                if !record.model.is_empty() {
                    app.selected_model = record.model.clone();
                }
                if !record.provider.is_empty() {
                    app.selected_provider = record.provider.clone();
                    app.provider_changed = true;
                    app.available_models = models_for_provider(&app.selected_provider);
                }
                app.set_status(format!(
                    "Loaded: {} ({} > {})",
                    record.title, record.provider, record.model
                ));
                app.mode = AppMode::Normal;
            }
        }
        KeyCode::Char('d') if app.history_search.is_empty() => {
            if app.history_delete_pending {
                // Confirmed — delete
                let filtered = filtered_history_entries(app);
                if let Some(record) = filtered.get(app.history_select_index).cloned() {
                    let _ = history::delete_conversation(&record.id);
                    app.history_entries.retain(|r| r.id != record.id);
                    let new_count = filtered_history_entries(app).len();
                    if app.history_select_index >= new_count && new_count > 0 {
                        app.history_select_index = new_count - 1;
                    } else if new_count == 0 {
                        app.history_select_index = 0;
                    }
                    app.set_status("Conversation deleted");
                }
                app.history_delete_pending = false;
            } else {
                app.history_delete_pending = true;
                app.set_status("Press 'd' again to confirm deletion, any other key to cancel");
            }
        }
        KeyCode::Char('s') if app.history_search.is_empty() => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
                return;
            }
            app.history_sort = (app.history_sort + 1) % 3;
            let sort_name = match app.history_sort {
                0 => "Date (newest first)",
                1 => "Title (A-Z)",
                2 => "Messages (most first)",
                _ => "Date",
            };
            app.set_status(format!("Sort: {sort_name}"));
        }
        KeyCode::Up | KeyCode::Char('k') if app.history_search.is_empty() => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
            }
            app.history_select_index = app.history_select_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') if app.history_search.is_empty() => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
            }
            let count = ui::history_browser::filtered_history_count(app);
            if app.history_select_index + 1 < count {
                app.history_select_index += 1;
            }
        }
        KeyCode::Up => {
            app.history_select_index = app.history_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::history_browser::filtered_history_count(app);
            if app.history_select_index + 1 < count {
                app.history_select_index += 1;
            }
        }
        KeyCode::Backspace => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
            }
            app.history_search.pop();
            app.history_select_index = 0;
        }
        KeyCode::Char(c) => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
                return;
            }
            app.history_search.push(c);
            app.history_select_index = 0;
        }
        _ => {
            if app.history_delete_pending {
                app.history_delete_pending = false;
                app.set_status("Deletion cancelled");
            }
        }
    }
}

pub(crate) fn filtered_history_entries(app: &App) -> Vec<history::ConversationRecord> {
    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::SkimMatcherV2;
    let matcher = SkimMatcherV2::default();
    let query = &app.history_search;
    let mut entries: Vec<history::ConversationRecord> = app
        .history_entries
        .iter()
        .filter(|record| {
            if query.is_empty() {
                return true;
            }
            if matcher.fuzzy_match(&record.title, query).is_some() {
                return true;
            }
            for msg in &record.messages {
                if matcher.fuzzy_match(&msg.content, query).is_some() {
                    return true;
                }
            }
            false
        })
        .cloned()
        .collect();

    // Apply sort order to match the rendering
    match app.history_sort {
        1 => entries.sort_by_key(|a| a.title.to_lowercase()),
        2 => entries.sort_by_key(|c| std::cmp::Reverse(c.messages.len())),
        _ => {} // Already sorted by date (default from list_conversations)
    }

    entries
}

// ── Search overlay ─────────────────────────────────────────────────────────

fn handle_search(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter
            // Jump to next match
            if !app.search_results.is_empty() => {
                app.search_current = (app.search_current + 1) % app.search_results.len();
                app.set_status(format!(
                    "Match {}/{}",
                    app.search_current + 1,
                    app.search_results.len()
                ));
            }
        KeyCode::Backspace => {
            app.search_query.pop();
            update_search_results(app);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            update_search_results(app);
        }
        _ => {}
    }
}

pub(crate) fn update_search_results(app: &mut App) {
    let query = app.search_query.to_lowercase();
    if query.is_empty() {
        app.search_results.clear();
        return;
    }
    app.search_results = app
        .current_conversation()
        .messages
        .iter()
        .enumerate()
        .filter(|(_, (_, content))| content.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
    app.search_current = 0;
}

// ── Settings ──────────────────────────────────────────────────────────────

fn handle_settings(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            // Save config and close
            let mut cfg = Config::load().unwrap_or_default();
            cfg.default_provider = app.selected_provider.clone();
            cfg.default_model = app.selected_model.clone();
            // Apply theme from selected preset
            let presets = config::theme_presets();
            if let Some((_, theme)) = presets.get(app.theme_index) {
                cfg.theme = theme.clone();
            }
            // Persist git author settings
            cfg.git_user_name = if app.git_user_name.is_empty() {
                None
            } else {
                Some(app.git_user_name.clone())
            };
            cfg.git_user_email = if app.git_user_email.is_empty() {
                None
            } else {
                Some(app.git_user_email.clone())
            };
            let _ = cfg.save();
            app.mode = AppMode::Normal;
            app.set_status("Settings saved");
        }
        KeyCode::Tab => {
            app.settings_tab = (app.settings_tab + 1) % 5;
            app.settings_select = 0;
        }
        KeyCode::BackTab => {
            app.settings_tab = if app.settings_tab == 0 {
                4
            } else {
                app.settings_tab - 1
            };
            app.settings_select = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = settings_item_count(app.settings_tab);
            if app.settings_select + 1 < max {
                app.settings_select += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings_select = app.settings_select.saturating_sub(1);
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            toggle_setting(app);
        }
        _ => {}
    }
}

fn settings_item_count(tab: usize) -> usize {
    match tab {
        0 => ui::settings::general_item_count(),
        1 => ui::settings::providers_item_count(),
        2 => ui::settings::theme_item_count(),
        3 => ui::settings::keybinds_item_count(),
        4 => ui::settings::git_item_count(),
        _ => 0,
    }
}

fn toggle_setting(app: &mut App) {
    match app.settings_tab {
        0 => {
            // General tab
            match app.settings_select {
                0 => {
                    // Provider: cycle
                    let providers = &app.available_providers;
                    let idx = providers
                        .iter()
                        .position(|p| p == &app.selected_provider)
                        .unwrap_or(0);
                    app.selected_provider = providers[(idx + 1) % providers.len()].clone();
                    app.provider_changed = true;
                    app.selected_model = default_model_for_provider(&app.selected_provider).into();
                    app.available_models = models_for_provider(&app.selected_provider);
                }
                1 => {
                    // Model: cycle
                    let idx = app
                        .available_models
                        .iter()
                        .position(|m| m == &app.selected_model)
                        .unwrap_or(0);
                    app.selected_model =
                        app.available_models[(idx + 1) % app.available_models.len()].clone();
                }
                2 => app.agent_mode = !app.agent_mode,
                3 => app.code_mode = !app.code_mode,
                4 => app.spending_limit.enabled = !app.spending_limit.enabled,
                _ => {}
            }
        }
        2
            // Theme tab: only the preset selector (item 0) cycles
            if app.settings_select == 0 => {
                let presets = config::theme_presets();
                app.theme_index = (app.theme_index + 1) % presets.len();
            }
        _ => {}
    }
}
