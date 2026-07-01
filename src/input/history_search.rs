use crossterm::event::KeyCode;

use crate::app::{self, App, AppMode};
use crate::ui;
use crate::{history, models_for_provider};

// ── History browser ────────────────────────────────────────────────────────

pub(crate) fn handle_history_browser(app: &mut App, key: crossterm::event::KeyEvent) {
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

pub(crate) fn handle_search(app: &mut App, key: crossterm::event::KeyEvent) {
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
