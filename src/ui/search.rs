use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};

use crate::app::App;

/// Render the search overlay as a small floating bar at the top-center of the screen.
pub fn render_search(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ~60% width, 3 lines tall, positioned at top-center.
    let popup_width = (area.width * 60 / 100)
        .max(30)
        .min(area.width.saturating_sub(2));
    let popup_height = 3u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = 1; // near the top, not centered
    let popup_area = Rect::new(x, y, popup_width, popup_height);
    // Note: Search bar intentionally NOT centered vertically — pinned to top.

    frame.render_widget(Clear, popup_area);

    // Border color: cyan when matches found (or query empty), red when no matches.
    let border_color = if app.search_query.is_empty() || !app.search_results.is_empty() {
        Color::Cyan
    } else {
        Color::Red
    };

    // Match count string for the right side.
    let match_info = if app.search_query.is_empty() {
        String::new()
    } else if app.search_results.is_empty() {
        "0/0 matches".to_string()
    } else {
        format!(
            "{}/{} matches",
            app.search_current + 1,
            app.search_results.len()
        )
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Search ",
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Build the search line: "> query...cursor    N/M matches"
    let query_display = app.search_query.as_str();
    let cursor = "\u{258c}"; // ▌

    // Calculate how much space we have for the match info on the right.
    let available = inner.width as usize;
    let left_part_len = 2 + query_display.len() + 1; // "> " + query + cursor
    let padding_len = available
        .saturating_sub(left_part_len)
        .saturating_sub(match_info.len());

    let line = Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(query_display.to_string(), Style::default().fg(Color::White)),
        Span::styled(
            cursor.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
        Span::raw(" ".repeat(padding_len)),
        Span::styled(match_info, Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(Paragraph::new(line), inner);
}
