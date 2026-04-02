use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::prompts::{self, BUILTIN_CACHE};
use super::utils::centered_rect_fixed;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Build the list of category tab labels: "All" followed by every real category.
pub(crate) fn category_tabs() -> Vec<String> {
    std::iter::once("All".to_string())
        .chain(prompts::categories())
        .collect()
}

/// Apply both category and fuzzy-search filters, returning scored results.
pub(crate) fn filtered_prompts(app: &App) -> Vec<(i64, prompts::SmartPrompt)> {
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
                let haystack =
                    format!("{} {} {} {}", p.name, p.description, p.category, p.template);
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
    let popup_width = (area.width * 80 / 100)
        .max(50)
        .min(area.width.saturating_sub(2));
    let popup_height = (area.height * 70 / 100)
        .max(16)
        .min(area.height.saturating_sub(2));
    let popup_area = centered_rect_fixed(popup_width, popup_height, area);

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
            Constraint::Min(5),    // prompt list
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
        Span::styled(
            app.command_bar_input.as_str(),
            Style::default().fg(Color::White),
        ),
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

    // ── 2. Category tabs (horizontally scrolled to keep active visible) ─
    let tabs = category_tabs();
    let available_width = chunks[1].width as usize;

    // Build tab labels with their character widths so we can compute scroll.
    let tab_labels: Vec<(String, bool)> = tabs
        .iter()
        .enumerate()
        .map(|(i, name)| (format!(" {} ", name), i == app.command_bar_category))
        .collect();

    // Calculate the character offset of the active tab and the total width.
    let mut active_start = 0usize;
    let mut active_end = 0usize;
    let mut cursor = 0usize;
    for (label, is_active) in &tab_labels {
        let w = label.len() + 2; // +2 for the "  " spacer
        if *is_active {
            active_start = cursor;
            active_end = cursor + w;
        }
        cursor += w;
    }

    // Determine horizontal scroll offset so the active tab is visible.
    // Try to center the active tab, but clamp to valid range.
    let scroll_x = if active_end <= available_width {
        0
    } else {
        let center = active_start.saturating_sub(available_width / 3);
        center.min(cursor.saturating_sub(available_width))
    };

    // Build visible spans with scroll offset applied.
    let mut tab_spans: Vec<Span> = Vec::new();
    let mut pos = 0usize;
    if scroll_x > 0 {
        tab_spans.push(Span::styled(
            "\u{25c0} ",
            Style::default().fg(Color::DarkGray),
        ));
    }
    for (i, (label, _is_active)) in tab_labels.iter().enumerate() {
        let w = label.len() + 2;
        let end = pos + w;
        // Skip tabs entirely before the scroll window.
        if end <= scroll_x {
            pos = end;
            continue;
        }
        let is_active = i == app.command_bar_category;
        let style = if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        tab_spans.push(Span::styled(label.clone(), style));
        tab_spans.push(Span::raw("  "));
        pos = end;
    }
    if cursor > scroll_x + available_width {
        tab_spans.push(Span::styled(
            " \u{25b6}",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let tab_line = Paragraph::new(Line::from(tab_spans));
    frame.render_widget(tab_line, chunks[1]);

    // ── 3. Prompt list ──────────────────────────────────────────────────
    let scored = filtered_prompts(app);
    let match_count = scored.len();
    // Use the cached builtin count + custom prompts instead of rebuilding
    // the full list a second time.
    let total = BUILTIN_CACHE.len() + prompts::custom::load_custom_prompts().len();

    // Available width inside the prompt list area (for right-aligning badges).
    let list_width = chunks[2].width as usize;

    let query = &app.command_bar_input;
    let mut items: Vec<ListItem<'_>> = Vec::new();

    // Show a helpful message when no prompts match the search.
    if scored.is_empty() && !query.is_empty() {
        let no_results = ListItem::new(Line::from(Span::styled(
            "  No prompts match your search. Try a different term.",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
        items.push(no_results);
    }

    items.extend(
        scored
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
                // Truncate long prompt names to fit the available width.
                let max_name_len = list_width.saturating_sub(badge.len() + 6); // 6 = marker + padding
                let truncated_name = if prompt.name.len() > max_name_len {
                    format!("{}...", &prompt.name[..max_name_len.saturating_sub(3)])
                } else {
                    prompt.name.clone()
                };
                let name_part = format!("{}{}", marker, truncated_name);
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

                // -- Line 2: indented description (truncated if too long) --
                let desc_style = Style::default().fg(Color::DarkGray);
                let max_desc_len = list_width.saturating_sub(4); // 4 = indent
                let truncated_desc = if prompt.description.len() > max_desc_len {
                    format!(
                        "{}...",
                        &prompt.description[..max_desc_len.saturating_sub(3)]
                    )
                } else {
                    prompt.description.clone()
                };
                let line2 = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(truncated_desc, desc_style),
                ]);

                // -- Line 3: blank spacer --
                let line3 = Line::from("");

                vec![
                    ListItem::new(line1),
                    ListItem::new(line2),
                    ListItem::new(line3),
                ]
            })
            .collect::<Vec<_>>(),
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    #[test]
    fn filtered_prompts_all_category() {
        let mut app = App::new();
        app.command_bar_category = 0; // "All"
        app.command_bar_input.clear();
        let results = filtered_prompts(&app);
        assert!(
            results.len() >= 130,
            "expected >= 130 prompts, got {}",
            results.len()
        );
    }

    #[test]
    fn filtered_prompts_specific_category() {
        let mut app = App::new();
        let tabs = category_tabs();
        let coding_idx = tabs.iter().position(|t| t == "Coding").unwrap_or(2);
        app.command_bar_category = coding_idx;
        app.command_bar_input.clear();
        let results = filtered_prompts(&app);
        assert!(
            results.len() >= 10,
            "expected >= 10 Coding prompts, got {}",
            results.len()
        );
        assert!(results.len() < 130, "expected fewer than all prompts");
        for (_, prompt) in &results {
            assert_eq!(prompt.category, "Coding");
        }
    }

    #[test]
    fn filtered_prompts_search_narrows_results() {
        let mut app = App::new();
        app.command_bar_category = 0;
        app.command_bar_input = "rust code review".into();
        let results = filtered_prompts(&app);
        assert!(
            !results.is_empty(),
            "should find at least one prompt matching 'rust code review'"
        );
        let all_count = {
            app.command_bar_input.clear();
            filtered_prompts(&app).len()
        };
        app.command_bar_input = "rust code review".into();
        let narrowed = filtered_prompts(&app);
        assert!(narrowed.len() < all_count, "search should narrow results");
    }

    #[test]
    fn filtered_prompts_no_match() {
        let mut app = App::new();
        app.command_bar_category = 0;
        app.command_bar_input = "zzzzzznonexistent".into();
        let results = filtered_prompts(&app);
        assert!(results.is_empty());
    }

    #[test]
    fn matched_prompt_count_empty_query() {
        let mut app = App::new();
        app.command_bar_input.clear();
        let count = matched_prompt_count(&app);
        assert!(count >= 130, "expected >= 130, got {}", count);
    }

    #[test]
    fn matched_prompt_count_with_query() {
        let mut app = App::new();
        app.command_bar_input = "summarize".into();
        let count = matched_prompt_count(&app);
        assert!(count >= 1);
        assert!(count < 130);
    }

    #[test]
    fn selected_prompt_first_item() {
        let mut app = App::new();
        app.command_bar_input.clear();
        app.command_bar_select_index = 0;
        let prompt = selected_prompt(&app);
        assert!(prompt.is_some());
    }

    #[test]
    fn selected_prompt_out_of_bounds() {
        let mut app = App::new();
        app.command_bar_input = "zzzzz".into();
        app.command_bar_select_index = 999;
        let prompt = selected_prompt(&app);
        assert!(prompt.is_none());
    }

    #[test]
    fn category_tabs_starts_with_all() {
        let tabs = category_tabs();
        assert_eq!(tabs[0], "All");
        assert!(
            tabs.len() >= 15,
            "expected >= 15 category tabs, got {}",
            tabs.len()
        );
    }

    #[test]
    fn search_matches_template_content() {
        let mut app = App::new();
        app.command_bar_category = 0;
        app.command_bar_input = "OWASP".into();
        let results = filtered_prompts(&app);
        assert!(
            !results.is_empty(),
            "should find Security Audit by template content"
        );
    }
}
