//! Autocomplete popup overlay rendered above the input area.
//!
//! Shows matching slash commands with descriptions (when typing `/`)
//! or file paths (when typing `@`).

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding},
};

use super::resolve_theme;
use crate::app::App;

/// Maximum number of visible items in the popup.
const MAX_VISIBLE: usize = 10;

/// Render the autocomplete popup above the input area.
///
/// Does nothing when autocomplete is not visible or has no items.
pub fn render_autocomplete(frame: &mut Frame, app: &App, input_area: Rect) {
    if !app.autocomplete_visible || app.autocomplete_items.is_empty() {
        return;
    }

    let theme = resolve_theme(app);

    let max_items = MAX_VISIBLE.min(app.autocomplete_items.len());
    let popup_height = max_items as u16 + 2; // +2 for borders

    // Calculate width: fit the longest item + description, capped at terminal width.
    let max_item_width = app
        .autocomplete_items
        .iter()
        .take(max_items)
        .map(|item| super::utils::display_width(item))
        .max()
        .unwrap_or(30);
    let popup_width = ((max_item_width + 4) as u16) // +4 for padding/borders
        .clamp(40, input_area.width.saturating_sub(2));

    // Position above the input if there's room, otherwise below.
    let above_y = input_area.y.saturating_sub(popup_height);
    let enough_above = input_area.y >= popup_height + 3; // +3 for top bar
    let popup_y = if enough_above {
        above_y
    } else {
        (input_area.y + input_area.height).min(frame.area().height.saturating_sub(popup_height))
    };

    let popup_area = Rect::new(
        input_area.x + 1,
        popup_y,
        popup_width.min(frame.area().width.saturating_sub(input_area.x + 2)),
        popup_height.min(frame.area().height.saturating_sub(popup_y)),
    );

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .autocomplete_items
        .iter()
        .enumerate()
        .take(max_items)
        .map(|(i, item)| {
            let selected = i == app.autocomplete_index;

            // Split "command  ── description" into parts for styling.
            if let Some(sep_pos) = item.find("  \u{2500}\u{2500} ") {
                let cmd = &item[..sep_pos];
                let desc = &item[sep_pos + 5..]; // skip "  ── "

                let cmd_style = if selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                };
                let desc_style = if selected {
                    Style::default().fg(Color::Black).bg(theme.accent)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {cmd}"), cmd_style),
                    Span::styled(format!("  {desc}"), desc_style),
                ]))
            } else {
                let style = if selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(format!(" {item}"), style)))
            }
        })
        .collect();

    // Dynamic title based on context.
    let title = if app.input.contains('@') {
        " Files "
    } else {
        " Commands "
    };

    // Footer hint.
    let footer = Line::from(vec![
        Span::styled(
            " Tab",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": accept  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "\u{2191}\u{2193}",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": dismiss ", Style::default().fg(Color::DarkGray)),
    ]);

    let count_text = format!(
        " {}/{} ",
        app.autocomplete_index + 1,
        app.autocomplete_items.len()
    );

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent))
            .title_top(Line::from(vec![Span::styled(
                title,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]))
            .title_top(
                Line::from(Span::styled(
                    &count_text,
                    Style::default().fg(Color::DarkGray),
                ))
                .right_aligned(),
            )
            .title_bottom(footer.left_aligned())
            .padding(Padding::right(1)),
    );

    frame.render_widget(list, popup_area);
}
