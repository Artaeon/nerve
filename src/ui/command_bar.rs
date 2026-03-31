use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
};

use crate::app::App;
use crate::prompts;

/// Render the Nerve Bar — a floating, centered command palette.
pub fn render_command_bar(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ── Dimensions: centered popup ~60% wide, up to 50% tall ─────────
    let popup_width = (area.width * 60 / 100).max(40).min(area.width.saturating_sub(4));
    let popup_height = (area.height * 50 / 100).max(10).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the region behind the popup.
    frame.render_widget(Clear, popup_area);

    // ── Outer block ──────────────────────────────────────────────────
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Nerve Bar ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]).alignment(Alignment::Center))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // ── Layout inside the popup: input (3 rows) + results list + match count ─
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // ── Input line ──────────────────────────────────────────────────
    let input_paragraph = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            app.command_bar_input.as_str(),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "\u{258c}", // ▌ cursor
            Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(input_paragraph, chunks[0]);

    // ── Filtered prompt list ────────────────────────────────────────
    let all_prompts = prompts::all_prompts();
    let matcher = SkimMatcherV2::default();
    let query = &app.command_bar_input;

    let mut scored: Vec<(i64, &prompts::SmartPrompt)> = if query.is_empty() {
        all_prompts.iter().map(|p| (0i64, p)).collect()
    } else {
        all_prompts
            .iter()
            .filter_map(|p| {
                let haystack = format!("{} {}", p.name, p.description);
                matcher
                    .fuzzy_match(&haystack, query)
                    .map(|score| (score, p))
            })
            .collect()
    };

    // Sort by descending score.
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let match_count = scored.len();

    let items: Vec<ListItem<'_>> = scored
        .iter()
        .enumerate()
        .map(|(i, (_score, prompt))| {
            let is_selected = i == app.command_bar_select_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let cat_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let line = Line::from(vec![
                Span::styled(&prompt.name, style),
                Span::styled("  ", style),
                Span::styled(&prompt.description, cat_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    // Use ListState for automatic scroll tracking.
    let mut state = ListState::default();
    state.select(Some(app.command_bar_select_index));

    frame.render_stateful_widget(list, chunks[1], &mut state);

    // ── Match count display ─────────────────────────────────────────
    let total = all_prompts.len();
    let count_text = format!("{}/{} prompts", match_count, total);
    let count_widget = Paragraph::new(Line::from(Span::styled(
        count_text,
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(count_widget, chunks[2]);
}

/// Return the number of fuzzy-matched prompts for the current command bar query.
pub fn matched_prompt_count(app: &App) -> usize {
    let all_prompts = prompts::all_prompts();
    let query = &app.command_bar_input;
    if query.is_empty() {
        return all_prompts.len();
    }
    let matcher = SkimMatcherV2::default();
    all_prompts
        .iter()
        .filter(|p| {
            let haystack = format!("{} {}", p.name, p.description);
            matcher.fuzzy_match(&haystack, query).is_some()
        })
        .count()
}

/// Return the SmartPrompt currently selected in the command bar, if any.
pub fn selected_prompt(app: &App) -> Option<prompts::SmartPrompt> {
    let all_prompts = prompts::all_prompts();
    let matcher = SkimMatcherV2::default();
    let query = &app.command_bar_input;

    let mut scored: Vec<(i64, &prompts::SmartPrompt)> = if query.is_empty() {
        all_prompts.iter().map(|p| (0i64, p)).collect()
    } else {
        all_prompts
            .iter()
            .filter_map(|p| {
                let haystack = format!("{} {}", p.name, p.description);
                matcher
                    .fuzzy_match(&haystack, query)
                    .map(|score| (score, p))
            })
            .collect()
    };
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    scored
        .get(app.command_bar_select_index)
        .map(|(_, p)| (*p).clone())
}
