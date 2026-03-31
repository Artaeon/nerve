use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::prompts;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Build the list of category tab labels: "All" followed by every real category.
fn category_tabs() -> Vec<String> {
    std::iter::once("All".to_string())
        .chain(prompts::categories())
        .collect()
}

/// Apply both category and fuzzy-search filters, returning scored results.
fn filtered_prompts(app: &App) -> Vec<(i64, prompts::SmartPrompt)> {
    let all_prompts = prompts::all_prompts();
    let tabs = category_tabs();
    let active_cat = tabs.get(app.command_bar_category).map(|s| s.as_str());

    // Category filter.
    let cat_filtered: Vec<&prompts::SmartPrompt> = all_prompts
        .iter()
        .filter(|p| match active_cat {
            Some("All") | None => true,
            Some(cat) => p.category == cat,
        })
        .collect();

    let matcher = SkimMatcherV2::default();
    let query = &app.command_bar_input;

    let mut scored: Vec<(i64, prompts::SmartPrompt)> = if query.is_empty() {
        cat_filtered.iter().map(|p| (0i64, (*p).clone())).collect()
    } else {
        cat_filtered
            .iter()
            .filter_map(|p| {
                let haystack = format!("{} {} {} {}", p.name, p.description, p.category, p.template);
                matcher
                    .fuzzy_match(&haystack, query)
                    .map(|score| (score, (*p).clone()))
            })
            .collect()
    };

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the Nerve Bar — a floating, centered command palette.
pub fn render_command_bar(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // ── Dimensions: centered popup 80% wide, 70% tall ───────────────────
    let popup_width = (area.width * 80 / 100).max(50).min(area.width.saturating_sub(2));
    let popup_height = (area.height * 70 / 100).max(16).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the region behind the popup.
    frame.render_widget(Clear, popup_area);

    // ── Outer block ─────────────────────────────────────────────────────
    let block = Block::default()
        .title(
            Line::from(vec![Span::styled(
                " Nerve Bar ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // ── Layout: search | cats | list | preview | footer ─────────────────
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Length(1), // category tabs
            Constraint::Min(5),   // prompt list
            Constraint::Length(6), // preview panel
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // ── 1. Search input ─────────────────────────────────────────────────
    let input_paragraph = Paragraph::new(Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(app.command_bar_input.as_str(), Style::default().fg(Color::White)),
        Span::styled(
            "\u{258c}", // ▌ cursor
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(input_paragraph, chunks[0]);

    // ── 2. Category tabs ────────────────────────────────────────────────
    let tabs = category_tabs();
    let tab_spans: Vec<Span> = tabs
        .iter()
        .enumerate()
        .flat_map(|(i, name)| {
            let is_active = i == app.command_bar_category;
            let style = if is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let label = format!(" {} ", name);
            vec![
                Span::styled(label, style),
                Span::raw("  "),
            ]
        })
        .collect();

    let tab_line = Paragraph::new(Line::from(tab_spans));
    frame.render_widget(tab_line, chunks[1]);

    // ── 3. Prompt list ──────────────────────────────────────────────────
    let scored = filtered_prompts(app);
    let match_count = scored.len();
    let total = prompts::all_prompts().len();

    // Available width inside the prompt list area (for right-aligning badges).
    let list_width = chunks[2].width as usize;

    let items: Vec<ListItem<'_>> = scored
        .iter()
        .enumerate()
        .flat_map(|(i, (_score, prompt))| {
            let is_selected = i == app.command_bar_select_index;

            // -- Line 1: [marker] Name                        [Category] --
            let marker = if is_selected { "\u{25b6} " } else { "  " };
            let name_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let badge = format!("[{}]", prompt.category);
            let name_part = format!("{}{}", marker, prompt.name);
            // Calculate padding between name and badge.
            let padding_len = list_width
                .saturating_sub(name_part.len())
                .saturating_sub(badge.len())
                .max(2);
            let padding = " ".repeat(padding_len);

            let badge_style = if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let line1 = Line::from(vec![
                Span::styled(name_part, name_style),
                Span::raw(padding),
                Span::styled(badge, badge_style),
            ]);

            // -- Line 2: indented description --
            let desc_style = Style::default().fg(Color::DarkGray);
            let line2 = Line::from(vec![
                Span::raw("    "),
                Span::styled(prompt.description.clone(), desc_style),
            ]);

            // -- Line 3: blank spacer --
            let line3 = Line::from("");

            vec![
                ListItem::new(line1),
                ListItem::new(line2),
                ListItem::new(line3),
            ]
        })
        .collect();

    let list = List::new(items).block(Block::default());

    // Use ListState for automatic scroll tracking.
    // Each prompt takes 3 visual lines, so translate index to visual offset.
    let mut state = ListState::default();
    state.select(Some(app.command_bar_select_index * 3));

    frame.render_stateful_widget(list, chunks[2], &mut state);

    // ── 4. Preview panel ────────────────────────────────────────────────
    let preview_block = Block::default()
        .title(Span::styled(
            " Preview ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    if let Some((_score, prompt)) = scored.get(app.command_bar_select_index) {
        let preview_text = prompt.template.replace("{{input}}", "<your text>");
        let preview = Paragraph::new(preview_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(preview_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(preview, chunks[3]);
    } else {
        let preview = Paragraph::new("No prompt selected")
            .style(Style::default().fg(Color::DarkGray))
            .block(preview_block);
        frame.render_widget(preview, chunks[3]);
    }

    // ── 5. Footer ───────────────────────────────────────────────────────
    let footer_left = format!("{}/{} prompts", match_count, total);
    let footer_right = "Enter \u{23ce}  |  Tab: category  |  Esc: close";
    let footer_pad = (chunks[4].width as usize)
        .saturating_sub(footer_left.len())
        .saturating_sub(footer_right.len());

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(footer_left, Style::default().fg(Color::DarkGray)),
        Span::raw(" ".repeat(footer_pad)),
        Span::styled(footer_right, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, chunks[4]);
}

/// Return the number of filtered prompts for the current command bar state.
pub fn matched_prompt_count(app: &App) -> usize {
    filtered_prompts(app).len()
}

/// Return the SmartPrompt currently selected in the command bar, if any.
pub fn selected_prompt(app: &App) -> Option<prompts::SmartPrompt> {
    filtered_prompts(app)
        .get(app.command_bar_select_index)
        .map(|(_, p)| p.clone())
}
