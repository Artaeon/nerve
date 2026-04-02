//! Autocomplete popup overlay rendered above the input area.
//!
//! Shows matching slash commands (when typing `/`) or file paths (when typing `@`).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use super::resolve_theme;
use crate::app::App;

/// Render the autocomplete popup above the input area.
///
/// Does nothing when autocomplete is not visible or has no items.
pub fn render_autocomplete(frame: &mut Frame, app: &App, input_area: Rect) {
    if !app.autocomplete_visible || app.autocomplete_items.is_empty() {
        return;
    }

    let theme = resolve_theme(app);

    let max_items = 8.min(app.autocomplete_items.len());
    let popup_height = max_items as u16 + 2; // +2 for borders
    let popup_width = 50.min(input_area.width.saturating_sub(2));

    let popup_area = Rect::new(
        input_area.x + 1,
        input_area.y.saturating_sub(popup_height),
        popup_width,
        popup_height,
    );

    // Clear the area behind the popup so it doesn't render on top of chat text.
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .autocomplete_items
        .iter()
        .enumerate()
        .take(max_items)
        .map(|(i, item)| {
            let style = if i == app.autocomplete_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(item.clone(), style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent)),
    );

    frame.render_widget(list, popup_area);
}
