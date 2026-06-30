use ratatui::{
    Frame,
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

use super::utils::centered_rect_fixed;
use crate::app::App;

/// Render the help overlay as a centred floating popup. Scrolls vertically
/// (j/k, arrows, PageUp/PageDown) when the content is taller than the popup.
pub fn render_help(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ── Popup dimensions ─────────────────────────────────────────────
    let popup_width = 64u16.min(area.width.saturating_sub(4));
    let popup_height = 58u16.min(area.height.saturating_sub(4));
    let popup_area = centered_rect_fixed(popup_width, popup_height, area);

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
        ("Ctrl+C / Ctrl+D", "Quit"),
        ("Esc", "Close overlay / cancel"),
        ("", ""),
        // -- Chat interaction --
        ("", "Chat Interaction"),
        ("Esc (streaming)", "Stop generation"),
        ("Ctrl+R", "Regenerate last response"),
        ("Ctrl+E", "Edit last message"),
        ("x (Normal mode)", "Delete last exchange"),
        ("", ""),
        // -- Navigation --
        ("", "Navigation"),
        ("Tab / Shift+Tab", "Switch conversations fwd / back"),
        ("j / Down", "Scroll down / next item"),
        ("k / Up", "Scroll up / previous item"),
        ("", ""),
        // -- Overlays --
        ("", "Overlays"),
        ("Ctrl+P", "Prompt picker"),
        ("Ctrl+M", "Model selector"),
        ("Ctrl+T", "Provider selector"),
        ("Ctrl+O", "History browser"),
        ("Ctrl+B", "Clipboard manager"),
        ("Ctrl+F", "Search in conversation"),
        ("Ctrl+,", "Settings"),
        ("Ctrl+H", "Help (this screen)"),
        ("", ""),
        // -- Editing --
        ("", "Editing"),
        ("i", "Enter insert mode"),
        ("Esc", "Return to normal mode"),
        ("Enter", "Send message"),
        ("Shift+Enter / Alt+Enter", "New line in input"),
        ("Ctrl+W", "Delete word before cursor"),
        ("Ctrl+V", "Paste from clipboard"),
        ("q (Normal mode)", "Quit"),
        ("", ""),
        // -- Clipboard & Copy --
        ("", "Clipboard"),
        ("Ctrl+Y", "Copy last AI response"),
        ("1-9 (Normal mode)", "Copy message #N from bottom"),
        ("", ""),
        // -- Copy commands --
        ("", "Copy Commands"),
        ("/copy", "Copy last AI response"),
        ("/copy <n>", "Copy message #n from bottom"),
        ("/copy code", "Copy last code block"),
        ("/copy all", "Copy entire conversation"),
        ("/copy last", "Copy last message (any role)"),
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
                    format!("--- {desc} "),
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
        let padded_key = format!("{key:<key_col_width$}");
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
        "j/k or \u{2191}\u{2193}: scroll   \u{2022}   Esc: close",
        Style::default().fg(Color::DarkGray),
    )));

    // ── Scroll handling ──────────────────────────────────────────────
    // Inner height = popup height minus the top/bottom borders (2) and the
    // top/bottom padding (2).
    let inner_height = popup_height.saturating_sub(4);
    let total_lines = lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(inner_height);
    app.help_max_scroll.set(max_scroll);
    let scroll = app.help_scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, popup_area);
}
