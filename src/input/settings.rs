use crossterm::event::KeyCode;

use crate::app::{App, AppMode};
use crate::config::{self, Config};
use crate::ui;
use crate::{default_model_for_provider, models_for_provider};

// ── Settings ──────────────────────────────────────────────────────────────

pub(crate) fn handle_settings(app: &mut App, key: crossterm::event::KeyEvent) {
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
