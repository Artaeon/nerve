pub mod autocomplete;
pub mod chat;
pub mod clipboard_manager;
pub mod command_bar;
pub mod help;
pub mod history_browser;
pub mod input_box;
pub mod markdown;
pub mod prompt_picker;
pub mod search;
pub mod selectors;
pub mod settings;
pub mod status_bar;
pub mod theme;
pub mod utils;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use input_box::render_input;
use selectors::{render_model_selector, render_provider_selector};
use status_bar::render_status_bar;
use utils::{display_width, truncate_with_ellipsis};

use crate::app::{App, AppMode};

// Re-exported so sibling render modules and external callers keep resolving
// these at the `crate::ui::…` path (e.g. `super::resolve_theme` from
// autocomplete) and so the names are available inside this module too.
pub use selectors::provider_display_name;
pub use theme::resolve_theme;

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
            lines += 1 + display_width(line) / input_width.max(1);
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
            Constraint::Length(3),             // top bar
            Constraint::Min(1),                // chat area
            Constraint::Length(bottom_height), // input + status (DYNAMIC)
        ])
        .split(area);

    render_top_bar(frame, app, main_chunks[0]);
    chat::render_chat(frame, app, main_chunks[1]);
    render_bottom(frame, app, main_chunks[2]);

    // ── Overlays (drawn on top of the base layout) ──────────────────
    match app.mode {
        AppMode::CommandBar => command_bar::render_command_bar(frame, app),
        AppMode::Help => help::render_help(frame, app),
        AppMode::ModelSelect => render_model_selector(frame, app),
        AppMode::ProviderSelect => render_provider_selector(frame, app),
        AppMode::ClipboardManager => clipboard_manager::render_clipboard_manager(frame, app),
        AppMode::SearchOverlay => search::render_search(frame, app),
        _ => {}
    }
}

// ─── Top bar ─────────────────────────────────────────────────────────────────

fn render_top_bar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = resolve_theme(app);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.separator()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate message count for the badge
    let msg_count = app.current_conversation().messages.len();
    let provider_label = provider_display_name(&app.selected_provider);

    // Build the conversation tab indicator
    let conv_count = app.conversations.len();
    let conv_num = app.active_conversation + 1;
    let conv_indicator = if conv_count > 1 {
        format!("\u{25c4} {conv_num}/{conv_count} \u{25ba} ")
    } else {
        String::new()
    };

    // Mode badges (shown next to branding when active)
    let mut badge_spans: Vec<Span> = vec![];
    if app.agent_mode {
        badge_spans.push(Span::styled(
            " AGENT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.code_mode {
        badge_spans.push(Span::styled(
            " CODE ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let badge_width: u16 = badge_spans.iter().map(|s| s.width() as u16).sum();
    let brand_width = 15 + badge_width;

    let right_display = format!(
        "{} \u{203a} {} \u{2502} {} msgs ",
        provider_label, app.selected_model, msg_count
    );
    let right_len = display_width(&right_display) as u16;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(brand_width), // branding + version + badges
            Constraint::Length(3),           // separator
            Constraint::Min(1),              // conversation indicator + title
            Constraint::Length(right_len),   // provider/model + msg count
        ])
        .split(inner);

    // Branding + mode badges
    let mut brand_spans = vec![
        Span::styled(
            " Nerve ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            concat!(" v", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.dim),
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
        Style::default().fg(theme.separator()),
    )));
    frame.render_widget(sep, chunks[1]);

    // Conversation indicator + title (truncated if too long for available space)
    let title = &app.current_conversation().title;
    let max_title_len = chunks[2]
        .width
        .saturating_sub(display_width(&conv_indicator) as u16) as usize;
    let display_title = truncate_with_ellipsis(title, max_title_len);
    let mut title_spans = Vec::new();
    if !conv_indicator.is_empty() {
        title_spans.push(Span::styled(conv_indicator, Style::default().fg(theme.dim)));
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
            format!("{provider_label} "),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled("\u{203a} ", Style::default().fg(theme.separator())),
        Span::styled(
            app.selected_model.to_string(),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" \u{2502} ", Style::default().fg(theme.separator())),
        Span::styled(
            format!("{msg_count} msgs "),
            Style::default().fg(theme.accent),
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

    // Autocomplete overlay (drawn on top, above the input area).
    autocomplete::render_autocomplete(frame, app, chunks[0]);
}
