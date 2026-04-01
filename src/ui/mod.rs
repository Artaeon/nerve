pub mod chat;
pub mod clipboard_manager;
pub mod command_bar;
pub mod help;
pub mod history_browser;
pub mod prompt_picker;
pub mod search;
pub mod settings;

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
            settings::render_settings(frame, app, area);
            return;
        }
        _ => {}
    }

    // ── Standard three-part layout: top bar | chat | bottom ─────────

    // Calculate how many lines the input text needs so the box grows dynamically.
    let input_width = area.width.saturating_sub(10) as usize; // account for borders, padding, mode indicator
    let input_lines = if app.input.is_empty() {
        1
    } else {
        // Count wrapped lines
        let mut lines = 0;
        for line in app.input.lines() {
            lines += 1 + line.len() / input_width.max(1);
        }
        lines.max(1)
    };
    // Clamp: minimum 3 (1 line + borders), maximum leaves room for top bar + chat
    let max_input = area.height.saturating_sub(8).max(3); // leave room for top bar + chat area
    let input_height = (input_lines as u16 + 2) // +2 for borders
        .clamp(3, max_input);
    let bottom_height = input_height + 1; // +1 for status bar

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),            // top bar
            Constraint::Min(1),              // chat area
            Constraint::Length(bottom_height), // input + status (DYNAMIC)
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
        AppMode::SearchOverlay => search::render_search(frame, app),
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

    // Build the conversation tab indicator
    let conv_count = app.conversations.len();
    let conv_num = app.active_conversation + 1;
    let conv_indicator = if conv_count > 1 {
        format!(
            "\u{25c4} {}/{} \u{25ba} ",
            conv_num, conv_count
        )
    } else {
        String::new()
    };

    // Mode badges (shown next to branding when active)
    let mut badge_spans: Vec<Span> = vec![];
    if app.agent_mode {
        badge_spans.push(Span::styled(
            " AGENT ",
            Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    }
    if app.code_mode {
        badge_spans.push(Span::styled(
            " CODE ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    let badge_width: u16 = badge_spans.iter().map(|s| s.width() as u16).sum();
    let brand_width = 15 + badge_width;

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
            Constraint::Length(brand_width), // branding + version + badges
            Constraint::Length(3),          // separator
            Constraint::Min(1),            // conversation indicator + title
            Constraint::Length(right_len), // provider/model + msg count
        ])
        .split(inner);

    // Branding + mode badges
    let mut brand_spans = vec![
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
    ];
    if !badge_spans.is_empty() {
        brand_spans.push(Span::raw(" "));
        brand_spans.extend(badge_spans);
    }
    let brand = Paragraph::new(Line::from(brand_spans));
    frame.render_widget(brand, chunks[0]);

    // Separator
    let sep = Paragraph::new(Line::from(Span::styled(
        " \u{2502} ",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(sep, chunks[1]);

    // Conversation indicator + title (truncated if too long for available space)
    let title = &app.current_conversation().title;
    let max_title_len = chunks[2].width.saturating_sub(conv_indicator.len() as u16) as usize;
    let display_title = if title.len() > max_title_len {
        format!("{}...", &title[..max_title_len.saturating_sub(3)])
    } else {
        title.to_string()
    };
    let mut title_spans = Vec::new();
    if !conv_indicator.is_empty() {
        title_spans.push(Span::styled(
            conv_indicator,
            Style::default().fg(Color::DarkGray),
        ));
    }
    title_spans.push(Span::styled(
        display_title,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    let title_widget = Paragraph::new(Line::from(title_spans));
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
    // For multi-line input we build a Vec<Line> so that explicit newlines are honoured.
    let input_lines: Vec<Line<'_>> = if is_empty && app.input_mode == InputMode::Insert {
        // Show placeholder text when empty in insert mode
        vec![Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(
                "Type your message... (Enter to send, Shift+Enter for newline)",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "\u{258c}",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ])]
    } else if is_empty && app.input_mode == InputMode::Normal {
        // Hint in normal mode when empty
        vec![Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(
                "Press i to start typing, / for Nerve Bar",
                Style::default().fg(Color::DarkGray),
            ),
        ])]
    } else {
        let pos = app.cursor_position.min(app.input.len());
        // Walk back to find a valid char boundary if needed.
        let pos = (0..=pos).rev().find(|&i| app.input.is_char_boundary(i)).unwrap_or(0);
        let before_cursor = &app.input[..pos];
        let after_cursor = &app.input[pos..];
        let cursor_char = if app.input_mode == InputMode::Insert {
            "\u{258c}" // ▌
        } else {
            ""
        };

        // Split the input by newlines and build a Line per logical line.
        // The cursor sits at the boundary between before_cursor and after_cursor,
        // which may themselves span multiple lines.
        let before_lines: Vec<&str> = before_cursor.split('\n').collect();
        let after_lines: Vec<&str> = after_cursor.split('\n').collect();

        let mut result: Vec<Line<'_>> = Vec::new();

        // Lines entirely before the cursor line
        for (i, line) in before_lines.iter().enumerate() {
            if i == 0 {
                // First line gets the mode indicator prefix
                if i == before_lines.len() - 1 {
                    // Cursor is on the first line — combine with after_cursor
                    let first_after = after_lines.first().copied().unwrap_or("");
                    let spans = vec![
                        mode_indicator.clone(),
                        Span::raw(" "),
                        Span::styled(line.to_string(), Style::default().fg(Color::White)),
                        Span::styled(
                            cursor_char.to_string(),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::SLOW_BLINK),
                        ),
                        Span::styled(first_after.to_string(), Style::default().fg(Color::White)),
                    ];
                    result.push(Line::from(spans));
                    // Remaining after-cursor lines
                    for after_line in after_lines.iter().skip(1) {
                        result.push(Line::from(Span::styled(
                            after_line.to_string(),
                            Style::default().fg(Color::White),
                        )));
                    }
                } else {
                    // First line, but cursor is on a later line
                    result.push(Line::from(vec![
                        mode_indicator.clone(),
                        Span::raw(" "),
                        Span::styled(line.to_string(), Style::default().fg(Color::White)),
                    ]));
                }
            } else if i < before_lines.len() - 1 {
                // Middle lines before cursor line
                result.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White),
                )));
            } else {
                // Last before-cursor line — this is where the cursor sits
                let first_after = after_lines.first().copied().unwrap_or("");
                result.push(Line::from(vec![
                    Span::styled(line.to_string(), Style::default().fg(Color::White)),
                    Span::styled(
                        cursor_char.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::SLOW_BLINK),
                    ),
                    Span::styled(first_after.to_string(), Style::default().fg(Color::White)),
                ]));
                // Remaining after-cursor lines
                for after_line in after_lines.iter().skip(1) {
                    result.push(Line::from(Span::styled(
                        after_line.to_string(),
                        Style::default().fg(Color::White),
                    )));
                }
            }
        }

        result
    };

    // Calculate scroll offset for the input widget when text exceeds visible area.
    // We want to keep the cursor line visible.
    let visible_input_height = area.height.saturating_sub(2); // subtract borders
    let scroll_pos = app.cursor_position.min(app.input.len());
    let scroll_pos = (0..=scroll_pos).rev().find(|&i| app.input.is_char_boundary(i)).unwrap_or(0);
    let cursor_line = app.input[..scroll_pos]
        .chars()
        .filter(|c| *c == '\n')
        .count() as u16;
    let input_scroll = if cursor_line >= visible_input_height {
        cursor_line - visible_input_height + 1
    } else {
        0
    };

    // Word count
    let word_count = if is_empty {
        0
    } else {
        app.input.split_whitespace().count()
    };

    // Hint text for bottom line
    let hint = match app.input_mode {
        InputMode::Insert => "Enter: send | Shift+Enter: newline | Esc: normal mode",
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

    let input_widget = Paragraph::new(input_lines)
        .block(input_block)
        .wrap(Wrap { trim: false })
        .scroll((input_scroll, 0));

    frame.render_widget(input_widget, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let provider_label = provider_display_name(&app.selected_provider);
    let sep = Span::styled(" \u{2502} ", Style::default().fg(Color::Rgb(60, 60, 70)));

    if app.is_streaming {
        // Streaming status bar with progress animation and stats
        let anim_chars = ['\u{2591}', '\u{2592}', '\u{2593}', '\u{2588}'];
        let tick = app.streaming_response.len() % 4;
        let progress: String = (0..6)
            .map(|i| anim_chars[(tick + i) % 4])
            .collect();

        // Approximate token count: words * 4/3
        let word_count = app.streaming_response.split_whitespace().count();
        let approx_tokens = word_count * 4 / 3;

        // Elapsed time and speed
        let elapsed_secs = app
            .streaming_start
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let tok_per_sec = if elapsed_secs > 0.1 {
            approx_tokens as f64 / elapsed_secs
        } else {
            0.0
        };

        let mut spans = vec![];

        // Agent iteration badge (shown before streaming indicator when active)
        if app.agent_mode && app.agent_iterations > 0 {
            spans.push(Span::styled(
                format!(" AGENT {}/10 ", app.agent_iterations),
                Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));
            spans.push(sep.clone());
        }

        spans.extend_from_slice(&[
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
                format!("~{} tokens", approx_tokens),
                Style::default().fg(Color::Cyan),
            ),
            sep.clone(),
            Span::styled(
                format!("{:.1}s", elapsed_secs),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(
                format!("{:.0} tok/s", tok_per_sec),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(
                app.selected_model.to_string(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
        ]);

        if app.code_mode {
            spans.insert(spans.len() - 1, sep.clone());
            spans.insert(
                spans.len() - 1,
                Span::styled(
                    "CODE",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    } else {
        // Normal status bar with conversation stats
        // Color-code status messages: red for errors/warnings, green for success, yellow for info.
        let left_status = if let Some(ref msg) = app.status_message {
            let msg_style = if msg.starts_with("Error") || msg.starts_with("Blocked") || msg.starts_with("Failed") {
                Style::default().fg(Color::Red)
            } else if msg.starts_with("Saved") || msg.starts_with("Copied") || msg.starts_with("Exported") || msg.contains("success") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            };
            Span::styled(format!(" {msg}"), msg_style)
        } else {
            Span::styled(
                " Ready",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        };

        // Calculate total word count and token estimate across all messages in current conversation
        let total_words: usize = app
            .current_conversation()
            .messages
            .iter()
            .map(|(_, content)| content.split_whitespace().count())
            .sum();
        let total_tokens: usize = app
            .current_conversation()
            .messages
            .iter()
            .map(|(_, content)| content.len() / 4 + 1)
            .sum();

        // Format word count with thousands separator
        let words_display = if total_words >= 1_000 {
            format!("{},{:03}", total_words / 1_000, total_words % 1_000)
        } else {
            format!("{}", total_words)
        };

        // Format token count with thousands separator
        let tokens_display = if total_tokens >= 1_000 {
            format!("{},{:03}", total_tokens / 1_000, total_tokens % 1_000)
        } else {
            format!("{}", total_tokens)
        };

        let right_text = format!(
            "Conv {}/{} \u{2502} Ctrl+K: Nerve Bar ",
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

        let mut left_spans = vec![left_status];

        // Agent iteration badge (shown when agent is active with iterations)
        if app.agent_mode && app.agent_iterations > 0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!(" AGENT {}/10 ", app.agent_iterations),
                Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));
        }

        // Show scroll position when user has scrolled up from the bottom
        if app.scroll_offset > 0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("{} lines above bottom", app.scroll_offset),
                Style::default().fg(Color::Cyan),
            ));
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                "j/k to scroll",
                Style::default().fg(Color::DarkGray),
            ));
        }

        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            format!("~{} tokens", tokens_display),
            Style::default().fg(Color::DarkGray),
        ));
        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            format!("{} words", words_display),
            Style::default().fg(Color::DarkGray),
        ));
        left_spans.push(sep.clone());
        left_spans.push(Span::styled(
            format!("{} \u{203a} {}", provider_label, app.selected_model),
            Style::default().fg(Color::DarkGray),
        ));

        // Show estimated cost badge for paid providers.
        if app.usage_stats.estimated_cost_usd > 0.0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("{} (est.)", app.usage_stats.format_cost()),
                Style::default().fg(Color::Yellow),
            ));
        }

        if app.code_mode {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                "CODE",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let left_line = Line::from(left_spans);

        frame.render_widget(Paragraph::new(left_line), chunks[0]);
        frame.render_widget(
            Paragraph::new(Line::from(right_span)).alignment(Alignment::Right),
            chunks[1],
        );
    }
}

// ─── Model info helper ──────────────────────────────────────────────────────

/// Returns (display_name, provider_group, context) for a known model ID.
pub(crate) fn model_info(id: &str) -> (&str, &str, &str) {
    match id {
        "opus" => ("Claude Opus 4.6", "Claude Code", "1M ctx"),
        "sonnet" => ("Claude Sonnet 4.6", "Claude Code", "200K ctx"),
        "haiku" => ("Claude Haiku 4.5", "Claude Code", "200K ctx"),
        "gpt-4o" => ("GPT-4o", "OpenAI", "128K ctx"),
        "gpt-4o-mini" => ("GPT-4o Mini", "OpenAI", "128K ctx"),
        "llama3" => ("Llama 3", "Ollama", "8K ctx"),
        other => (other, "Other", ""),
    }
}

// ─── Model selector overlay ──────────────────────────────────────────────────

fn render_model_selector(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Group models by provider, preserving the order defined here.
    let provider_groups: &[(&str, &[&str])] = &[
        ("Claude Code", &["opus", "sonnet", "haiku"]),
        ("OpenAI", &["gpt-4o", "gpt-4o-mini"]),
        ("Ollama", &["llama3"]),
        ("Other", &[]),
    ];

    // Build lines and track which line indices correspond to selectable models.
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut model_index: usize = 0;
    // Map from flat model index -> line index (for scroll tracking).
    let mut _model_line_map: Vec<usize> = Vec::new();

    // Collect "other" models not in any predefined group.
    let known: std::collections::HashSet<&str> = [
        "opus", "sonnet", "haiku", "gpt-4o", "gpt-4o-mini", "llama3",
    ]
    .iter()
    .copied()
    .collect();
    let other_models: Vec<&str> = app
        .available_models
        .iter()
        .filter(|m| !known.contains(m.as_str()))
        .map(|m| m.as_str())
        .collect();

    for &(provider_name, group_models) in provider_groups {
        // Determine which models in this group are actually available.
        let models_in_group: Vec<&str> = if provider_name == "Other" {
            other_models.clone()
        } else {
            group_models
                .iter()
                .filter(|id| app.available_models.contains(&id.to_string()))
                .copied()
                .collect()
        };
        if models_in_group.is_empty() {
            continue;
        }

        // Blank line before group (except at top)
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        // Provider header
        lines.push(Line::from(Span::styled(
            format!("  {}", provider_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        // Underline
        let underline_len = provider_name.len().min(40);
        lines.push(Line::from(Span::styled(
            format!("  {}", "\u{2500}".repeat(underline_len)),
            Style::default().fg(Color::DarkGray),
        )));

        for model_id in &models_in_group {
            let (display_name, _, ctx) = model_info(model_id);
            let is_selected = model_index == app.model_select_index;
            let is_active = *model_id == app.selected_model;

            let marker = if is_selected { "\u{25ba} " } else { "  " };
            let active_badge = if is_active { " [active]" } else { "" };

            // Pad model_id to 13 chars, display_name to 20 chars for alignment
            let id_padded = format!("{:<13}", model_id);
            let name_padded = format!("{:<20}", display_name);
            let label = format!(
                "  {}{} {} {}{}",
                marker, id_padded, name_padded, ctx, active_badge
            );

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

            _model_line_map.push(lines.len());
            lines.push(Line::from(Span::styled(label, style)));
            model_index += 1;
        }
    }

    // Calculate popup dimensions
    let content_height = lines.len() as u16 + 2; // +2 for top/bottom borders
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = (content_height + 2).min(area.height.saturating_sub(4)); // +2 for title/footer padding
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

    // Calculate scroll offset to keep the selected model visible.
    let inner_height = popup_height.saturating_sub(4) as usize; // borders + padding
    let selected_line = _model_line_map
        .get(app.model_select_index)
        .copied()
        .unwrap_or(0);
    let scroll = if selected_line >= inner_height {
        (selected_line - inner_height + 1) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll, 0));

    frame.render_widget(paragraph, popup_area);
}

// ─── Provider selector overlay ───────────────────────────────────────────────

/// Human-friendly display name for a provider key.
pub(crate) fn provider_display_name(key: &str) -> &'static str {
    match key {
        "claude_code" | "claude" => "Claude Code",
        "ollama" => "Ollama",
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        _ => "Custom",
    }
}

/// Short description for the provider selector overlay.
pub(crate) fn provider_description(key: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_display_names() {
        assert_eq!(provider_display_name("claude_code"), "Claude Code");
        assert_eq!(provider_display_name("claude"), "Claude Code");
        assert_eq!(provider_display_name("openai"), "OpenAI");
        assert_eq!(provider_display_name("ollama"), "Ollama");
        assert_eq!(provider_display_name("openrouter"), "OpenRouter");
        // Unknown should return "Custom"
        let unknown = provider_display_name("unknown_provider");
        assert_eq!(unknown, "Custom");
    }

    #[test]
    fn provider_descriptions() {
        let desc = provider_description("openai");
        assert!(desc.contains("API key"), "OpenAI desc should mention API key, got: {desc}");

        let desc = provider_description("ollama");
        assert!(desc.contains("local") || desc.contains("no API"),
            "Ollama desc should mention local, got: {desc}");

        let desc = provider_description("claude_code");
        assert!(desc.contains("subscription"),
            "Claude Code desc should mention subscription, got: {desc}");
    }

    #[test]
    fn model_info_known_models() {
        let (name, group, ctx) = model_info("opus");
        assert!(name.contains("Opus"), "expected Opus in name, got: {name}");
        assert!(group.contains("Claude"), "expected Claude in group, got: {group}");
        assert!(!ctx.is_empty(), "context should not be empty");

        let (name, group, _ctx) = model_info("gpt-4o");
        assert!(name.contains("GPT"), "expected GPT in name, got: {name}");
        assert_eq!(group, "OpenAI");
    }

    #[test]
    fn model_info_unknown_model() {
        let (name, group, ctx) = model_info("totally_unknown_model");
        assert_eq!(name, "totally_unknown_model");
        assert_eq!(group, "Other");
        assert!(ctx.is_empty());
    }
}

