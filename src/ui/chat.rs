use std::sync::LazyLock;

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
/// Inline code colours.
const INLINE_CODE_FG: Color = Color::Cyan;
const INLINE_CODE_BG: Color = Color::Rgb(40, 40, 55);

// ── Public entry point ───────────────────────────────────────────────────────

/// Render the chat message history (and any in-flight streaming response) into
/// the given area.
pub fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let conversation = app.current_conversation();

    // ── Build styled lines from conversation history ──────────────────
    let mut lines: Vec<Line<'_>> = Vec::new();

    if conversation.messages.is_empty() && !app.is_streaming {
        // Empty state — show a welcome message.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ⚡ Welcome to Nerve",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Start typing a message, or press Ctrl+K to open the Nerve Bar.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Press Ctrl+H for help with keybindings.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    for (role, content) in &conversation.messages {
        // Blank line between messages for breathing room.
        lines.push(Line::from(""));

        match role.as_str() {
            "user" => {
                // Header
                lines.push(Line::from(Span::styled(
                    " You >",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                // Body — plain white, no markdown processing
                for text_line in content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("   {text_line}"),
                        Style::default().fg(Color::White),
                    )));
                }
            }
            "assistant" => {
                // Header
                lines.push(Line::from(Span::styled(
                    " AI >",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
                // Body — with markdown + syntax highlighting
                lines.extend(parse_assistant_content(content));
            }
            _ => {
                // System / unknown role
                lines.push(Line::from(Span::styled(
                    format!(" {role} >"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                for text_line in content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("   {text_line}"),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }
    }

    // ── Currently-streaming response ────────────────────────────────────
    if app.is_streaming && !app.streaming_response.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " AI >",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        // Apply the same markdown/code-block rendering to streaming content.
        lines.extend(parse_assistant_content(&app.streaming_response));
        // Blinking cursor indicator at the end of the streaming text.
        lines.push(Line::from(Span::styled(
            "   \u{258c}", // ▌
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::SLOW_BLINK),
        )));
    } else if app.is_streaming {
        // Streaming started but no tokens yet.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " AI > thinking...",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::SLOW_BLINK),
        )));
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

/// Produces a header line like: `   ╭─ rust ──────────`
fn code_header_line(lang: &str) -> Line<'static> {
    let label = if lang.is_empty() {
        "code".to_string()
    } else {
        lang.to_string()
    };
    let bar = "─".repeat(40usize.saturating_sub(label.len() + 5));
    let text = format!("   ╭─ {label} {bar}");
    Line::from(Span::styled(
        text,
        Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
    ))
}

/// Produces a footer line: `   ╰────────────────────`
fn code_footer_line() -> Line<'static> {
    let bar = "─".repeat(40);
    let text = format!("   ╰{bar}");
    Line::from(Span::styled(
        text,
        Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
    ))
}

// ── Syntax highlighting ──────────────────────────────────────────────────────

/// Syntax-highlight a code block and return styled `Line`s.
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

    for line in code.lines() {
        let ranges = h.highlight_line(line, ss).unwrap_or_default();
        let mut spans: Vec<Span<'static>> = Vec::new();

        // 3-space indent to match message body alignment.
        spans.push(Span::styled(
            "   ",
            Style::default().bg(CODE_BG),
        ));

        for (style, text) in ranges {
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(
                text.to_string(),
                Style::default().fg(fg).bg(CODE_BG),
            ));
        }

        lines.push(Line::from(spans));
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
            // ── Inline code: `code` ──────────────────────────────
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
                        spans.push(Span::styled(
                            text[content_start..j].to_string(),
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
