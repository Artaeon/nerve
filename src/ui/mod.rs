pub mod chat;
pub mod clipboard_manager;
pub mod command_bar;
pub mod help;
pub mod history_browser;
pub mod prompt_picker;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::app::{App, AppMode, InputMode};

// ─── Public entry point ──────────────────────────────────────────────────────

/// Main draw function — called once per frame by the event loop.
///
/// Delegates to sub-renderers based on the current `AppMode`.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    match app.mode {
        // Full-screen modes that replace the standard layout entirely.
        AppMode::PromptPicker => {
            prompt_picker::render_prompt_picker(frame, app, area);
            return;
        }
        AppMode::HistoryBrowser => {
            history_browser::render_history_browser(frame, app, area);
            return;
        }
        AppMode::Settings => {
            render_settings_placeholder(frame, area);
            return;
        }
        _ => {}
    }

    // ── Standard three-part layout: top bar | chat | bottom ─────────
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // top bar
            Constraint::Min(1),    // chat area
            Constraint::Length(5), // input + status
        ])
        .split(area);

    render_top_bar(frame, app, main_chunks[0]);
    chat::render_chat(frame, app, main_chunks[1]);
    render_bottom(frame, app, main_chunks[2]);

    // ── Overlays (drawn on top of the base layout) ──────────────────
    match app.mode {
        AppMode::CommandBar => command_bar::render_command_bar(frame, app),
        AppMode::Help => help::render_help(frame),
        AppMode::ModelSelect => render_model_selector(frame, app),
        AppMode::ProviderSelect => render_provider_selector(frame, app),
        AppMode::ClipboardManager => clipboard_manager::render_clipboard_manager(frame, app),
        _ => {}
    }
}

// ─── Top bar ─────────────────────────────────────────────────────────────────

fn render_top_bar(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate message count for the badge
    let msg_count = app.current_conversation().messages.len();
    let provider_label = provider_display_name(&app.selected_provider);
    let right_display = format!(
        "{} \u{203a} {} \u{2502} {} msgs ",
        provider_label,
        app.selected_model,
        msg_count
    );
    let right_len = right_display.len() as u16;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(15),        // branding + version
            Constraint::Length(3),          // separator
            Constraint::Min(1),            // conversation title
            Constraint::Length(right_len), // provider/model + msg count
        ])
        .split(inner);

    // Branding
    let brand = Paragraph::new(Line::from(vec![
        Span::styled(
            " Nerve ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " v0.1",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(brand, chunks[0]);

    // Separator
    let sep = Paragraph::new(Line::from(Span::styled(
        " \u{2502} ",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(sep, chunks[1]);

    // Conversation title
    let title = &app.current_conversation().title;
    let title_widget = Paragraph::new(Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(title_widget, chunks[2]);

    // Right side: provider > model | msg count
    let model_badge = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("{} ", provider_label),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(
            "\u{203a} ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            app.selected_model.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " \u{2502} ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{} msgs ", msg_count),
            Style::default().fg(Color::Cyan),
        ),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(model_badge, chunks[3]);
}

// ─── Bottom area (input + status) ────────────────────────────────────────────

fn render_bottom(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // input area
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_input(frame, app, chunks[0]);
    render_status_bar(frame, app, chunks[1]);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = match app.input_mode {
        InputMode::Normal => Span::styled(
            " NOR ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Insert => Span::styled(
            " INS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let border_color = match app.input_mode {
        InputMode::Insert => Color::Cyan,
        InputMode::Normal => Color::DarkGray,
    };

    let is_empty = app.input.is_empty();

    // Build the displayed text with a cursor indicator or placeholder.
    let input_line = if is_empty && app.input_mode == InputMode::Insert {
        // Show placeholder text when empty in insert mode
        Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(
                "Type your message... (Enter to send, Esc for normal mode)",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "\u{258c}",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ])
    } else if is_empty && app.input_mode == InputMode::Normal {
        // Hint in normal mode when empty
        Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(
                "Press i to start typing, / for Nerve Bar",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let before_cursor = &app.input[..app.cursor_position];
        let after_cursor = &app.input[app.cursor_position..];
        let cursor_char = if app.input_mode == InputMode::Insert {
            "\u{258c}" // ▌
        } else {
            ""
        };

        Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(before_cursor, Style::default().fg(Color::White)),
            Span::styled(
                cursor_char,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
            Span::styled(after_cursor, Style::default().fg(Color::White)),
        ])
    };

    // Word count
    let word_count = if is_empty {
        0
    } else {
        app.input.split_whitespace().count()
    };

    // Hint text for bottom line
    let hint = match app.input_mode {
        InputMode::Insert => "Enter: send | Esc: normal mode",
        InputMode::Normal => "i: insert | /: Nerve Bar | q: quit",
    };

    let title_line = Line::from(vec![
        Span::styled(
            " Message ",
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({} words) ", word_count),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let bottom_line = Line::from(vec![
        Span::styled(
            format!(" {} ", hint),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let input_block = Block::default()
        .title_top(title_line)
        .title_bottom(bottom_line)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));

    let input_widget = Paragraph::new(input_line)
        .block(input_block)
        .wrap(Wrap { trim: false });

    frame.render_widget(input_widget, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let provider_label = provider_display_name(&app.selected_provider);
    let sep = Span::styled(" \u{2502} ", Style::default().fg(Color::Rgb(60, 60, 70)));

    if app.is_streaming {
        // Streaming status bar with progress animation
        // Cycle through animation frames based on streaming response length
        let anim_chars = ['\u{2591}', '\u{2592}', '\u{2593}', '\u{2588}'];
        let tick = app.streaming_response.len() % 4;
        let progress: String = (0..8)
            .map(|i| anim_chars[(tick + i) % 4])
            .collect();

        let line = Line::from(vec![
            Span::styled(
                " Streaming... ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                progress,
                Style::default().fg(Color::Green),
            ),
            sep.clone(),
            Span::styled(
                format!("{} \u{203a} {}", provider_label, app.selected_model),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(
                format!(
                    "Conv {}/{}",
                    app.active_conversation + 1,
                    app.conversations.len(),
                ),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" "),
        ]);

        frame.render_widget(Paragraph::new(line), area);
    } else {
        // Normal status bar
        let left_status = if let Some(ref msg) = app.status_message {
            Span::styled(format!(" {msg}"), Style::default().fg(Color::Yellow))
        } else {
            Span::styled(
                " Ready",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        };

        let right_text = format!(
            "Conv {}/{} \u{2502} Ctrl+K: Nerve Bar \u{2502} F1: Help ",
            app.active_conversation + 1,
            app.conversations.len(),
        );
        let right_span = Span::styled(
            right_text.clone(),
            Style::default().fg(Color::DarkGray),
        );
        let right_width = right_text.len() as u16;

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(right_width)])
            .split(area);

        let left_line = Line::from(vec![
            left_status,
            sep.clone(),
            Span::styled(
                format!("Provider: {}", provider_label),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(
                format!("Model: {}", app.selected_model),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(Paragraph::new(left_line), chunks[0]);
        frame.render_widget(
            Paragraph::new(Line::from(right_span)).alignment(Alignment::Right),
            chunks[1],
        );
    }
}

// ─── Model selector overlay ──────────────────────────────────────────────────

fn render_model_selector(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let popup_width = 44u16.min(area.width.saturating_sub(4));
    let popup_height = (app.available_models.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Select Model ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let items: Vec<ListItem<'_>> = app
        .available_models
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let is_selected = i == app.model_select_index;
            let is_active = *model == app.selected_model;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_active { " * " } else { "   " };
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{model}"),
                style,
            )))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    // Use ListState for automatic scroll tracking.
    let mut state = ListState::default();
    state.select(Some(app.model_select_index));

    frame.render_stateful_widget(list, popup_area, &mut state);
}

// ─── Provider selector overlay ───────────────────────────────────────────────

/// Human-friendly display name for a provider key.
fn provider_display_name(key: &str) -> &'static str {
    match key {
        "claude_code" | "claude" => "Claude Code",
        "ollama" => "Ollama",
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        _ => "Custom",
    }
}

/// Short description for the provider selector overlay.
fn provider_description(key: &str) -> &'static str {
    match key {
        "claude_code" | "claude" => "subscription, no API key",
        "ollama" => "local, no API key",
        "openai" => "requires API key",
        "openrouter" => "requires API key",
        _ => "custom provider",
    }
}

fn render_provider_selector(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = (app.available_providers.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Select Provider ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .title_bottom(
            Line::from(Span::styled(
                " Enter: Select | Esc: Cancel ",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let items: Vec<ListItem<'_>> = app
        .available_providers
        .iter()
        .enumerate()
        .map(|(i, provider_key)| {
            let is_selected = i == app.provider_select_index;
            let is_active = *provider_key == app.selected_provider;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_active { " * " } else { "   " };
            let name = provider_display_name(provider_key);
            let desc = provider_description(provider_key);
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{name} ({desc})"),
                style,
            )))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(app.provider_select_index));

    frame.render_stateful_widget(list, popup_area, &mut state);
}

// ─── Settings placeholder ────────────────────────────────────────────────────

fn render_settings_placeholder(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Settings ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::new(2, 2, 1, 1));

    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Settings panel coming soon.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc to go back.",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(block)
    .wrap(Wrap { trim: false });

    frame.render_widget(text, area);
}
