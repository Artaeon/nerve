use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::selectors::provider_display_name;
use super::theme::resolve_theme;
use super::utils::display_width;
use crate::app::{App, InputMode};

pub fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let theme = resolve_theme(app);
    let provider_label = provider_display_name(&app.selected_provider);
    let sep = Span::styled(" \u{2502} ", Style::default().fg(theme.separator()));

    // ── Helper: build mode badges (reused by both streaming and idle) ───
    let mut badge_spans: Vec<Span<'_>> = Vec::new();

    // NerveMode badge — always visible so users know what mode they're in.
    let (mode_label, mode_fg, mode_bg) = match app.mode_name.as_str() {
        "efficient" | "eco" => ("ECO", Color::Black, Color::Green),
        "thorough" => ("THOROUGH", Color::Black, Color::Cyan),
        "learning" => ("LEARN", Color::Black, Color::Blue),
        "auto" => ("AUTO", Color::Black, Color::Yellow),
        "code" => ("CODE-MODE", Color::Black, Color::Magenta),
        "review" => ("REVIEW", Color::Black, Color::Red),
        "agent" => ("AGENT-MODE", Color::Black, Color::Magenta),
        _ => ("STANDARD", Color::Black, Color::Rgb(88, 91, 112)), // visible neutral badge
    };
    badge_spans.push(Span::styled(
        format!(" {mode_label} "),
        Style::default()
            .fg(mode_fg)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD),
    ));

    // Agent badge with iteration count.
    if app.agent_mode {
        badge_spans.push(Span::raw(" "));
        let agent_text = if let Some(ref tool) = app.active_tool {
            format!(" \u{2699} {tool} ")
        } else if app.agent_iterations > 0 {
            format!(" AGENT {}/10 ", app.agent_iterations)
        } else {
            " AGENT ".to_string()
        };
        let agent_bg = if app.active_tool.is_some() {
            Color::Yellow
        } else {
            Color::Magenta
        };
        badge_spans.push(Span::styled(
            agent_text,
            Style::default()
                .fg(Color::Black)
                .bg(agent_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Code mode badge.
    if app.code_mode {
        badge_spans.push(Span::raw(" "));
        badge_spans.push(Span::styled(
            " CODE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Provider + model.
    badge_spans.push(sep.clone());
    badge_spans.push(Span::styled(
        format!("{} \u{203a} {}", provider_label, app.selected_model),
        Style::default().fg(Color::DarkGray),
    ));

    if app.is_streaming {
        // ── Streaming status bar ───────────────────────────────────────
        let bar_width: usize = 8;
        let bar_pos = (app.thinking_frame / 3) % (bar_width * 2);
        let mut progress = String::new();
        for i in 0..bar_width {
            let dist = if bar_pos < bar_width {
                (bar_pos as i32 - i as i32).unsigned_abs() as usize
            } else {
                ((bar_width * 2 - bar_pos) as i32 - i as i32).unsigned_abs() as usize
            };
            progress.push(match dist {
                0 => '\u{2588}',
                1 => '\u{2593}',
                2 => '\u{2592}',
                _ => '\u{2591}',
            });
        }

        let word_count = app.streaming_response.split_whitespace().count();
        let approx_tokens = word_count * 4 / 3;
        let elapsed_secs = app
            .streaming_start
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let tok_per_sec = if elapsed_secs > 0.1 {
            approx_tokens as f64 / elapsed_secs
        } else {
            0.0
        };

        let spinner_frames = ["\u{25dc}", "\u{25dd}", "\u{25de}", "\u{25df}"];
        let spinner = spinner_frames[(app.thinking_frame / 4) % 4];

        let mut spans = vec![];
        // Left: badges
        spans.extend(badge_spans);
        spans.push(sep.clone());
        // Streaming indicator
        spans.extend_from_slice(&[
            Span::styled(
                format!("{spinner} Streaming... "),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(progress, Style::default().fg(Color::Green)),
            sep.clone(),
            Span::styled(
                format!("~{approx_tokens} tokens"),
                Style::default().fg(Color::Cyan),
            ),
            sep.clone(),
            Span::styled(
                format_elapsed(elapsed_secs),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(
                format!("{tok_per_sec:.0} tok/s"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    } else {
        // ── Idle status bar ────────────────────────────────────────────

        // Context-aware status/hint message.
        let status_span = if let Some(ref msg) = app.status_message {
            let msg_style = if msg.starts_with("Error")
                || msg.starts_with("Blocked")
                || msg.starts_with("Failed")
                || msg.starts_with("Cannot")
                || msg.starts_with("Unable")
                || msg.starts_with("Invalid")
                || msg.contains("error:")
            {
                Style::default().fg(theme.error)
            } else if msg.starts_with("Saved")
                || msg.starts_with("Copied")
                || msg.starts_with("Exported")
                || msg.starts_with("Running")
                || msg.contains("success")
            {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.warning)
            };
            Span::styled(format!(" {msg}"), msg_style)
        } else if app.input_mode == InputMode::Insert && app.input.starts_with('/') {
            Span::styled(
                " Tab: accept | \u{2191}\u{2193}: navigate | Enter: send | Try /help, /agent, /mode",
                Style::default().fg(theme.dim),
            )
        } else if app.input_mode == InputMode::Normal {
            let cwd_hint = if app.code_mode {
                let dir = app.working_dir.as_deref().unwrap_or(".");
                let short = std::path::Path::new(dir)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(dir);
                format!(" [{short}]")
            } else {
                String::new()
            };
            Span::styled(
                format!(
                    "{cwd_hint} j/k: scroll | g/G: top/bottom | /: commands | i: insert | ?: help"
                ),
                Style::default().fg(theme.dim),
            )
        } else if app.agent_mode {
            Span::styled(
                " Agent mode \u{2500} describe what to build, fix, or change",
                Style::default().fg(theme.dim),
            )
        } else {
            Span::styled(
                " Ready \u{2500} type a message, / for commands, @ to attach files",
                Style::default().fg(theme.dim),
            )
        };

        // Token/word count for the conversation.
        let total_tokens: usize = app
            .current_conversation()
            .messages
            .iter()
            .map(|(_, content)| content.len() / 4 + 1)
            .sum();
        let tokens_display = format_number(total_tokens);
        let context_limit = crate::agent::context::ContextManager::effective_limit(
            &app.selected_provider,
            app.context_limit_override,
        );
        let pct = if context_limit > 0 {
            (total_tokens as f64 / context_limit as f64 * 100.0).min(100.0) as u8
        } else {
            0
        };
        let token_color = match pct {
            0..=49 => Color::Green,
            50..=74 => Color::Yellow,
            _ => Color::Red,
        };

        // Right section: conversation position + token usage.
        let right_text = format!(
            " ~{} tokens ({}%) \u{2502} Conv {}/{} ",
            tokens_display,
            pct,
            app.active_conversation + 1,
            app.conversations.len(),
        );
        let right_span = Span::styled(right_text.clone(), Style::default().fg(token_color));
        let right_width = display_width(&right_text) as u16;

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(right_width)])
            .split(area);

        // Left side: badges + status + scroll indicator + cost.
        let mut left_spans: Vec<Span<'_>> = Vec::new();
        left_spans.extend(badge_spans);

        // Scroll indicator.
        if app.scroll_offset > 0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                format!("\u{2191}{}", app.scroll_offset),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Cost badge for paid providers.
        if app.usage_stats.estimated_cost_usd > 0.0 {
            left_spans.push(sep.clone());
            left_spans.push(Span::styled(
                app.usage_stats.format_cost(),
                Style::default().fg(Color::Yellow),
            ));
        }

        left_spans.push(sep.clone());
        left_spans.push(status_span);

        frame.render_widget(Paragraph::new(Line::from(left_spans)), chunks[0]);
        frame.render_widget(
            Paragraph::new(Line::from(right_span)).alignment(Alignment::Right),
            chunks[1],
        );
    }
}

/// Format a number with thousands separators.
fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!(
            "{},{:03},{:03}",
            n / 1_000_000,
            (n / 1_000) % 1_000,
            n % 1_000
        )
    } else if n >= 1_000 {
        format!("{},{:03}", n / 1_000, n % 1_000)
    } else {
        format!("{n}")
    }
}

/// Format elapsed seconds into a compact human-readable string.
fn format_elapsed(secs: f64) -> String {
    if secs < 1.0 {
        return "<1s".into();
    }
    if secs < 60.0 {
        return format!("{secs:.0}s");
    }
    let mins = (secs / 60.0).floor() as u64;
    let remaining = (secs % 60.0).floor() as u64;
    format!("{mins}m{remaining:02}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_number ───────────────────────────────────────────────

    #[test]
    fn format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(42), "42");
    }

    #[test]
    fn format_number_exactly_1000() {
        assert_eq!(format_number(1000), "1,000");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1_234), "1,234");
    }

    #[test]
    fn format_number_hundred_thousands() {
        assert_eq!(format_number(123_456), "123,456");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(1_234_567), "1,234,567");
    }

    #[test]
    fn format_number_exactly_million() {
        assert_eq!(format_number(1_000_000), "1,000,000");
    }

    // ── format_elapsed ──────────────────────────────────────────────

    #[test]
    fn format_elapsed_sub_second() {
        assert_eq!(format_elapsed(0.5), "<1s");
    }

    #[test]
    fn format_elapsed_zero() {
        assert_eq!(format_elapsed(0.0), "<1s");
    }

    #[test]
    fn format_elapsed_seconds() {
        assert_eq!(format_elapsed(30.0), "30s");
    }

    #[test]
    fn format_elapsed_one_second() {
        assert_eq!(format_elapsed(1.0), "1s");
    }

    #[test]
    fn format_elapsed_minutes() {
        assert_eq!(format_elapsed(90.0), "1m30s");
    }

    #[test]
    fn format_elapsed_exact_minute() {
        assert_eq!(format_elapsed(60.0), "1m00s");
    }

    #[test]
    fn format_elapsed_large() {
        assert_eq!(format_elapsed(3661.0), "61m01s");
    }
}
