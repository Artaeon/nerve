use chrono::Utc;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use super::markdown::{count_wrapped_rows, parse_assistant_content};
use super::utils::sanitize_display;
use crate::app::App;

// ── Public entry point ───────────────────────────────────────────────────────

/// Render the chat message history (and any in-flight streaming response) into
/// the given area.
pub fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let conversation = app.current_conversation();

    // ── Build styled lines from conversation history ──────────────────
    let estimated_lines = conversation.messages.len() * 5 + 20;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(estimated_lines);

    // Show the welcome screen until there's a real exchange. Check for
    // user/assistant turns rather than `messages.is_empty()`: on startup a
    // workspace system prompt is seeded at index 0, so `is_empty()` would be
    // false and the welcome would never appear on the common in-project first run.
    let has_conversation = conversation
        .messages
        .iter()
        .any(|(role, _)| role == "user" || role == "assistant");
    if !has_conversation && !app.is_streaming {
        // Empty state — show a branded welcome screen.
        // Use a compact variant for narrow terminals (< 60 cols).
        let section_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let key_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);

        if area.width < 60 {
            // Compact welcome for narrow terminals
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Nerve — AI for your terminal",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Type a message or press Ctrl+K for prompts",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        } else {
            // Full welcome screen with ASCII art
            let box_fg = Style::default().fg(Color::DarkGray);
            let art_style = Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
            let tagline_style = Style::default().fg(Color::DarkGray);

            lines.push(Line::from(""));

            // Box top
            lines.push(Line::from(Span::styled(
            "   \u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}",
            box_fg,
        )));
            lines.push(Line::from(Span::styled(
                "   \u{2502}                                                      \u{2502}",
                box_fg,
            )));

            // ASCII art lines inside box — each line has box borders
            let ascii_art = [
                "      \u{2588}\u{2588}\u{2588}\u{2557}   \u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2557}   \u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}",
                "      \u{2588}\u{2588}\u{2588}\u{2588}\u{2557}  \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2551}   \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}",
                "      \u{2588}\u{2588}\u{2554}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255d}\u{2588}\u{2588}\u{2551}   \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}  ",
                "      \u{2588}\u{2588}\u{2551}\u{255a}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{255d}  \u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{255a}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2554}\u{255d}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{255d}  ",
                "      \u{2588}\u{2588}\u{2551} \u{255a}\u{2588}\u{2588}\u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2551}  \u{2588}\u{2588}\u{2551} \u{255a}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255d} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}",
                "      \u{255a}\u{2550}\u{255d}  \u{255a}\u{2550}\u{2550}\u{2550}\u{255d}\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}\u{255a}\u{2550}\u{255d}  \u{255a}\u{2550}\u{255d}  \u{255a}\u{2550}\u{2550}\u{2550}\u{255d}  \u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}",
            ];

            for art_line in &ascii_art {
                lines.push(Line::from(vec![
                    Span::styled("   \u{2502}", box_fg),
                    Span::styled(art_line.to_string(), art_style),
                    Span::styled("  \u{2502}", box_fg),
                ]));
            }

            lines.push(Line::from(Span::styled(
                "   \u{2502}                                                      \u{2502}",
                box_fg,
            )));
            // Tagline
            lines.push(Line::from(vec![
                Span::styled("   \u{2502}", box_fg),
                Span::styled(
                    "          Raw AI power in your terminal           ",
                    tagline_style,
                ),
                Span::styled("\u{2502}", box_fg),
            ]));
            lines.push(Line::from(Span::styled(
                "   \u{2502}                                                      \u{2502}",
                box_fg,
            )));
            // Box bottom
            lines.push(Line::from(Span::styled(
            "   \u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
            box_fg,
        )));

            lines.push(Line::from(""));

            // Quick Start section
            lines.push(Line::from(Span::styled("   Quick Start", section_style)));
            lines.push(Line::from(Span::styled(
            "   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            box_fg,
        )));
            lines.push(Line::from(Span::styled(
                "   Type a message and press Enter to chat",
                desc_style,
            )));
            lines.push(Line::from(""));

            // Keyboard shortcuts
            let shortcuts = [
                ("Ctrl+K", "Nerve Bar (command palette)"),
                ("Ctrl+T", "Switch provider"),
                ("Ctrl+M", "Switch model"),
                ("Ctrl+P", "Browse prompts"),
                ("Ctrl+O", "History browser"),
                ("Ctrl+B", "Clipboard manager"),
                ("Ctrl+F", "Search in conversation"),
                ("Ctrl+H", "Help & keybindings"),
                ("1-9", "Copy message to clipboard"),
            ];
            for (key, desc) in &shortcuts {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {key:<10}"), key_style),
                    Span::styled(desc.to_string(), desc_style),
                ]));
            }

            lines.push(Line::from(""));

            // Slash commands
            let commands = [
                ("/help", "Show all slash commands"),
                ("/agent", "Toggle coding agent (auto-enables in projects)"),
                ("/mode", "Switch smart mode"),
                ("/usage", "Show API cost & token usage"),
                ("/branch save", "Save conversation branch"),
                ("/url", "Scrape a webpage for context"),
                ("/kb", "Manage knowledge base"),
                ("/auto", "Run automations"),
            ];
            for (cmd, desc) in &commands {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {cmd:<15}"), key_style),
                    Span::styled(desc.to_string(), desc_style),
                ]));
            }

            // Getting Started tips
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "   Getting Started",
                section_style,
            )));
            lines.push(Line::from(Span::styled(
                "   \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            // Tip 1
            lines.push(Line::from(vec![
                Span::styled(
                    "   1. ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Just describe a task \u{2014} the agent activates automatically:",
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "      \"fix the failing login test\"  (reads, edits & runs code for you)",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            // Tip 2
            lines.push(Line::from(vec![
                Span::styled(
                    "   2. ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Use @file in messages to auto-include code:",
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "      \"review @src/auth.rs for security issues\"",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            // Tip 3
            lines.push(Line::from(vec![
                Span::styled(
                    "   3. ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Run tests and ask about failures:",
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "      /test  then \"why did test_auth fail?\"",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            // Tip 4
            lines.push(Line::from(vec![
                Span::styled(
                    "   4. ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Add extra file context when you want it:",
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "      /file src/main.rs  or keep the agent off with /agent off",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            // Tip 5
            lines.push(Line::from(vec![
                Span::styled(
                    "   5. ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Use /mode to optimize for your use case:",
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                "      /mode efficient (saves tokens) | /mode thorough (detailed)",
                Style::default().fg(Color::DarkGray),
            )));

            if let Some(ref ws) = app.detected_workspace {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("   Project", section_style)));
                lines.push(Line::from(Span::styled(
                    format!("   {ws}"),
                    Style::default().fg(Color::White),
                )));
            }

            lines.push(Line::from(""));
        } // end else (full welcome)
    }

    let gutter = Span::styled(" \u{2502} ", Style::default().fg(Color::Rgb(50, 50, 60)));
    // Thin dim separator that spans the usable chat width (minus block borders
    // and horizontal padding = 4 columns).
    let sep_width = area.width.saturating_sub(4) as usize;
    let separator_str = "\u{2500}".repeat(sep_width);
    let separator_line = Line::from(Span::styled(
        separator_str.clone(),
        Style::default()
            .fg(Color::Rgb(50, 50, 60))
            .add_modifier(Modifier::DIM),
    ));
    // Indented separator reused for user messages and streaming header.
    let user_separator_line = Line::from(Span::styled(
        format!("   {separator_str}"),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));

    let msg_count = conversation.messages.len();
    let now = Utc::now();

    for (idx, (role, content)) in conversation.messages.iter().enumerate() {
        // Subtle separator between messages (not before the first one).
        if idx > 0 {
            lines.push(separator_line.clone());
            lines.push(Line::from("")); // breathing room after separator
        }

        // Blank line for breathing room.
        lines.push(Line::from(""));

        // Compute relative timestamp
        let time_ago = format_time_ago(now, conversation.created_at, idx, msg_count);

        // Message number counting from bottom (1 = last message)
        let msg_number = msg_count - idx;
        let number_badge = if msg_number <= 9 {
            Span::styled(
                format!(" {msg_number} "),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled("   ", Style::default())
        };

        match role.as_str() {
            "user" => {
                // Thin separator before user message (precomputed).
                lines.push(user_separator_line.clone());

                // Header: number badge + role badge + timestamp
                let mut header_spans = vec![];
                if msg_number <= 9 {
                    header_spans.push(Span::styled(
                        format!(" {msg_number} "),
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    header_spans.push(Span::raw("   "));
                }
                header_spans.push(Span::styled(
                    " You ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ));
                header_spans.push(Span::raw("  "));
                header_spans.push(Span::styled(time_ago, Style::default().fg(Color::DarkGray)));
                lines.push(Line::from(header_spans));

                // Content with accent bar (control chars stripped so a pasted
                // escape sequence can't manipulate the terminal).
                for text_line in content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("   \u{2502} ", Style::default().fg(Color::Blue)),
                        Span::styled(
                            sanitize_display(text_line),
                            Style::default().fg(Color::White),
                        ),
                    ]));
                }
            }
            "assistant" => {
                // Header with badge and timestamp
                lines.push(Line::from(vec![
                    gutter.clone(),
                    number_badge.clone(),
                    Span::styled(
                        "  AI  ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(time_ago, Style::default().fg(Color::DarkGray)),
                ]));
                // Body — with markdown + syntax highlighting, guttered
                let content_lines = parse_assistant_content(content);
                for cl in content_lines {
                    let mut spans = vec![gutter.clone()];
                    spans.extend(cl.spans);
                    lines.push(Line::from(spans));
                }
            }
            _ => {
                // System messages — very subtle, truncated
                lines.push(Line::from(vec![
                    Span::styled("   \u{2014} ", Style::default().fg(Color::Rgb(100, 90, 50))),
                    Span::styled(
                        sanitize_display(&content.chars().take(100).collect::<String>()),
                        Style::default()
                            .fg(Color::Rgb(100, 90, 50))
                            .add_modifier(Modifier::DIM | Modifier::ITALIC),
                    ),
                    if content.len() > 100 {
                        Span::styled("...", Style::default().fg(Color::Rgb(80, 70, 40)))
                    } else {
                        Span::raw("")
                    },
                ]));
            }
        }
    }

    // ── Currently-streaming response ────────────────────────────────────
    if app.is_streaming && !app.streaming_response.is_empty() {
        if !conversation.messages.is_empty() {
            lines.push(separator_line.clone());
            lines.push(Line::from("")); // breathing room after separator
        }
        lines.push(Line::from(""));

        // Separator (precomputed).
        lines.push(user_separator_line.clone());

        // Streaming header with animated spinner and word counter
        let spinner_frames = ["\u{25dc}", "\u{25dd}", "\u{25de}", "\u{25df}"]; // ◜ ◝ ◞ ◟
        let spinner = spinner_frames[(app.thinking_frame / 4) % 4];
        let word_count = app.streaming_response.split_whitespace().count();

        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(format!("  {spinner} "), Style::default().fg(Color::Green)),
            Span::styled(
                " AI ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  streaming... ({word_count} words)"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));

        // Apply the same markdown/code-block rendering to streaming content.
        let stream_lines = parse_assistant_content(&app.streaming_response);
        let stream_line_count = stream_lines.len();
        for (si, sl) in stream_lines.into_iter().enumerate() {
            let mut spans = vec![gutter.clone()];
            spans.extend(sl.spans);
            // Append a pulsing cursor to the very last line of content.
            if si == stream_line_count - 1 {
                // Pulse the cursor colour between bright green and dim green
                let cursor_color = if (app.thinking_frame / 6) % 2 == 0 {
                    Color::Green
                } else {
                    Color::Rgb(0, 160, 0)
                };
                spans.push(Span::styled(
                    "\u{258c}",
                    Style::default()
                        .fg(cursor_color)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(spans));
        }
    } else if app.is_streaming {
        // Streaming started but no tokens yet — animated thinking indicator.
        if !conversation.messages.is_empty() {
            lines.push(separator_line.clone());
            lines.push(Line::from("")); // breathing room after separator
        }
        lines.push(Line::from(""));

        // Animated dots: cycles through growing and shrinking dots
        let frame = (app.thinking_frame / 8) % 12; // change every 8 frames (~400ms at 50ms poll)

        let thinking_anim = match frame {
            0 => "  Thinking",
            1 => "  Thinking.",
            2 => "  Thinking..",
            3 => "  Thinking...",
            4 => "  Thinking....",
            5 => "  Thinking.....",
            6 | 7 => "  Thinking......",
            8 => "  Thinking.....",
            9 => "  Thinking....",
            10 => "  Thinking...",
            11 => "  Thinking..",
            _ => "  Thinking.",
        };

        // Animated spinner character: ◜ ◝ ◞ ◟
        let spinner_frames = ["\u{25dc}", "\u{25dd}", "\u{25de}", "\u{25df}"];
        let spinner = spinner_frames[(app.thinking_frame / 4) % 4];

        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(
                format!("  {spinner} "),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " AI ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(
                thinking_anim.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    // Trailing padding so last message isn't glued to the input area.
    lines.push(Line::from(""));

    // ── Compute scroll ──────────────────────────────────────────────────
    // We need the WRAPPED line count, not the logical line count, because
    // Paragraph::scroll operates on wrapped rows when Wrap is enabled. We
    // accumulate in usize (a long conversation can exceed u16) and saturate
    // to u16 only at the end — summing into a u16 directly overflow-panics in
    // debug builds and silently wraps in release.
    let content_width = area.width.saturating_sub(4) as usize; // borders + padding
    let total_rows: usize = lines
        .iter()
        .map(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            count_wrapped_rows(&text, content_width)
        })
        .sum();
    let total_lines: u16 = total_rows.min(u16::MAX as usize) as u16;

    let visible_lines = area.height.saturating_sub(2); // account for block borders
    let max_scroll = total_lines.saturating_sub(visible_lines);
    // Record the max so App::scroll_up / scroll_to_top can clamp the offset
    // (the renderer only has &App, hence the Cell).
    app.last_max_scroll.set(max_scroll);

    let scroll_y = if app.scroll_offset == 0 {
        // Auto-scroll to bottom — always show latest content.
        max_scroll
    } else {
        // User has scrolled up — respect their position.
        max_scroll.saturating_sub(app.scroll_offset)
    };

    let chat_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    let paragraph = Paragraph::new(lines)
        .block(chat_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, area);
}

// ── Time formatting ─────────────────────────────────────────────────────────

/// Produce a human-friendly relative timestamp like "2m ago", "1h ago", etc.
///
/// Since individual messages don't carry their own timestamps, we estimate
/// based on conversation creation time and message index within the conversation.
pub fn format_time_ago(
    now: chrono::DateTime<chrono::Utc>,
    conv_created: chrono::DateTime<chrono::Utc>,
    msg_index: usize,
    total_messages: usize,
) -> String {
    let total_secs = now.signed_duration_since(conv_created).num_seconds().max(0);

    // Spread messages evenly across the conversation's lifetime.
    let msg_secs = if total_messages <= 1 {
        total_secs
    } else {
        let fraction = msg_index as f64 / (total_messages as f64 - 1.0).max(1.0);
        (total_secs as f64 * fraction) as i64
    };
    let ago_secs = (total_secs - msg_secs).max(0);

    if ago_secs < 60 {
        "just now".to_string()
    } else if ago_secs < 3600 {
        format!("{}m ago", ago_secs / 60)
    } else if ago_secs < 86400 {
        format!("{}h ago", ago_secs / 3600)
    } else {
        format!("{}d ago", ago_secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    // ── format_time_ago tests ───────────────────────────────────────────

    #[test]
    fn format_time_ago_just_now() {
        let now = Utc::now();
        let result = format_time_ago(now, now, 0, 1);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_time_ago_minutes() {
        let now = Utc::now();
        let created = now - Duration::minutes(10);
        // For a multi-message conversation, the first message (index 0) should
        // show the full elapsed time since it gets fraction=0 of the timeline.
        let result = format_time_ago(now, created, 0, 3);
        assert!(result.contains("m ago"), "expected minutes, got: {result}");
    }

    #[test]
    fn format_time_ago_first_message_in_multi() {
        let now = Utc::now();
        let created = now - Duration::hours(2);
        let result = format_time_ago(now, created, 0, 5);
        assert!(
            result.contains("h ago"),
            "first message should show hours ago, got: {result}"
        );
    }

    #[test]
    fn format_time_ago_last_message_in_multi() {
        let now = Utc::now();
        let created = now - Duration::hours(2);
        let result = format_time_ago(now, created, 4, 5);
        assert_eq!(result, "just now");
    }
}
