use std::sync::LazyLock;

use chrono::Utc;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};

use crate::app::App;

// ── Lazily-initialised syntect resources (loaded once) ───────────────────────

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let ts = ThemeSet::load_defaults();
    ts.themes["base16-eighties.dark"].clone()
});

/// Background colour for code blocks — a subtle dark surface.
const CODE_BG: Color = Color::Rgb(30, 30, 46);
/// Foreground for code-block chrome (borders, language label).
const CODE_CHROME_FG: Color = Color::Rgb(100, 100, 120);
/// Inline code colours (brighter background so it pops).
const INLINE_CODE_FG: Color = Color::Cyan;
const INLINE_CODE_BG: Color = Color::Rgb(55, 55, 75);

// ── Public entry point ───────────────────────────────────────────────────────

/// Render the chat message history (and any in-flight streaming response) into
/// the given area.
pub fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let conversation = app.current_conversation();

    // ── Build styled lines from conversation history ──────────────────
    let mut lines: Vec<Line<'_>> = Vec::new();

    if conversation.messages.is_empty() && !app.is_streaming {
        // Empty state — show a rich branded welcome screen.
        let box_fg = Style::default().fg(Color::DarkGray);
        let art_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let tagline_style = Style::default().fg(Color::DarkGray);
        let section_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let key_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);

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
            Span::styled("          Raw AI power in your terminal           ", tagline_style),
            Span::styled("\u{2502}", box_fg),
        ]));
        lines.push(Line::from(Span::styled(
            "   \u{2502}                                                      \u{2502}",
            box_fg,
        )));
        // Box bottom
        lines.push(Line::from(Span::styled(
            "   \u{256e}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}",
            box_fg,
        )));

        lines.push(Line::from(""));

        // Quick Start section
        lines.push(Line::from(Span::styled(
            "   Quick Start",
            section_style,
        )));
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
                Span::styled(format!("   {:<10}", key), key_style),
                Span::styled(desc.to_string(), desc_style),
            ]));
        }

        lines.push(Line::from(""));

        // Slash commands
        let commands = [
            ("/help", "Show all slash commands"),
            ("/agent on", "Enable coding agent"),
            ("/usage", "Show API cost & token usage"),
            ("/branch save", "Save conversation branch"),
            ("/url", "Scrape a webpage for context"),
            ("/kb", "Manage knowledge base"),
            ("/auto", "Run automations"),
        ];
        for (cmd, desc) in &commands {
            lines.push(Line::from(vec![
                Span::styled(format!("   {:<15}", cmd), key_style),
                Span::styled(desc.to_string(), desc_style),
            ]));
        }

        lines.push(Line::from(""));
    }

    let gutter = Span::styled(
        " \u{2502} ",
        Style::default().fg(Color::Rgb(50, 50, 60)),
    );
    // Thin dim separator that spans the usable chat width (minus block borders
    // and horizontal padding = 4 columns).
    let sep_width = area.width.saturating_sub(4) as usize;
    let separator_line = Line::from(Span::styled(
        "\u{2500}".repeat(sep_width),
        Style::default()
            .fg(Color::Rgb(50, 50, 60))
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
                format!(" {} ", msg_number),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled("   ", Style::default())
        };

        match role.as_str() {
            "user" => {
                // Header with badge and timestamp
                lines.push(Line::from(vec![
                    gutter.clone(),
                    number_badge.clone(),
                    Span::styled(
                        "  You  ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        time_ago,
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                // Body — plain white, with gutter
                for text_line in content.lines() {
                    lines.push(Line::from(vec![
                        gutter.clone(),
                        Span::styled(
                            format!("  {text_line}"),
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
                    Span::styled(
                        time_ago,
                        Style::default().fg(Color::DarkGray),
                    ),
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
                // System / unknown role — subtle italic amber text.
                lines.push(Line::from(vec![
                    gutter.clone(),
                    number_badge.clone(),
                    Span::styled(
                        "  \u{2014} ",
                        Style::default()
                            .fg(Color::Rgb(180, 150, 60))
                            .add_modifier(Modifier::DIM),
                    ),
                    Span::styled(
                        time_ago,
                        Style::default()
                            .fg(Color::Rgb(100, 90, 50))
                            .add_modifier(Modifier::DIM),
                    ),
                ]));
                for text_line in content.lines() {
                    lines.push(Line::from(vec![
                        gutter.clone(),
                        Span::styled(
                            format!("  {text_line}"),
                            Style::default()
                                .fg(Color::Rgb(180, 150, 60))
                                .add_modifier(Modifier::DIM | Modifier::ITALIC),
                        ),
                    ]));
                }
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
        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(
                "  AI  ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("now", Style::default().fg(Color::DarkGray)),
        ]));
        // Apply the same markdown/code-block rendering to streaming content.
        let stream_lines = parse_assistant_content(&app.streaming_response);
        let stream_line_count = stream_lines.len();
        for (si, sl) in stream_lines.into_iter().enumerate() {
            let mut spans = vec![gutter.clone()];
            spans.extend(sl.spans);
            // Append the blinking cursor to the very last line of content.
            if si == stream_line_count - 1 {
                spans.push(Span::styled(
                    "\u{258c}",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::SLOW_BLINK),
                ));
            }
            lines.push(Line::from(spans));
        }
    } else if app.is_streaming {
        // Streaming started but no tokens yet — show "Thinking..." with animation.
        if !conversation.messages.is_empty() {
            lines.push(separator_line.clone());
            lines.push(Line::from("")); // breathing room after separator
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(
                "  AI  ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            gutter.clone(),
            Span::styled(
                "  Thinking...",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC | Modifier::SLOW_BLINK),
            ),
        ]));
    }

    // Trailing padding so last message isn't glued to the input area.
    lines.push(Line::from(""));

    // ── Compute scroll ──────────────────────────────────────────────────
    let total_lines = lines.len() as u16;
    let visible_lines = area.height.saturating_sub(2); // account for block borders
    let max_scroll = total_lines.saturating_sub(visible_lines);

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

// ── Markdown / code-block parser for assistant messages ──────────────────────

/// Parse assistant message content into styled `Line`s.
///
/// Handles:
/// - Fenced code blocks (with optional language hint) → syntax-highlighted
/// - Inline code → cyan on dark background
/// - Bold (`**text**`) → BOLD modifier
/// - Italic (`*text*`) → ITALIC modifier
/// - Headers (`# …`) → Bold + Cyan
/// - List items (`- …`) → indented bullet
fn parse_assistant_content(content: &str) -> Vec<Line<'static>> {
    let ss = &*SYNTAX_SET;
    let theme = &*THEME;

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer: Vec<String> = Vec::new();

    for text_line in content.lines() {
        if text_line.starts_with("```") {
            if in_code_block {
                // ── End of code block ────────────────────────────────
                let highlighted = highlight_code(&code_buffer.join("\n"), &code_lang, ss, theme);
                lines.extend(highlighted);
                lines.push(code_footer_line());
                code_buffer.clear();
                code_lang.clear();
                in_code_block = false;
            } else {
                // ── Start of code block ──────────────────────────────
                code_lang = text_line
                    .trim_start_matches('`')
                    .trim()
                    .to_string();
                in_code_block = true;
                lines.push(code_header_line(&code_lang));
            }
        } else if in_code_block {
            code_buffer.push(text_line.to_string());
        } else {
            // Regular text — apply basic markdown formatting.
            lines.push(format_markdown_line(text_line));
        }
    }

    // Handle unclosed code block (e.g. still streaming).
    if in_code_block && !code_buffer.is_empty() {
        let highlighted = highlight_code(&code_buffer.join("\n"), &code_lang, ss, theme);
        lines.extend(highlighted);
    }

    lines
}

// ── Code-block chrome ────────────────────────────────────────────────────────

/// Produces a header line like: `   ╭─  rust  ──────────`
fn code_header_line(lang: &str) -> Line<'static> {
    let label = if lang.is_empty() {
        "code".to_string()
    } else {
        lang.to_string()
    };
    // Padded label: `  rust  ` for nicer appearance.
    let padded_label = format!("  {label}  ");
    let bar = "\u{2500}".repeat(40usize.saturating_sub(padded_label.len() + 4));
    let text = format!("   \u{256d}\u{2500}{padded_label}{bar}");
    Line::from(Span::styled(
        text,
        Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
    ))
}

/// Produces a footer line: `   ╰────────────────────`
fn code_footer_line() -> Line<'static> {
    let bar = "\u{2500}".repeat(40);
    let text = format!("   \u{2570}{bar}");
    Line::from(Span::styled(
        text,
        Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
    ))
}

// ── Syntax highlighting ──────────────────────────────────────────────────────

/// Maximum number of lines to syntax-highlight in a single code block.
/// Beyond this limit lines are rendered as plain text for performance.
const MAX_HIGHLIGHT_LINES: usize = 200;

/// Syntax-highlight a code block and return styled `Line`s with line numbers.
fn highlight_code(
    code: &str,
    lang: &str,
    ss: &SyntaxSet,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let syntax = ss
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Handle the empty code block case.
    if code.is_empty() {
        return lines;
    }

    let line_num_style = Style::default().fg(Color::DarkGray).bg(CODE_BG);
    let pipe_style = Style::default().fg(Color::DarkGray).bg(CODE_BG);

    let total_lines = code.lines().count();

    for (line_idx, line) in code.lines().enumerate() {
        // Limit syntax highlighting to first MAX_HIGHLIGHT_LINES lines for
        // performance — syntect can be expensive on very large blocks.
        let ranges = if line_idx < MAX_HIGHLIGHT_LINES {
            h.highlight_line(line, ss).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Prefix: `   │ 1 │ ` — chrome + line number + pipe
        spans.push(Span::styled(
            "   \u{2502}",
            pipe_style,
        ));
        spans.push(Span::styled(
            format!("{:>4}", line_idx + 1),
            line_num_style,
        ));
        spans.push(Span::styled(
            " \u{2502} ",
            pipe_style,
        ));

        if line_idx < MAX_HIGHLIGHT_LINES {
            for (style, text) in ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(fg).bg(CODE_BG),
                ));
            }
        } else {
            // Past the highlight limit — render as plain text on code background.
            spans.push(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White).bg(CODE_BG),
            ));
        }

        lines.push(Line::from(spans));
    }

    // If truncated, add a note about unhighlighted lines.
    if total_lines > MAX_HIGHLIGHT_LINES {
        let note = format!(
            "   [... syntax highlighting skipped for {} lines above]",
            total_lines - MAX_HIGHLIGHT_LINES
        );
        lines.push(Line::from(Span::styled(
            note,
            Style::default()
                .fg(Color::DarkGray)
                .bg(CODE_BG)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    lines
}

// ── Basic markdown formatting for non-code text ──────────────────────────────

/// Format a single line of non-code assistant text with simple markdown support.
fn format_markdown_line(text_line: &str) -> Line<'static> {
    let trimmed = text_line.trim_start();

    // ── Headers: `# Heading`, `## Heading`, etc. ─────────────────────
    if let Some(rest) = try_strip_header(trimmed) {
        return Line::from(Span::styled(
            format!("   {rest}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // ── Blockquotes: `> text` ──────────────────────────────────────
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return Line::from(vec![
            Span::styled(
                "   \u{2502} ".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                rest.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]);
    }
    // Bare blockquote marker with no content
    if trimmed == ">" {
        return Line::from(Span::styled(
            "   \u{2502}".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // ── Horizontal rules: `---`, `***`, `___` ──────────────────────
    if (trimmed == "---" || trimmed == "***" || trimmed == "___")
        && trimmed.len() >= 3
    {
        return Line::from(Span::styled(
            format!("   {}", "\u{2500}".repeat(40)),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    // ── Unordered list items: `- item` or `* item` ──────────────────
    if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        let mut spans = vec![Span::styled(
            "   • ".to_string(),
            Style::default().fg(Color::DarkGray),
        )];
        spans.extend(parse_inline_spans(rest));
        return Line::from(spans);
    }

    // ── Ordered list items: `1. item` ───────────────────────────────
    if let Some(dot_pos) = trimmed.find(". ") {
        let prefix = &trimmed[..dot_pos];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            let rest = &trimmed[dot_pos + 2..];
            let mut spans = vec![Span::styled(
                format!("   {prefix}. "),
                Style::default().fg(Color::DarkGray),
            )];
            spans.extend(parse_inline_spans(rest));
            return Line::from(spans);
        }
    }

    // ── Regular paragraph text ──────────────────────────────────────
    let mut spans = vec![Span::styled("   ".to_string(), Style::default())];
    spans.extend(parse_inline_spans(trimmed));
    Line::from(spans)
}

/// If the line starts with a markdown header (`#`, `##`, …), return the text
/// portion (without the `#` markers and leading space).
fn try_strip_header(s: &str) -> Option<&str> {
    if !s.starts_with('#') {
        return None;
    }
    let hashes = s.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &s[hashes..];
    if rest.starts_with(' ') {
        Some(rest.trim_start())
    } else {
        // `#text` without a space is not a valid header.
        None
    }
}

// ── Inline span parser ───────────────────────────────────────────────────────

/// Parse inline markdown spans: **bold**, *italic*, `code`.
///
/// Uses a simple state-machine approach rather than full markdown parsing.
fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut plain_start = 0;

    while let Some(&(i, ch)) = chars.peek() {
        match ch {
            // ── Bold: **text** ───────────────────────────────────
            '*' if text[i..].starts_with("**") => {
                // Flush plain text accumulated so far.
                if i > plain_start {
                    spans.push(Span::styled(
                        text[plain_start..i].to_string(),
                        Style::default().fg(Color::White),
                    ));
                }
                // Skip opening **
                chars.next();
                chars.next();
                let content_start = i + 2;
                let mut found_end = false;
                while let Some(&(j, c2)) = chars.peek() {
                    if c2 == '*' && text[j..].starts_with("**") {
                        spans.push(Span::styled(
                            text[content_start..j].to_string(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ));
                        chars.next();
                        chars.next();
                        plain_start = j + 2;
                        found_end = true;
                        break;
                    }
                    chars.next();
                }
                if !found_end {
                    // Unmatched ** — treat as plain text.
                    spans.push(Span::styled(
                        text[i..].to_string(),
                        Style::default().fg(Color::White),
                    ));
                    return spans;
                }
            }
            // ── Italic: *text* ───────────────────────────────────
            '*' => {
                if i > plain_start {
                    spans.push(Span::styled(
                        text[plain_start..i].to_string(),
                        Style::default().fg(Color::White),
                    ));
                }
                chars.next(); // skip *
                let content_start = i + 1;
                let mut found_end = false;
                while let Some(&(j, c2)) = chars.peek() {
                    if c2 == '*' {
                        spans.push(Span::styled(
                            text[content_start..j].to_string(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::ITALIC),
                        ));
                        chars.next(); // skip closing *
                        plain_start = j + 1;
                        found_end = true;
                        break;
                    }
                    chars.next();
                }
                if !found_end {
                    spans.push(Span::styled(
                        text[i..].to_string(),
                        Style::default().fg(Color::White),
                    ));
                    return spans;
                }
            }
            // ── Links: [text](url) ──────────────────────────────
            '[' => {
                // Try to parse a markdown link: [text](url)
                if let Some(close_bracket) = text[i + 1..].find(']') {
                    let link_text_end = i + 1 + close_bracket;
                    if text[link_text_end..].starts_with("](") {
                        if let Some(close_paren) = text[link_text_end + 2..].find(')') {
                            let url_end = link_text_end + 2 + close_paren;
                            let link_text = &text[i + 1..link_text_end];
                            let url = &text[link_text_end + 2..url_end];
                            // Flush plain text accumulated so far.
                            if i > plain_start {
                                spans.push(Span::styled(
                                    text[plain_start..i].to_string(),
                                    Style::default().fg(Color::White),
                                ));
                            }
                            spans.push(Span::styled(
                                link_text.to_string(),
                                Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::UNDERLINED),
                            ));
                            spans.push(Span::styled(
                                format!(" ({})", url),
                                Style::default()
                                    .fg(Color::DarkGray)
                                    .add_modifier(Modifier::DIM),
                            ));
                            // Advance the char iterator past the entire link
                            while let Some(&(pos, _)) = chars.peek() {
                                if pos > url_end {
                                    break;
                                }
                                chars.next();
                            }
                            plain_start = url_end + 1;
                            continue;
                        }
                    }
                }
                // Not a valid link — treat as plain text
                chars.next();
            }
            // ── Inline code: `code` → ` code ` with padding ─────
            '`' => {
                if i > plain_start {
                    spans.push(Span::styled(
                        text[plain_start..i].to_string(),
                        Style::default().fg(Color::White),
                    ));
                }
                chars.next(); // skip opening `
                let content_start = i + 1;
                let mut found_end = false;
                while let Some(&(j, c2)) = chars.peek() {
                    if c2 == '`' {
                        // 1-space padding on each side for readability.
                        spans.push(Span::styled(
                            format!(" {} ", &text[content_start..j]),
                            Style::default().fg(INLINE_CODE_FG).bg(INLINE_CODE_BG),
                        ));
                        chars.next(); // skip closing `
                        plain_start = j + 1;
                        found_end = true;
                        break;
                    }
                    chars.next();
                }
                if !found_end {
                    spans.push(Span::styled(
                        text[i..].to_string(),
                        Style::default().fg(Color::White),
                    ));
                    return spans;
                }
            }
            _ => {
                chars.next();
            }
        }
    }

    // Flush remaining plain text.
    if plain_start < text.len() {
        spans.push(Span::styled(
            text[plain_start..].to_string(),
            Style::default().fg(Color::White),
        ));
    }

    spans
}

// ── Time formatting ─────────────────────────────────────────────────────────

/// Produce a human-friendly relative timestamp like "2m ago", "1h ago", etc.
///
/// Since individual messages don't carry their own timestamps, we estimate
/// based on conversation creation time and message index within the conversation.
fn format_time_ago(
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
