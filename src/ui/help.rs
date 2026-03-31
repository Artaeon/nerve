use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

/// Render the help overlay as a centred floating popup.
pub fn render_help(frame: &mut Frame) {
    let area = frame.area();

    // ── Popup dimensions ─────────────────────────────────────────────
    let popup_width = 64u16.min(area.width.saturating_sub(4));
    let popup_height = 38u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the background behind the popup.
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Keybindings ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::new(2, 2, 1, 1));

    // ── Keybinding entries ──────────────────────────────────────────
    // Format: (key, description)
    // Empty key + empty desc = blank line
    // Empty key + non-empty desc = section header
    let bindings: &[(&str, &str)] = &[
        // -- General --
        ("", "General"),
        ("Ctrl+K  /  :", "Open Nerve Bar (command palette)"),
        ("Ctrl+N", "New conversation"),
        ("Ctrl+L", "Clear conversation"),
        ("Ctrl+C", "Quit"),
        ("Esc", "Close overlay / cancel"),
        ("", ""),
        // -- Navigation --
        ("", "Navigation"),
        ("Tab", "Switch conversations"),
        ("j / Down", "Scroll down / next item"),
        ("k / Up", "Scroll up / previous item"),
        ("g", "Scroll to top"),
        ("G", "Scroll to bottom"),
        ("", ""),
        // -- Overlays --
        ("", "Overlays"),
        ("Ctrl+P", "Prompt picker"),
        ("Ctrl+M", "Model selector"),
        ("Ctrl+H", "Help (this screen)"),
        ("", ""),
        // -- Editing --
        ("", "Editing"),
        ("i", "Enter insert mode"),
        ("Esc", "Return to normal mode"),
        ("Enter", "Send message"),
        ("", ""),
        // -- Clipboard & Copy --
        ("", "Clipboard"),
        ("Ctrl+Y", "Copy last AI response"),
        ("Ctrl+Shift+V", "Clipboard manager"),
    ];

    let key_col_width = 20;
    let mut lines: Vec<Line<'_>> = Vec::new();

    for &(key, desc) in bindings {
        if key.is_empty() && desc.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        if key.is_empty() {
            // Section header with a subtle underline effect.
            lines.push(Line::from(vec![
                Span::styled(
                    format!("--- {} ", desc),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "-".repeat(40usize.saturating_sub(desc.len() + 5)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            continue;
        }

        // Pad the key column to a fixed width for alignment.
        let padded_key = format!("{key:<width$}", width = key_col_width);
        lines.push(Line::from(vec![
            Span::styled(
                padded_key,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(Color::White)),
        ]));
    }

    // Footer
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}
