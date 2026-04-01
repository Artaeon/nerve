use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::prompts;

/// Render the full-screen SmartPrompt browser.
///
/// Layout:
/// ```text
/// ┌──────────────────────────────────────────────────┐
/// │  Filter: ________________________________________│
/// ├──────────────┬───────────────────────────────────┤
/// │  Categories  │  Prompts in selected category     │
/// │              │                                   │
/// │  > Writing   │  > Fix Grammar                    │
/// │    Coding    │    Improve Writing                │
/// │    ...       │    Summarize                      │
/// │              ├───────────────────────────────────┤
/// │              │  Preview of selected prompt       │
/// └──────────────┴───────────────────────────────────┘
/// ```
pub fn render_prompt_picker(frame: &mut Frame, app: &App, area: Rect) {
    // ── Outer block ──────────────────────────────────────────────────
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " SmartPrompts ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "— Tab to switch panels, j/k navigate, Enter to use, Esc to close ",
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // ── Top: filter input (3 lines) ─────────────────────────────────
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let filter_block = Block::default()
        .title(" Filter ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    let filter_text = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        Span::styled(
            app.prompt_filter.as_str(),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "\u{258c}",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(filter_block);
    frame.render_widget(filter_text, vert[0]);

    // ── Bottom: two-column layout ───────────────────────────────────
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(vert[1]);

    // ── Left: category list ─────────────────────────────────────────
    let categories = prompts::categories();

    let cat_items: Vec<ListItem<'_>> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let is_selected = i == app.prompt_category_index;
            let is_focused = !app.prompt_focus_right;
            let style = if is_selected && is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_selected { " > " } else { "   " };
            ListItem::new(Line::from(Span::styled(format!("{marker}{cat}"), style)))
        })
        .collect();

    let cat_border_color = if !app.prompt_focus_right {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let cat_list = List::new(cat_items).block(
        Block::default()
            .title(" Categories ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(cat_border_color)),
    );
    frame.render_widget(cat_list, columns[0]);

    // ── Right: split into prompt list (top) + preview (bottom) ──────
    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(columns[1]);

    // ── Prompts in selected category, filtered ──────────────────────
    let selected_category = categories
        .get(app.prompt_category_index)
        .cloned()
        .unwrap_or_default();

    let all_prompts = prompts::all_prompts();
    let matcher = SkimMatcherV2::default();
    let query = &app.prompt_filter;

    let filtered: Vec<&prompts::SmartPrompt> = all_prompts
        .iter()
        .filter(|p| p.category == selected_category)
        .filter(|p| {
            if query.is_empty() {
                return true;
            }
            let haystack = format!("{} {}", p.name, p.description);
            matcher.fuzzy_match(&haystack, query).is_some()
        })
        .collect();

    let prompt_items: Vec<ListItem<'_>> = filtered
        .iter()
        .enumerate()
        .map(|(i, prompt)| {
            let is_selected = i == app.prompt_select_index;
            let is_focused = app.prompt_focus_right;
            let style = if is_selected && is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let desc_style = if is_selected && is_focused {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let marker = if is_selected { " > " } else { "   " };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker}{}", prompt.name), style),
                Span::styled(format!("  {}", prompt.description), desc_style),
            ]))
        })
        .collect();

    let prompt_border_color = if app.prompt_focus_right {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let prompt_list = List::new(prompt_items).block(
        Block::default()
            .title(format!(" {} ", selected_category))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(prompt_border_color)),
    );
    frame.render_widget(prompt_list, right_split[0]);

    // ── Preview of the selected prompt ──────────────────────────────
    let preview_text = if let Some(prompt) = filtered.get(app.prompt_select_index) {
        vec![
            Line::from(Span::styled(
                &prompt.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                &prompt.description,
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Template:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                &prompt.template,
                Style::default().fg(Color::White),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No prompts match the filter",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::new(1, 1, 1, 1)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, right_split[1]);
}

/// Return the number of prompts visible in the right panel for the current
/// category and filter.
pub fn visible_prompt_count(app: &App) -> usize {
    let categories = prompts::categories();
    let selected_category = categories
        .get(app.prompt_category_index)
        .cloned()
        .unwrap_or_default();

    let all_prompts = prompts::all_prompts();
    let matcher = SkimMatcherV2::default();
    let query = &app.prompt_filter;

    all_prompts
        .iter()
        .filter(|p| p.category == selected_category)
        .filter(|p| {
            if query.is_empty() {
                return true;
            }
            let haystack = format!("{} {}", p.name, p.description);
            matcher.fuzzy_match(&haystack, query).is_some()
        })
        .count()
}
