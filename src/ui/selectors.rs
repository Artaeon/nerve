use ratatui::{
    Frame,
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
};

use super::utils::centered_rect_fixed;
use crate::app::App;

// ─── Model info helper ──────────────────────────────────────────────────────

/// Returns (display_name, provider_group, context) for a known model ID.
pub fn model_info(id: &str) -> (&str, &str, &str) {
    match id {
        "opus" => ("Claude Opus 4.8", "Claude Code", "1M ctx"),
        "sonnet" => ("Claude Sonnet 4.6", "Claude Code", "200K ctx"),
        "haiku" => ("Claude Haiku 4.5", "Claude Code", "200K ctx"),
        "gpt-4o" => ("GPT-4o", "OpenAI", "128K ctx"),
        "gpt-4o-mini" => ("GPT-4o Mini", "OpenAI", "128K ctx"),
        "copilot" => ("GitHub Copilot", "Copilot", "8K ctx"),
        // OpenRouter models
        "anthropic/claude-sonnet-4-20250514" => ("Claude Sonnet 4", "OpenRouter", "200K ctx"),
        "openai/gpt-4o" => ("GPT-4o", "OpenRouter", "128K ctx"),
        "meta-llama/llama-3-70b" => ("Llama 3 70B", "OpenRouter", "8K ctx"),
        "google/gemini-pro" => ("Gemini Pro", "OpenRouter", "32K ctx"),
        // Ollama and other models — show the ID as the name
        other => (other, "Ollama", ""),
    }
}

// ─── Model selector overlay ──────────────────────────────────────────────────

pub fn render_model_selector(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Dynamically group available models by their provider_group from model_info.
    // Preserve a preferred ordering for known groups.
    let group_order: &[&str] = &[
        "Claude Code",
        "OpenAI",
        "OpenRouter",
        "Copilot",
        "Ollama",
        "Other",
    ];

    // Build a map from group name -> list of model IDs.
    let mut group_map: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for model_id in &app.available_models {
        let (_, group, _) = model_info(model_id);
        group_map.entry(group).or_default().push(model_id.as_str());
    }

    // Build lines and track which line indices correspond to selectable models.
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut model_index: usize = 0;
    // Map from flat model index -> line index (for scroll tracking).
    let mut _model_line_map: Vec<usize> = Vec::new();

    // Collect groups in preferred order, then any remaining groups.
    let mut seen_groups: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut ordered_groups: Vec<&str> = Vec::new();
    for &g in group_order {
        if group_map.contains_key(g) {
            ordered_groups.push(g);
            seen_groups.insert(g);
        }
    }
    for g in group_map.keys() {
        if !seen_groups.contains(g) {
            ordered_groups.push(g);
        }
    }

    for provider_name in &ordered_groups {
        let models_in_group = match group_map.get(provider_name) {
            Some(models) => models.clone(),
            None => continue,
        };
        if models_in_group.is_empty() {
            continue;
        }

        // Blank line before group (except at top)
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        // Provider header
        lines.push(Line::from(Span::styled(
            format!("  {provider_name}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        // Underline
        let underline_len = provider_name.len().min(40);
        lines.push(Line::from(Span::styled(
            format!("  {}", "\u{2500}".repeat(underline_len)),
            Style::default().fg(Color::DarkGray),
        )));

        for model_id in &models_in_group {
            let (display_name, _, ctx) = model_info(model_id);
            let is_selected = model_index == app.model_select_index;
            let is_active = *model_id == app.selected_model;

            let marker = if is_selected { "\u{25ba} " } else { "  " };
            let active_badge = if is_active { " [active]" } else { "" };

            // Pad model_id to 13 chars, display_name to 20 chars for alignment
            let id_padded = format!("{model_id:<13}");
            let name_padded = format!("{display_name:<20}");
            let label = format!("  {marker}{id_padded} {name_padded} {ctx}{active_badge}");

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            _model_line_map.push(lines.len());
            lines.push(Line::from(Span::styled(label, style)));
            model_index += 1;
        }

        // Show download hint for Ollama
        if *provider_name == "Ollama" {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "\u{2193} Download more models: /ollama pull <name>",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "    Popular: llama3, mistral, codellama, qwen2.5, phi3, gemma2",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Calculate popup dimensions
    let content_height = lines.len() as u16 + 2; // +2 for top/bottom borders
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = (content_height + 2).min(area.height.saturating_sub(4)); // +2 for title/footer padding
    let popup_area = centered_rect_fixed(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Select Model ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .title_bottom(
            Line::from(Span::styled(
                if app.selected_provider == "ollama" {
                    " Enter: Select | /ollama pull <name> to download | Esc: Cancel "
                } else {
                    " Enter: Select | Esc: Cancel "
                },
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    // Calculate scroll offset to keep the selected model visible.
    let inner_height = popup_height.saturating_sub(4) as usize; // borders + padding
    let selected_line = _model_line_map
        .get(app.model_select_index)
        .copied()
        .unwrap_or(0);
    let scroll = if selected_line >= inner_height {
        (selected_line - inner_height + 1) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).block(block).scroll((scroll, 0));

    frame.render_widget(paragraph, popup_area);
}

// ─── Provider selector overlay ───────────────────────────────────────────────

/// Human-friendly display name for a provider key.
pub fn provider_display_name(key: &str) -> &'static str {
    match key {
        "claude_code" | "claude" => "Claude Code",
        "ollama" => "Ollama",
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        _ => "Custom",
    }
}

/// Short description for the provider selector overlay.
pub fn provider_description(key: &str) -> &'static str {
    match key {
        "claude_code" | "claude" => "subscription, no API key",
        "ollama" => "local, no API key",
        "openai" => "requires API key",
        "openrouter" => "requires API key",
        _ => "custom provider",
    }
}

pub fn render_provider_selector(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height =
        (app.available_providers.len() as u16 + 4).min(area.height.saturating_sub(4));
    let popup_area = centered_rect_fixed(popup_width, popup_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(
            Line::from(Span::styled(
                " Select Provider ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        )
        .title_bottom(
            Line::from(Span::styled(
                " Enter: Select | Esc: Cancel ",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::horizontal(1));

    let items: Vec<ListItem<'_>> = app
        .available_providers
        .iter()
        .enumerate()
        .map(|(i, provider_key)| {
            let is_selected = i == app.provider_select_index;
            let is_active = *provider_key == app.selected_provider;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let marker = if is_active { " * " } else { "   " };
            let name = provider_display_name(provider_key);
            let desc = provider_description(provider_key);
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{name} ({desc})"),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(app.provider_select_index));

    frame.render_stateful_widget(list, popup_area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_display_names() {
        assert_eq!(provider_display_name("claude_code"), "Claude Code");
        assert_eq!(provider_display_name("claude"), "Claude Code");
        assert_eq!(provider_display_name("openai"), "OpenAI");
        assert_eq!(provider_display_name("ollama"), "Ollama");
        assert_eq!(provider_display_name("openrouter"), "OpenRouter");
        // Unknown should return "Custom"
        let unknown = provider_display_name("unknown_provider");
        assert_eq!(unknown, "Custom");
    }

    #[test]
    fn provider_descriptions() {
        let desc = provider_description("openai");
        assert!(
            desc.contains("API key"),
            "OpenAI desc should mention API key, got: {desc}"
        );

        let desc = provider_description("ollama");
        assert!(
            desc.contains("local") || desc.contains("no API"),
            "Ollama desc should mention local, got: {desc}"
        );

        let desc = provider_description("claude_code");
        assert!(
            desc.contains("subscription"),
            "Claude Code desc should mention subscription, got: {desc}"
        );
    }

    #[test]
    fn model_info_known_models() {
        let (name, group, ctx) = model_info("opus");
        assert!(name.contains("Opus"), "expected Opus in name, got: {name}");
        assert!(
            group.contains("Claude"),
            "expected Claude in group, got: {group}"
        );
        assert!(!ctx.is_empty(), "context should not be empty");

        let (name, group, _ctx) = model_info("gpt-4o");
        assert!(name.contains("GPT"), "expected GPT in name, got: {name}");
        assert_eq!(group, "OpenAI");
    }

    #[test]
    fn model_info_unknown_model() {
        let (name, group, ctx) = model_info("totally_unknown_model");
        assert_eq!(name, "totally_unknown_model");
        assert_eq!(group, "Ollama");
        assert!(ctx.is_empty());
    }
}
