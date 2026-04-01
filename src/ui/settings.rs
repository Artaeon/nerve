use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::config;

// ─── Tab definitions ────────────────────────────────────────────────────────

const TAB_NAMES: &[&str] = &["General", "Providers", "Theme", "Keybinds"];

// ─── Public entry point ─────────────────────────────────────────────────────

/// Render the full-screen settings overlay.
pub fn render_settings(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Settings ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::new(2, 2, 1, 1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area: tab bar | separator | content | footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // separator
            Constraint::Min(1),    // content
            Constraint::Length(1), // separator
            Constraint::Length(1), // footer hints
        ])
        .split(inner);

    render_tab_bar(frame, app, chunks[0]);
    render_separator(frame, chunks[1]);
    render_tab_content(frame, app, chunks[2]);
    render_separator(frame, chunks[3]);
    render_footer(frame, chunks[4]);
}

// ─── Tab bar ────────────────────────────────────────────────────────────────

fn render_tab_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::raw("  "));

    for (i, name) in TAB_NAMES.iter().enumerate() {
        if i == app.settings_tab {
            spans.push(Span::styled(
                format!("[{name}]"),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("[{name}]"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        spans.push(Span::raw("  "));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ─── Separator ──────────────────────────────────────────────────────────────

fn render_separator(frame: &mut Frame, area: Rect) {
    let width = area.width as usize;
    let line = "\u{2500}".repeat(width);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            line,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

// ─── Footer ─────────────────────────────────────────────────────────────────

fn render_footer(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "Enter/Space",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Edit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Tab/Shift+Tab",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Switch tab  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "j/k",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": Close & Save", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

// ─── Tab content dispatcher ─────────────────────────────────────────────────

fn render_tab_content(frame: &mut Frame, app: &App, area: Rect) {
    match app.settings_tab {
        0 => render_general_tab(frame, app, area),
        1 => render_providers_tab(frame, app, area),
        2 => render_theme_tab(frame, app, area),
        3 => render_keybinds_tab(frame, app, area),
        _ => {}
    }
}

// ─── General tab ────────────────────────────────────────────────────────────

fn render_general_tab(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<(&str, String)> = vec![
        ("Default Provider", app.selected_provider.clone()),
        ("Default Model", app.selected_model.clone()),
        (
            "Agent Mode",
            if app.agent_mode {
                "ON".to_string()
            } else {
                "OFF".to_string()
            },
        ),
        (
            "Code Mode",
            if app.code_mode {
                "ON".to_string()
            } else {
                "OFF".to_string()
            },
        ),
        (
            "Spending Limit",
            if app.spending_limit.enabled {
                format!("ON (${:.2})", app.spending_limit.max_cost_usd)
            } else {
                format!("OFF (${:.2})", app.spending_limit.max_cost_usd)
            },
        ),
    ];

    let clamped = app.settings_select.min(items.len().saturating_sub(1));

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "General Settings",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (label, value)) in items.iter().enumerate() {
        let selected = i == clamped;
        let marker = if selected { "\u{25ba} " } else { "  " };
        let label_style = if selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let value_style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        lines.push(Line::from(vec![
            Span::styled(
                marker.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(format!("{:<20}", label), label_style),
            Span::styled(value.clone(), value_style),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Returns the item count for the General tab (used for clamping).
pub fn general_item_count() -> usize {
    5
}

// ─── Providers tab ──────────────────────────────────────────────────────────

fn render_providers_tab(frame: &mut Frame, app: &App, area: Rect) {
    let config = config::Config::load().unwrap_or_default();

    let providers: Vec<(&str, bool, String)> = vec![
        (
            "Claude Code",
            config
                .providers
                .claude_code
                .as_ref()
                .map(|p| p.enabled)
                .unwrap_or(false),
            "subscription, no API key".into(),
        ),
        (
            "OpenAI",
            config
                .providers
                .openai
                .as_ref()
                .map(|p| p.enabled)
                .unwrap_or(false),
            mask_api_key(
                config
                    .providers
                    .openai
                    .as_ref()
                    .and_then(|p| p.api_key.as_deref()),
            ),
        ),
        (
            "Ollama",
            config
                .providers
                .ollama
                .as_ref()
                .map(|p| p.enabled)
                .unwrap_or(false),
            config
                .providers
                .ollama
                .as_ref()
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".into()),
        ),
        (
            "OpenRouter",
            config
                .providers
                .openrouter
                .as_ref()
                .map(|p| p.enabled)
                .unwrap_or(false),
            mask_api_key(
                config
                    .providers
                    .openrouter
                    .as_ref()
                    .and_then(|p| p.api_key.as_deref()),
            ),
        ),
    ];

    let clamped = app.settings_select.min(providers.len().saturating_sub(1));

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Provider Configuration",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (name, enabled, detail)) in providers.iter().enumerate() {
        let selected = i == clamped;
        let marker = if selected { "\u{25ba} " } else { "  " };
        let status = if *enabled { "Enabled" } else { "Disabled" };
        let status_color = if *enabled { Color::Green } else { Color::Red };

        let label_style = if selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(
                marker.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(format!("{:<14}", name), label_style),
            Span::styled(format!("{:<10}", status), Style::default().fg(status_color)),
            Span::styled(detail.clone(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Returns the item count for the Providers tab.
pub fn providers_item_count() -> usize {
    4
}

fn mask_api_key(key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() > 8 => {
            let prefix = &k[..4];
            let suffix = &k[k.len() - 4..];
            format!("{prefix}...{suffix}")
        }
        Some(k) if !k.is_empty() => "****".into(),
        _ => "(not set)".into(),
    }
}

// ─── Theme tab ──────────────────────────────────────────────────────────────

fn render_theme_tab(frame: &mut Frame, app: &App, area: Rect) {
    let presets = config::theme_presets();

    // Items: 0 = preset selector, 1..4 = color previews (read-only display)
    let active_theme = presets.get(app.theme_index);
    let theme_name = active_theme.map(|(n, _)| *n).unwrap_or("Unknown");
    let theme = active_theme.map(|(_, t)| t);

    let items: Vec<(&str, String, Option<Color>)> = vec![
        ("Theme Preset", theme_name.to_string(), None),
        (
            "User Color",
            theme.map(|t| t.user_color.clone()).unwrap_or_default(),
            theme.map(|t| hex_to_color(&t.user_color)),
        ),
        (
            "Assistant Color",
            theme.map(|t| t.assistant_color.clone()).unwrap_or_default(),
            theme.map(|t| hex_to_color(&t.assistant_color)),
        ),
        (
            "Border Color",
            theme.map(|t| t.border_color.clone()).unwrap_or_default(),
            theme.map(|t| hex_to_color(&t.border_color)),
        ),
        (
            "Accent Color",
            theme.map(|t| t.accent_color.clone()).unwrap_or_default(),
            theme.map(|t| hex_to_color(&t.accent_color)),
        ),
    ];

    let clamped = app.settings_select.min(items.len().saturating_sub(1));

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Theme Settings",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (label, value, color)) in items.iter().enumerate() {
        let selected = i == clamped;
        let marker = if selected { "\u{25ba} " } else { "  " };
        let label_style = if selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(
                marker.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(format!("{:<20}", label), label_style),
        ];

        if let Some(c) = color {
            // Show a color swatch followed by the hex value
            spans.push(Span::styled(
                "\u{2588}\u{2588}\u{2588} ",
                Style::default().fg(*c),
            ));
            spans.push(Span::styled(
                value.clone(),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            // Plain value (theme name)
            let value_style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(value.clone(), value_style));
        }

        lines.push(Line::from(spans));
    }

    // Show all available presets below
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Available Presets:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (name, _)) in presets.iter().enumerate() {
        let active = i == app.theme_index;
        let marker = if active { "\u{25ba} " } else { "  " };
        let style = if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(Span::styled(format!("  {marker}{name}"), style)));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Returns the item count for the Theme tab.
pub fn theme_item_count() -> usize {
    5
}

/// Parse a hex colour string (#rrggbb) into a ratatui Color.
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

// ─── Keybinds tab ───────────────────────────────────────────────────────────

fn render_keybinds_tab(frame: &mut Frame, app: &App, area: Rect) {
    let config = config::Config::load().unwrap_or_default();

    let bindings: Vec<(&str, &str)> = vec![
        ("Ctrl+K / :", "Open Nerve Bar"),
        ("Ctrl+N", "New conversation"),
        ("Ctrl+L", "Clear conversation"),
        ("Ctrl+C / Ctrl+D", "Quit"),
        ("Ctrl+,", "Settings"),
        ("Ctrl+P", "Prompt picker"),
        ("Ctrl+M", "Model selector"),
        ("Ctrl+T", "Provider selector"),
        ("Ctrl+H", "Help"),
        ("Ctrl+F", "Search in conversation"),
        ("Ctrl+Y", "Copy last AI response"),
        ("Ctrl+B", "Clipboard manager"),
        ("Ctrl+O", "History browser"),
        ("Ctrl+R", "Regenerate response"),
        ("Ctrl+E", "Edit last message"),
        ("Tab/Shift+Tab", "Switch conversations"),
        ("j/k", "Scroll up/down"),
        ("i", "Enter insert mode"),
        ("Esc", "Cancel / close"),
        ("Enter", "Send message"),
        ("Shift+Enter", "New line"),
    ];

    let _ = config; // config is loaded for future use if keybinds become editable

    let clamped = app.settings_select.min(bindings.len().saturating_sub(1));

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Keybindings (read-only)",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, (key, desc)) in bindings.iter().enumerate() {
        let selected = i == clamped;
        let marker = if selected { "\u{25ba} " } else { "  " };
        let key_style = if selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let desc_style = if selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(
                marker.to_string(),
                if selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(format!("{:<22}", key), key_style),
            Span::styled(*desc, desc_style),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Returns the item count for the Keybinds tab.
pub fn keybinds_item_count() -> usize {
    21
}
