use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use super::theme::resolve_theme;
use crate::app::{App, InputMode};

pub fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let theme = resolve_theme(app);

    let mode_indicator = match app.input_mode {
        InputMode::Normal => Span::styled(
            " NOR ",
            Style::default()
                .fg(Color::White)
                .bg(theme.user_color)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Insert => Span::styled(
            " INS ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let border_color = match app.input_mode {
        InputMode::Insert => theme.active_border(),
        InputMode::Normal => theme.inactive_border(),
    };

    let is_empty = app.input.is_empty();

    // Build the displayed text with a cursor indicator or placeholder.
    // For multi-line input we build a Vec<Line> so that explicit newlines are honoured.
    let input_lines: Vec<Line<'_>> = if is_empty && app.input_mode == InputMode::Insert {
        // Show placeholder showcasing capabilities when empty in insert mode.
        vec![Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled("Ask anything... ", Style::default().fg(theme.dim)),
            Span::styled(
                "/",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("commands  ", Style::default().fg(theme.dim)),
            Span::styled(
                "@",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("files  ", Style::default().fg(theme.dim)),
            Span::styled(
                "Ctrl+K",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Nerve Bar", Style::default().fg(theme.dim)),
            Span::styled(
                "\u{258c}",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ])]
    } else if is_empty && app.input_mode == InputMode::Normal {
        // Hint in normal mode when empty — show key actions.
        vec![Line::from(vec![
            mode_indicator,
            Span::raw(" "),
            Span::styled(
                "i",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": type  ", Style::default().fg(theme.dim)),
            Span::styled(
                "/",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": commands  ", Style::default().fg(theme.dim)),
            Span::styled(
                "?",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": help  ", Style::default().fg(theme.dim)),
            Span::styled(
                "Ctrl+K",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": Nerve Bar  ", Style::default().fg(theme.dim)),
            Span::styled(
                "Ctrl+,",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(": settings", Style::default().fg(theme.dim)),
        ])]
    } else {
        let pos = app.cursor_position.min(app.input.len());
        // Walk back to find a valid char boundary if needed.
        let pos = (0..=pos)
            .rev()
            .find(|&i| app.input.is_char_boundary(i))
            .unwrap_or(0);
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
                                .fg(theme.accent)
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
                            .fg(theme.accent)
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
    let scroll_pos = (0..=scroll_pos)
        .rev()
        .find(|&i| app.input.is_char_boundary(i))
        .unwrap_or(0);
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

    // Hint text for bottom border of the input box.
    let hint = match app.input_mode {
        InputMode::Insert => {
            "Enter: send | Alt+Enter: newline | Esc: normal | /: commands | @: files"
        }
        InputMode::Normal => "i: insert | /: commands | ?: help | Ctrl+K: Nerve Bar | q: quit",
    };

    let title_line = Line::from(vec![
        Span::styled(
            " Message ",
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({word_count} words) "),
            Style::default().fg(theme.dim),
        ),
    ]);
    let bottom_line = Line::from(vec![Span::styled(
        format!(" {hint} "),
        Style::default().fg(theme.dim),
    )]);

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
