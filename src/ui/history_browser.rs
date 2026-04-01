use chrono::{DateTime, Utc};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::history::ConversationRecord;

/// Classify a timestamp into a human-readable date group label.
pub(crate) fn group_label(dt: &DateTime<Utc>) -> &'static str {
    let now = Utc::now();
    let today = now.date_naive();
    let date = dt.date_naive();
    if date == today {
        return "Today";
    }
    if date == today - chrono::Duration::days(1) {
        return "Yesterday";
    }
    if (today - date).num_days() <= 7 {
        return "This Week";
    }
    "Older"
}

/// Format a relative time string from a datetime.
pub(crate) fn format_relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(*dt);
    if diff.num_seconds() < 60 {
        return "just now".into();
    }
    if diff.num_minutes() < 60 {
        return format!("{}m ago", diff.num_minutes());
    }
    if diff.num_hours() < 24 {
        return format!("{}h ago", diff.num_hours());
    }
    if diff.num_days() < 30 {
        return format!("{}d ago", diff.num_days());
    }
    format!("{}mo ago", diff.num_days() / 30)
}

/// Return the filtered list of history entries based on the current search query.
pub(crate) fn filtered_entries(app: &App) -> Vec<&ConversationRecord> {
    let matcher = SkimMatcherV2::default();
    let query = &app.history_search;

    app.history_entries
        .iter()
        .filter(|record| {
            if query.is_empty() {
                return true;
            }
            // Match against title
            if matcher.fuzzy_match(&record.title, query).is_some() {
                return true;
            }
            // Match against message content
            for msg in &record.messages {
                if matcher.fuzzy_match(&msg.content, query).is_some() {
                    return true;
                }
            }
            false
        })
        .collect()
}

/// Build a flat list of items for the left panel, interleaving group headers
/// and conversation entries. Returns the items and a mapping from item index
/// to the index into `filtered` (None for headers).
fn build_left_panel_items<'a>(
    filtered: &[&'a ConversationRecord],
    select_index: usize,
) -> (Vec<ListItem<'a>>, Vec<Option<usize>>) {
    let mut items: Vec<ListItem<'a>> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut current_group: Option<&str> = None;
    for (entry_idx, record) in filtered.iter().enumerate() {
        let label = group_label(&record.updated_at);
        if current_group != Some(label) {
            current_group = Some(label);
            // Insert group header
            let header = ListItem::new(Line::from(Span::styled(
                format!("  {} {}", "\u{25b8}", label),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            items.push(header);
            index_map.push(None);
        }

        let is_selected = entry_idx == select_index;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let meta_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let msg_count = record.messages.len();
        let time_str = format_relative_time(&record.updated_at);
        let title_display: String = record.title.chars().take(30).collect();

        let line = Line::from(vec![
            Span::styled(
                if is_selected { "  > " } else { "    " },
                style,
            ),
            Span::styled(title_display, style),
        ]);
        let detail_line = Line::from(vec![
            Span::styled("      ", meta_style),
            Span::styled(
                format!("{} msgs", msg_count),
                meta_style,
            ),
            Span::styled(" | ", meta_style),
            Span::styled(&record.model, meta_style),
            Span::styled(" | ", meta_style),
            Span::styled(time_str, meta_style),
        ]);

        items.push(ListItem::new(vec![line, detail_line]));
        index_map.push(Some(entry_idx));
    }

    (items, index_map)
}

/// Render the full-screen history browser.
pub fn render_history_browser(frame: &mut Frame, app: &App, area: Rect) {
    let filtered = filtered_entries(app);

    // ── Outer block ──────────────────────────────────────────────────
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " History ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // ── Vertical layout: search | main | help bar ───────────────────
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search
            Constraint::Min(1),   // main content
            Constraint::Length(1), // help bar
        ])
        .split(inner);

    // ── Search filter ───────────────────────────────────────────────
    let search_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let search_text = Paragraph::new(Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.history_search, Style::default().fg(Color::White)),
        Span::styled(
            "\u{258c}",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
        if app.history_search.is_empty() {
            Span::styled(
                "search filter...",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::raw("")
        },
    ]))
    .block(search_block);
    frame.render_widget(search_text, vert[0]);

    // ── Handle empty history ────────────────────────────────────────
    if filtered.is_empty() {
        let empty_msg = if app.history_entries.is_empty() {
            "No conversations yet"
        } else {
            "No conversations match the filter"
        };
        let empty_paragraph = Paragraph::new(Line::from(Span::styled(
            empty_msg,
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .padding(Padding::new(0, 0, vert[1].height / 2, 0)),
        );
        frame.render_widget(empty_paragraph, vert[1]);

        // Help bar
        let help_text = "Esc: Close";
        let help_widget = Paragraph::new(Line::from(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(help_widget, vert[2]);
        return;
    }

    // ── Two-column layout: conversation list | preview ──────────────
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vert[1]);

    // ── Left panel: conversation list grouped by date ───────────────
    let (left_items, index_map) = build_left_panel_items(&filtered, app.history_select_index);

    // Find the ListState index that corresponds to the selected entry.
    let list_state_index = index_map
        .iter()
        .position(|entry| *entry == Some(app.history_select_index))
        .unwrap_or(0);

    let left_list = List::new(left_items).block(
        Block::default()
            .title(Line::from(Span::styled(
                " Conversations ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(list_state_index));
    frame.render_stateful_widget(left_list, columns[0], &mut list_state);

    // ── Right panel: preview of selected conversation ───────────────
    let preview_lines = if let Some(record) = filtered.get(app.history_select_index) {
        let mut lines: Vec<Line<'_>> = Vec::new();

        // Title
        lines.push(Line::from(Span::styled(
            &record.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "Model: {} | {} messages | {}",
                record.model,
                record.messages.len(),
                format_relative_time(&record.updated_at),
            ),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));

        // Show message preview (first several messages)
        let max_preview_messages = 20;
        for msg in record.messages.iter().take(max_preview_messages) {
            let (role_label, role_color) = match msg.role.as_str() {
                "user" => ("You", Color::Green),
                "assistant" => ("AI", Color::Cyan),
                "system" => ("System", Color::Yellow),
                _ => (&*msg.role, Color::White),
            };

            lines.push(Line::from(Span::styled(
                format!("{} >", role_label),
                Style::default()
                    .fg(role_color)
                    .add_modifier(Modifier::BOLD),
            )));

            // Show first few lines of content, truncated
            let content_preview: String = msg
                .content
                .lines()
                .take(4)
                .collect::<Vec<_>>()
                .join("\n");
            let truncated: String = content_preview.chars().take(300).collect();
            for content_line in truncated.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", content_line),
                    Style::default().fg(Color::White),
                )));
            }
            if msg.content.len() > 300 || msg.content.lines().count() > 4 {
                lines.push(Line::from(Span::styled(
                    "  ...",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
        }

        if record.messages.len() > max_preview_messages {
            lines.push(Line::from(Span::styled(
                format!(
                    "  ... and {} more messages",
                    record.messages.len() - max_preview_messages
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No conversation selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let preview = Paragraph::new(preview_lines)
        .block(
            Block::default()
                .title(Line::from(Span::styled(
                    " Preview ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::new(1, 1, 1, 1)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, columns[1]);

    // ── Bottom help bar ─────────────────────────────────────────────
    let help_text = format!(
        "{}/{} conversations | Enter: Load | d: Delete | Esc: Close | {}: Navigate",
        filtered.len(),
        app.history_entries.len(),
        "\u{2191}\u{2193}",
    );
    let help_widget = Paragraph::new(Line::from(Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(help_widget, vert[2]);
}

/// Return the count of history entries matching the current search filter.
pub fn filtered_history_count(app: &App) -> usize {
    filtered_entries(app).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::history::{ConversationRecord, MessageRecord};
    use chrono::{Duration, Utc};

    fn make_record(title: &str, age_days: i64, messages: Vec<(&str, &str)>) -> ConversationRecord {
        let now = Utc::now();
        let ts = now - Duration::days(age_days);
        ConversationRecord {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            messages: messages
                .into_iter()
                .map(|(role, content)| MessageRecord {
                    role: role.into(),
                    content: content.into(),
                    timestamp: ts,
                })
                .collect(),
            model: "opus".into(),
            created_at: ts,
            updated_at: ts,
        }
    }

    #[test]
    fn group_label_today() {
        let now = Utc::now();
        assert_eq!(group_label(&now), "Today");
    }

    #[test]
    fn group_label_yesterday() {
        let yesterday = Utc::now() - Duration::days(1);
        assert_eq!(group_label(&yesterday), "Yesterday");
    }

    #[test]
    fn group_label_this_week() {
        let three_days_ago = Utc::now() - Duration::days(3);
        assert_eq!(group_label(&three_days_ago), "This Week");
    }

    #[test]
    fn group_label_older() {
        let old = Utc::now() - Duration::days(30);
        assert_eq!(group_label(&old), "Older");
    }

    #[test]
    fn format_relative_time_just_now() {
        let now = Utc::now();
        let result = format_relative_time(&now);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_relative_time_minutes() {
        let five_min_ago = Utc::now() - Duration::minutes(5);
        let result = format_relative_time(&five_min_ago);
        assert!(result.contains("m ago"), "expected minutes ago, got: {result}");
    }

    #[test]
    fn format_relative_time_hours() {
        let three_hours_ago = Utc::now() - Duration::hours(3);
        let result = format_relative_time(&three_hours_ago);
        assert!(result.contains("h ago"), "expected hours ago, got: {result}");
    }

    #[test]
    fn format_relative_time_days() {
        let ten_days_ago = Utc::now() - Duration::days(10);
        let result = format_relative_time(&ten_days_ago);
        assert!(result.contains("d ago"), "expected days ago, got: {result}");
    }

    #[test]
    fn format_relative_time_months() {
        let ninety_days_ago = Utc::now() - Duration::days(90);
        let result = format_relative_time(&ninety_days_ago);
        assert!(result.contains("mo ago"), "expected months ago, got: {result}");
    }

    #[test]
    fn filtered_entries_empty_history() {
        let app = App::new();
        let entries = filtered_entries(&app);
        assert!(entries.is_empty());
    }

    #[test]
    fn filtered_entries_returns_all_when_no_query() {
        let mut app = App::new();
        app.history_entries = vec![
            make_record("Rust discussion", 0, vec![("user", "hello")]),
            make_record("Python question", 1, vec![("user", "world")]),
        ];
        app.history_search.clear();
        let entries = filtered_entries(&app);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn filtered_entries_filters_by_title() {
        let mut app = App::new();
        app.history_entries = vec![
            make_record("Rust discussion", 0, vec![("user", "hello")]),
            make_record("Python question", 1, vec![("user", "world")]),
        ];
        app.history_search = "Rust".into();
        let entries = filtered_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Rust discussion");
    }

    #[test]
    fn filtered_entries_filters_by_message_content() {
        let mut app = App::new();
        app.history_entries = vec![
            make_record("Chat A", 0, vec![("user", "tell me about quantum physics")]),
            make_record("Chat B", 1, vec![("user", "hello world")]),
        ];
        app.history_search = "quantum".into();
        let entries = filtered_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Chat A");
    }

    #[test]
    fn filtered_history_count_matches_filtered_entries() {
        let mut app = App::new();
        app.history_entries = vec![
            make_record("A", 0, vec![("user", "foo")]),
            make_record("B", 1, vec![("user", "bar")]),
        ];
        app.history_search.clear();
        assert_eq!(filtered_history_count(&app), 2);

        app.history_search = "A".into();
        assert_eq!(filtered_history_count(&app), filtered_entries(&app).len());
    }
}
