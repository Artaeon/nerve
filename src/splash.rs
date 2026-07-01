pub(crate) fn render_splash(frame: &mut ratatui::Frame, status: &str) {
    use ratatui::{
        layout::{Alignment, Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Clear, Paragraph},
    };

    let area = frame.area();
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(12),
            Constraint::Percentage(30),
        ])
        .split(area);

    let center = chunks[1];

    let art_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);
    let version_style = Style::default().fg(Color::Yellow);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("    _   __", art_style)),
        Line::from(Span::styled("   / | / /__  ______   _____", art_style)),
        Line::from(Span::styled("  /  |/ / _ \\/ ___/ | / / _ \\", art_style)),
        Line::from(Span::styled(" / /|  /  __/ /   | |/ /  __/", art_style)),
        Line::from(Span::styled("/_/ |_/\\___/_/    |___/\\___/", art_style)),
        Line::from(""),
        Line::from(Span::styled("  Raw AI power in your terminal", dim_style)),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  v{}", env!("CARGO_PKG_VERSION")), version_style),
            Span::styled("  |  ", dim_style),
            Span::styled(status.to_string(), dim_style),
        ]),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default());

    frame.render_widget(paragraph, center);
}

pub(crate) fn render_goodbye(frame: &mut ratatui::Frame, app: &crate::app::App) {
    use ratatui::{
        layout::{Alignment, Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Clear, Paragraph},
    };

    let area = frame.area();
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Min(8),
            Constraint::Percentage(35),
        ])
        .split(area);

    let art_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let stat_style = Style::default().fg(Color::Yellow);

    // Session stats
    let msg_count: usize = app.conversations.iter().map(|c| c.messages.len()).sum();
    let conv_count = app.conversations.len();

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Thanks for using Nerve.", art_style)),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Session: ", dim),
            Span::styled(
                format!("{conv_count} conversation(s), {msg_count} message(s)"),
                stat_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("  Cost: ", dim),
            Span::styled(app.usage_stats.format_cost(), stat_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  nerve --continue to resume  |  nerve --help for options",
            dim,
        )),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(Block::default());

    frame.render_widget(paragraph, chunks[1]);
}
