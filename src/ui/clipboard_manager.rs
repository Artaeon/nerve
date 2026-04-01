use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
};

use crate::app::App;

/// Render the clipboard manager overlay.
pub fn render_clipboard_manager(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ── Dimensions: centered popup ~70% wide, ~60% tall ─────────────
    let popup_width = (area.width * 70 / 100)
        .max(40)
        .min(area.width.saturating_sub(4));
    let popup_height = (area.height * 60 / 100)
        .max(10)
        .min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the region behind the popup.
    frame.render_widget(Clear, popup_area);

    // ── Outer block ──────────────────────────────────────────────────
    let block = Block::default()
        .title(
            Line::from(vec![Span::styled(
                " Clipboard Manager ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // ── Layout: search input (3) + list (min 1) + help bar (1) ──────
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // ── Search input line ───────────────────────────────────────────
    let input_paragraph = Paragraph::new(Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.clipboard_search, Style::default().fg(Color::White)),
        Span::styled(
            "\u{258c}", // ▌ cursor
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(input_paragraph, chunks[0]);

    // ── Filtered clipboard entries ──────────────────────────────────
    let filtered = app.clipboard_manager.search(&app.clipboard_search);
    let match_count = filtered.len();

    // Available width for content inside the list area.
    let content_width = chunks[1].width as usize;

    let items: Vec<ListItem<'_>> = filtered
        .iter()
        .enumerate()
        .map(|(i, (_original_idx, entry))| {
            let is_selected = i == app.clipboard_select_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let badge_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            };
            let time_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let badge = entry.source.badge();
            let time_str = format_relative_time(&entry.timestamp);

            // Compute how much space the preview text can use.
            // Layout: "{badge} {preview}  {time}"
            let overhead = badge.len() + 1 + 2 + time_str.len();
            let max_preview = content_width.saturating_sub(overhead);
            let preview: String = entry.preview.chars().take(max_preview).collect();

            let line = Line::from(vec![
                Span::styled(badge, badge_style),
                Span::styled(" ", style),
                Span::styled(preview, style),
                Span::styled("  ", style),
                Span::styled(time_str, time_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(Block::default()).highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(app.clipboard_select_index));
    frame.render_stateful_widget(list, chunks[1], &mut state);

    // ── Bottom help bar ─────────────────────────────────────────────
    let total = app.clipboard_manager.entries().len();
    let help_text = format!(
        "{}/{} entries | Enter: Copy to clipboard | d: Delete | Esc: Close",
        match_count, total
    );
    let help_widget = Paragraph::new(Line::from(Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(help_widget, chunks[2]);
}

/// Return the number of entries matching the current clipboard search query.
pub fn matched_entry_count(app: &App) -> usize {
    app.clipboard_manager.search(&app.clipboard_search).len()
}

fn format_relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(*dt);
    if diff.num_seconds() < 60 {
        return "just now".into();
    }
    if diff.num_minutes() < 60 {
        return format!("{}m ago", diff.num_minutes());
    }
    if diff.num_hours() < 24 {
        return format!("{}h ago", diff.num_hours());
    }
    format!("{}d ago", diff.num_days())
}
