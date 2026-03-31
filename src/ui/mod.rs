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

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(14),  // branding
            Constraint::Length(3),   // separator
            Constraint::Min(1),     // conversation title
            Constraint::Length(30), // model badge
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
        " | ",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(sep, chunks[1]);

    // Conversation title
    let title = &app.current_conversation().title;
    let title_widget = Paragraph::new(Line::from(Span::styled(
        format!("{title}"),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(title_widget, chunks[2]);

    // Model badge with inferred provider
    let provider_hint = infer_provider(&app.selected_model);
    let model_badge = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("{} ", provider_hint),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            &app.selected_model,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(model_badge, chunks[3]);
}

/// Infer a provider label from the model name for display purposes.
fn infer_provider(model: &str) -> &'static str {
    if model.starts_with("gpt") {
        "OpenAI"
    } else if model.starts_with("claude") {
        "Anthropic"
    } else if model.starts_with("llama") || model.starts_with("mistral") || model.starts_with("codellama") {
        "Ollama"
    } else if model.starts_with("gemini") {
        "Google"
    } else {
        "model:"
    }
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

    // Build the displayed text with a cursor indicator.
    let before_cursor = &app.input[..app.cursor_position];
    let after_cursor = &app.input[app.cursor_position..];
    let cursor_char = if app.input_mode == InputMode::Insert {
        "\u{258c}" // ▌
    } else {
        ""
    };

    let input_line = Line::from(vec![
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
    ]);

    // Hint text and character count for the title bar
    let hint = match app.input_mode {
        InputMode::Insert => "Enter: send | Esc: normal mode",
        InputMode::Normal => "i: insert | Ctrl+K: Nerve Bar",
    };
    let char_count = app.input.chars().count();
    let title_line = Line::from(vec![
        Span::styled(" Message ", Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("({} chars) ", char_count),
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
    let left_text = if let Some(ref msg) = app.status_message {
        Span::styled(format!(" {msg}"), Style::default().fg(Color::Yellow))
    } else if app.is_streaming {
        Span::styled(
            " Streaming...",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::SLOW_BLINK),
        )
    } else {
        Span::styled(" Ready", Style::default().fg(Color::DarkGray))
    };

    let provider_hint = infer_provider(&app.selected_model);
    let right_text = Span::styled(
        format!(
            "Conv {}/{} | {} {} | Ctrl+K: Nerve Bar | Ctrl+H: Help ",
            app.active_conversation + 1,
            app.conversations.len(),
            provider_hint,
            app.selected_model,
        ),
        Style::default().fg(Color::DarkGray),
    );

    let right_width = right_text.width() as u16;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(right_width)])
        .split(area);

    frame.render_widget(Paragraph::new(Line::from(left_text)), chunks[0]);
    frame.render_widget(
        Paragraph::new(Line::from(right_text)).alignment(Alignment::Right),
        chunks[1],
    );
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
