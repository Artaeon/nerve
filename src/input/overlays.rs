use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, AppMode, InputMode};
use crate::config::{self, Config};
use crate::prompts;
use crate::ui;
use crate::{default_model_for_provider, models_for_provider};

// ── Command bar ─────────────────────────────────────────────────────────────

pub(crate) fn handle_command_bar(app: &mut App, key: crossterm::event::KeyEvent) {
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

pub(crate) fn handle_prompt_picker(app: &mut App, key: crossterm::event::KeyEvent) {
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

pub(crate) fn handle_model_select(app: &mut App, key: crossterm::event::KeyEvent) {
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

pub(crate) fn handle_provider_select(app: &mut App, key: crossterm::event::KeyEvent) {
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

pub(crate) fn handle_clipboard_manager(app: &mut App, key: crossterm::event::KeyEvent) {
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
