use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use super::utils::{centered_rect_fixed, display_width, truncate_with_ellipsis};
use crate::app::App;
use crate::prompts::{self, BUILTIN_CACHE};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Return quick action entries for the Nerve Bar.
///
/// Quick actions use the `@action:<id>` prefix in their template field so the
/// command-bar Enter handler can distinguish them from slash commands and
/// SmartPrompts.
fn quick_action_prompts() -> Vec<prompts::SmartPrompt> {
    let actions: Vec<(&str, &str, &str)> = vec![
        (
            "Open Settings",
            "Open the settings overlay",
            "@action:settings",
        ),
        (
            "Switch Theme",
            "Cycle through available themes",
            "@action:theme",
        ),
        (
            "Toggle Agent Mode",
            "Enable or disable agent mode",
            "@action:agent_toggle",
        ),
        (
            "Toggle Code Mode",
            "Enable or disable code mode",
            "@action:code_toggle",
        ),
        ("Show Help", "Open the help overlay", "@action:help"),
        (
            "Browse History",
            "Open conversation history browser",
            "@action:history",
        ),
        (
            "Clipboard Manager",
            "Open the clipboard manager",
            "@action:clipboard",
        ),
    ];

    actions
        .into_iter()
        .map(|(name, desc, template)| prompts::SmartPrompt {
            name: name.into(),
            description: desc.into(),
            template: template.into(),
            category: "Quick Actions".into(),
            tags: vec!["action".into()],
        })
        .collect()
}

/// Return all slash commands as SmartPrompt entries for the Nerve Bar.
fn command_prompts() -> Vec<prompts::SmartPrompt> {
    let commands: Vec<(&str, &str, &str)> = vec![
        // Chat
        ("/help", "Show all available commands", "Chat"),
        ("/clear", "Clear current conversation", "Chat"),
        ("/new", "Start new conversation", "Chat"),
        ("/delete", "Delete current conversation", "Chat"),
        ("/rename", "Rename current conversation", "Chat"),
        ("/export", "Export conversation to markdown", "Chat"),
        ("/copy", "Copy last AI response to clipboard", "Chat"),
        (
            "/copy code",
            "Copy last code block from AI response",
            "Chat",
        ),
        ("/copy all", "Copy entire conversation", "Chat"),
        ("/system", "Show or set system prompt", "Chat"),
        // AI Provider
        ("/provider", "Switch AI provider", "AI Provider"),
        ("/providers", "List available providers", "AI Provider"),
        ("/model", "Switch AI model", "AI Provider"),
        ("/models", "List available models", "AI Provider"),
        (
            "/ollama",
            "Manage Ollama models (list/pull/remove)",
            "AI Provider",
        ),
        (
            "/mode",
            "Switch mode (efficient/thorough/agent/learning/auto/code/review)",
            "AI Provider",
        ),
        (
            "/agent on",
            "Enable agent mode (AI tool loop)",
            "AI Provider",
        ),
        ("/agent off", "Disable agent mode", "AI Provider"),
        ("/agent status", "Show agent mode status", "AI Provider"),
        (
            "/agent undo",
            "Roll back to pre-agent git checkpoint",
            "AI Provider",
        ),
        (
            "/agent diff",
            "Show what the agent changed (git diff)",
            "AI Provider",
        ),
        ("/agent commit", "Commit agent changes", "AI Provider"),
        (
            "/autocontext",
            "Auto-gather project context (alias: /ac)",
            "AI Provider",
        ),
        (
            "/code on",
            "Enable code mode (Claude Code only)",
            "AI Provider",
        ),
        ("/code off", "Disable code mode", "AI Provider"),
        ("/cwd", "Set working directory", "AI Provider"),
        ("/cd", "Change working directory", "AI Provider"),
        // Knowledge & Context
        ("/file", "Read file as context", "Knowledge"),
        ("/files", "Read multiple files as context", "Knowledge"),
        ("/summary", "Summarize current conversation", "Knowledge"),
        (
            "/compact",
            "Compact conversation (save tokens)",
            "Knowledge",
        ),
        ("/context", "Show current AI context window", "Knowledge"),
        ("/tokens", "Show token usage breakdown", "Knowledge"),
        ("/kb add", "Add directory to knowledge base", "Knowledge"),
        ("/kb search", "Search knowledge base", "Knowledge"),
        ("/kb list", "List knowledge bases", "Knowledge"),
        ("/kb status", "Show KB statistics", "Knowledge"),
        ("/kb clear", "Clear knowledge base", "Knowledge"),
        ("/url", "Scrape URL for context", "Knowledge"),
        // Shell & Git
        ("/run", "Run shell command and show output", "Shell & Git"),
        (
            "/pipe",
            "Run command and add output as context",
            "Shell & Git",
        ),
        ("/diff", "Show git diff (adds as context)", "Shell & Git"),
        ("/test", "Auto-detect and run project tests", "Shell & Git"),
        ("/build", "Auto-detect and run project build", "Shell & Git"),
        (
            "/lint",
            "Auto-detect and run linter (clippy/eslint/ruff)",
            "Shell & Git",
        ),
        (
            "/format",
            "Auto-detect and run formatter (fmt/prettier/ruff)",
            "Shell & Git",
        ),
        (
            "/search",
            "Search codebase with ripgrep (adds results as context)",
            "Shell & Git",
        ),
        (
            "/web",
            "Search the web (DuckDuckGo, adds results as context)",
            "Shell & Git",
        ),
        (
            "/git",
            "Quick git operations (status/log/diff/branch)",
            "Shell & Git",
        ),
        (
            "/commit",
            "Stage all and commit (AI message if omitted)",
            "Shell & Git",
        ),
        ("/stage", "Stage files (all if no args)", "Shell & Git"),
        ("/unstage", "Unstage files (all if no args)", "Shell & Git"),
        (
            "/gitbranch",
            "Create/switch/delete git branches",
            "Shell & Git",
        ),
        (
            "/gitbranch switch",
            "Switch to existing branch",
            "Shell & Git",
        ),
        ("/gitbranch delete", "Delete a git branch", "Shell & Git"),
        ("/stash", "Stash changes", "Shell & Git"),
        ("/stash pop", "Pop latest stash", "Shell & Git"),
        ("/stash list", "List stashes", "Shell & Git"),
        ("/log", "Show git log (default 10)", "Shell & Git"),
        ("/gitstatus", "Show full git status", "Shell & Git"),
        // Project Scaffolding
        (
            "/template list",
            "List available project templates",
            "Scaffolding",
        ),
        ("/template", "Create project from template", "Scaffolding"),
        (
            "/scaffold",
            "AI-generate a project from description",
            "Scaffolding",
        ),
        (
            "/map",
            "Show project map (file tree + symbols)",
            "Scaffolding",
        ),
        // Automation
        ("/auto list", "List automations", "Automation"),
        ("/auto run", "Run automation", "Automation"),
        ("/auto info", "Show automation details", "Automation"),
        ("/auto create", "Create custom automation", "Automation"),
        ("/auto delete", "Delete custom automation", "Automation"),
        // Power User
        ("/alias", "List or create aliases", "Power User"),
        ("/!!", "Recall last input", "Power User"),
        ("/repeat", "Recall last input (same as /!!)", "Power User"),
        // Plugins
        ("/plugin list", "List installed plugins", "Plugins"),
        ("/plugin init", "Create example plugin", "Plugins"),
        ("/plugin reload", "Reload all plugins", "Plugins"),
        // Sessions
        ("/session", "Show session info", "Sessions"),
        ("/session save", "Save current session", "Sessions"),
        ("/session list", "List saved sessions", "Sessions"),
        ("/session restore", "Restore last session", "Sessions"),
        // Branching
        (
            "/branch save",
            "Save conversation branch point",
            "Branching",
        ),
        ("/branch list", "List saved branches", "Branching"),
        ("/branch restore", "Restore a saved branch", "Branching"),
        ("/branch delete", "Delete a branch", "Branching"),
        ("/branch diff", "Compare current with a branch", "Branching"),
        // Workspace
        ("/workspace", "Show detected project info", "Workspace"),
        // Usage & Cost
        (
            "/usage",
            "Show session usage stats (estimated)",
            "Usage & Cost",
        ),
        ("/cost", "Alias for /usage", "Usage & Cost"),
        ("/limit", "Show spending limit info", "Usage & Cost"),
        ("/limit on", "Enable spending limit", "Usage & Cost"),
        ("/limit off", "Disable spending limit", "Usage & Cost"),
        ("/limit set", "Set spending limit amount", "Usage & Cost"),
        // System
        ("/status", "Show system status", "System"),
        // Theme
        ("/theme", "Switch UI theme", "System"),
    ];

    commands
        .into_iter()
        .map(|(name, desc, cat)| prompts::SmartPrompt {
            name: name.into(),
            description: desc.into(),
            template: name.into(),
            category: format!("Commands: {}", cat),
            tags: vec!["command".into()],
        })
        .collect()
}

/// Build the list of category tab labels: "All" followed by every real category,
/// plus "Quick Actions" and "Commands" groups.
pub(crate) fn category_tabs() -> Vec<String> {
    let mut tabs: Vec<String> = std::iter::once("All".to_string())
        .chain(prompts::categories())
        .collect();

    // Quick Actions tab (always present).
    tabs.push("Quick Actions".to_string());

    // Collect unique command categories and append them.
    let cmd_cats: Vec<String> = command_prompts()
        .iter()
        .map(|p| p.category.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    tabs.extend(cmd_cats);

    tabs
}

/// Apply both category and fuzzy-search filters, returning scored results.
/// Includes SmartPrompts, quick actions, and slash commands.
pub(crate) fn filtered_prompts(app: &App) -> Vec<(i64, prompts::SmartPrompt)> {
    let mut all_prompts = prompts::all_prompts();
    all_prompts.extend(quick_action_prompts());
    all_prompts.extend(command_prompts());

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
        let w = display_width(label) + 2; // +2 for the "  " spacer
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
        let w = display_width(label) + 2;
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
    // Use the cached builtin count + custom prompts + quick actions + command
    // prompts instead of rebuilding the full list a second time.
    let total = BUILTIN_CACHE.len()
        + prompts::custom::load_custom_prompts().len()
        + quick_action_prompts().len()
        + command_prompts().len();

    // Available width inside the prompt list area (for right-aligning badges).
    let list_width = chunks[2].width as usize;

    let query = &app.command_bar_input;
    let mut items: Vec<ListItem<'_>> = Vec::new();

    // Show a helpful message when no prompts match the search.
    if scored.is_empty() && !query.is_empty() {
        let no_results = ListItem::new(Line::from(Span::styled(
            "  No results match your search. Try a different term.",
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
                let max_name_len = list_width.saturating_sub(display_width(&badge) + 6); // 6 = marker + padding
                let truncated_name = truncate_with_ellipsis(&prompt.name, max_name_len);
                let name_part = format!("{}{}", marker, truncated_name);
                // Calculate padding between name and badge.
                let padding_len = list_width
                    .saturating_sub(display_width(&name_part))
                    .saturating_sub(display_width(&badge))
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
                let truncated_desc = truncate_with_ellipsis(&prompt.description, max_desc_len);
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
    let footer_left = format!("{}/{} items", match_count, total);
    let footer_right = "Enter \u{23ce}  |  Tab: category  |  Esc: close";
    let footer_pad = (chunks[4].width as usize)
        .saturating_sub(display_width(&footer_left))
        .saturating_sub(display_width(footer_right));

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
        // 166+ builtin prompts + 7 quick actions + ~90 command entries
        assert!(
            results.len() >= 225,
            "expected >= 225 prompts+actions+commands, got {}",
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
        let all_count = {
            app.command_bar_category = 0;
            filtered_prompts(&app).len()
        };
        app.command_bar_category = coding_idx;
        let coding_count = filtered_prompts(&app).len();
        assert!(coding_count < all_count, "expected fewer than all items");
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
        assert!(count >= 225, "expected >= 225, got {}", count);
    }

    #[test]
    fn matched_prompt_count_with_query() {
        let mut app = App::new();
        app.command_bar_input = "summarize".into();
        let count = matched_prompt_count(&app);
        assert!(count >= 1);
        let all_count = {
            app.command_bar_input.clear();
            matched_prompt_count(&app)
        };
        app.command_bar_input = "summarize".into();
        assert!(matched_prompt_count(&app) < all_count);
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
    fn command_prompts_included_in_results() {
        let mut app = App::new();
        app.command_bar_category = 0;
        app.command_bar_input = "/help".into();
        let results = filtered_prompts(&app);
        assert!(
            results.iter().any(|(_, p)| p.name == "/help"),
            "should find /help command in results"
        );
    }

    #[test]
    fn command_category_tabs_present() {
        let tabs = category_tabs();
        assert!(
            tabs.iter().any(|t| t.starts_with("Commands:")),
            "expected at least one Commands: category tab"
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

    // ── Quick action tests ──────────────────────────────────────────────

    #[test]
    fn quick_action_prompts_have_action_prefix() {
        let actions = quick_action_prompts();
        assert!(!actions.is_empty(), "should have at least one quick action");
        for action in &actions {
            assert!(
                action.template.starts_with("@action:"),
                "quick action template should start with @action: but got: {}",
                action.template
            );
        }
    }

    #[test]
    fn quick_action_prompts_all_quick_actions_category() {
        let actions = quick_action_prompts();
        for action in &actions {
            assert_eq!(
                action.category, "Quick Actions",
                "quick action category should be 'Quick Actions'"
            );
        }
    }

    #[test]
    fn quick_action_settings_in_filtered_results() {
        let mut app = App::new();
        app.command_bar_category = 0;
        app.command_bar_input = "Open Settings".into();
        let results = filtered_prompts(&app);
        assert!(
            results
                .iter()
                .any(|(_, p)| p.template == "@action:settings"),
            "should find settings quick action"
        );
    }

    #[test]
    fn quick_actions_category_tab_present() {
        let tabs = category_tabs();
        assert!(
            tabs.iter().any(|t| t == "Quick Actions"),
            "expected a 'Quick Actions' category tab"
        );
    }

    #[test]
    fn quick_actions_filter_by_category() {
        let mut app = App::new();
        let tabs = category_tabs();
        let qa_idx = tabs
            .iter()
            .position(|t| t == "Quick Actions")
            .expect("Quick Actions tab should exist");
        app.command_bar_category = qa_idx;
        app.command_bar_input.clear();
        let results = filtered_prompts(&app);
        assert!(
            !results.is_empty(),
            "Quick Actions category should have entries"
        );
        for (_, p) in &results {
            assert_eq!(p.category, "Quick Actions");
        }
    }

    #[test]
    fn quick_action_template_detection() {
        // Verify that @action: templates are distinct from / commands and SmartPrompts
        let actions = quick_action_prompts();
        for action in &actions {
            assert!(!action.template.starts_with('/'));
            assert!(!action.template.contains("{{input}}"));
            assert!(action.template.starts_with("@action:"));
        }
    }
}
