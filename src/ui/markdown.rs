use std::sync::LazyLock;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};

use super::utils::{display_width, sanitize_display};

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
pub fn parse_assistant_content(content: &str) -> Vec<Line<'static>> {
    let ss = &*SYNTAX_SET;
    let theme = &*THEME;

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer: Vec<String> = Vec::new();

    for raw_line in content.lines() {
        // Strip control chars / ANSI escapes from assistant output before it
        // reaches the terminal (prevents escape-sequence injection).
        let sanitized = sanitize_display(raw_line);
        let text_line = sanitized.as_str();
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
                code_lang = text_line.trim_start_matches('`').trim().to_string();
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
    let lang_badge = if lang.is_empty() || lang == "text" {
        String::new()
    } else {
        format!("  {lang}  ")
    };
    let remaining = 40usize.saturating_sub(lang_badge.len() + 5);

    Line::from(vec![
        Span::styled(
            "   \u{256d}\u{2500}",
            Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
        ),
        Span::styled(
            lang_badge,
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Rgb(40, 40, 56))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "\u{2500}".repeat(remaining),
            Style::default().fg(CODE_CHROME_FG).bg(CODE_BG),
        ),
    ])
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
pub fn highlight_code(code: &str, lang: &str, ss: &SyntaxSet, theme: &Theme) -> Vec<Line<'static>> {
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
        spans.push(Span::styled("   \u{2502}", pipe_style));
        spans.push(Span::styled(format!("{:>4}", line_idx + 1), line_num_style));
        spans.push(Span::styled(" \u{2502} ", pipe_style));

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
pub fn format_markdown_line(text_line: &str) -> Line<'static> {
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
    if (trimmed == "---" || trimmed == "***" || trimmed == "___") && trimmed.len() >= 3 {
        return Line::from(Span::styled(
            format!("   {}", "\u{2500}".repeat(40)),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    // ── Unordered list items: `- item` or `* item` ──────────────────
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
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
pub fn try_strip_header(s: &str) -> Option<&str> {
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
pub fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
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
                    if text[link_text_end..].starts_with("](")
                        && let Some(close_paren) = text[link_text_end + 2..].find(')')
                    {
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
                            format!(" ({url})"),
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

// ── Wrapped-row estimation ───────────────────────────────────────────────────

/// Estimate how many terminal rows a logical line occupies under ratatui's
/// word wrapping (`Wrap { trim: false }`): greedy by word, a word that would
/// overflow the current row starts a new row, and a word wider than the whole
/// row is hard-broken. This matches real word-wrap far better than naive
/// character packing (which *under*counts prose and lets the newest streamed
/// lines fall off the bottom of the viewport).
// `col` is a loop-carried accumulator; its write on the final iteration is
// intentionally discarded (we only return `rows`).
#[allow(unused_assignments)]
pub fn count_wrapped_rows(text: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut col = 0usize;
    for (i, word) in text.split(' ').enumerate() {
        // Every space except the leading one occupies a cell (trim:false).
        let sep = usize::from(i > 0);
        let need = display_width(word) + sep;
        if col + need <= width {
            col += need;
            continue;
        }
        // Doesn't fit. Move to a fresh row only if the current one has content
        // (a word starting at column 0 must not add a spurious wrap row).
        let w = display_width(word);
        if col > 0 {
            rows += 1;
            col = 0;
        }
        if w <= width {
            col = w;
        } else {
            // Word wider than a full row is hard-broken across extra rows.
            let extra = (w - 1) / width;
            rows += extra;
            col = w - extra * width;
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    // ── count_wrapped_rows tests ────────────────────────────────────────

    #[test]
    fn count_wrapped_rows_basics() {
        assert_eq!(count_wrapped_rows("", 10), 1);
        assert_eq!(count_wrapped_rows("hello", 10), 1);
        assert_eq!(count_wrapped_rows("hello", 0), 1); // width 0 → no div-by-zero
        // "aaaa bbbb cccc" with width 9: "aaaa bbbb"(9) then "cccc" → 2 rows.
        assert_eq!(count_wrapped_rows("aaaa bbbb cccc", 9), 2);
        // A word longer than the width is hard-broken: 25 chars / width 10 = 3 rows.
        assert_eq!(count_wrapped_rows(&"a".repeat(25), 10), 3);
    }

    #[test]
    fn count_wrapped_rows_word_wrap_not_undercount() {
        // Char-packing would say ceil(11/6)=2, but word wrap needs 3 rows:
        // "ab cd" (5) | "ef gh" (5) | "ij" — words don't split mid-word.
        let rows = count_wrapped_rows("ab cd ef gh ij", 6);
        assert!(rows >= 3, "word wrap should not undercount, got {rows}");
    }

    #[test]
    fn count_wrapped_rows_huge_input_no_overflow() {
        // Regression: summing wrapped rows into a u16 overflowed (debug panic /
        // release wrap). The accumulator is usize now; this just must not hang
        // or panic and must report a large row count.
        let huge = "x ".repeat(100_000); // 100k words
        let rows = count_wrapped_rows(&huge, 1);
        assert!(rows > u16::MAX as usize);
    }

    // ── highlight_code tests ────────────────────────────────────────────

    #[test]
    fn highlight_code_detects_rust() {
        let ss = &*SYNTAX_SET;
        let theme = &*THEME;
        let lines = highlight_code("fn main() {}", "rust", ss, theme);
        assert!(!lines.is_empty());
    }

    #[test]
    fn highlight_code_unknown_language() {
        let ss = &*SYNTAX_SET;
        let theme = &*THEME;
        let lines = highlight_code("some text", "unknown_lang_xyz", ss, theme);
        assert!(!lines.is_empty());
    }

    #[test]
    fn highlight_code_empty() {
        let ss = &*SYNTAX_SET;
        let theme = &*THEME;
        let lines = highlight_code("", "rust", ss, theme);
        assert!(lines.is_empty());
    }

    #[test]
    fn highlight_code_multiline() {
        let ss = &*SYNTAX_SET;
        let theme = &*THEME;
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let lines = highlight_code(code, "rust", ss, theme);
        assert_eq!(lines.len(), 3);
    }

    // ── format_markdown_line tests ──────────────────────────────────────

    #[test]
    fn format_markdown_line_header() {
        let line = format_markdown_line("# Hello World");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("Hello World"));
    }

    #[test]
    fn format_markdown_line_bullet() {
        let line = format_markdown_line("- item one");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("item one"));
    }

    #[test]
    fn format_markdown_line_blockquote() {
        let line = format_markdown_line("> quoted text");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("quoted text"));
    }

    #[test]
    fn format_markdown_line_horizontal_rule() {
        let line = format_markdown_line("---");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains('\u{2500}'));
    }

    #[test]
    fn format_markdown_line_ordered_list() {
        let line = format_markdown_line("1. first item");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("first item"));
    }

    #[test]
    fn format_markdown_line_plain_text() {
        let line = format_markdown_line("just regular text");
        let content: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(content.contains("just regular text"));
    }

    // ── try_strip_header tests ──────────────────────────────────────────

    #[test]
    fn try_strip_header_h1() {
        assert_eq!(try_strip_header("# Title"), Some("Title"));
    }

    #[test]
    fn try_strip_header_h2() {
        assert_eq!(try_strip_header("## Subtitle"), Some("Subtitle"));
    }

    #[test]
    fn try_strip_header_h6() {
        assert_eq!(try_strip_header("###### Deep"), Some("Deep"));
    }

    #[test]
    fn try_strip_header_too_many_hashes() {
        assert_eq!(try_strip_header("####### Not valid"), None);
    }

    #[test]
    fn try_strip_header_no_space() {
        assert_eq!(try_strip_header("#nospace"), None);
    }

    #[test]
    fn try_strip_header_not_a_header() {
        assert_eq!(try_strip_header("regular text"), None);
    }

    // ── parse_inline_spans tests ────────────────────────────────────────

    #[test]
    fn parse_inline_spans_plain() {
        let spans = parse_inline_spans("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn parse_inline_spans_bold() {
        let spans = parse_inline_spans("before **bold** after");
        assert!(
            spans.len() >= 3,
            "expected at least 3 spans, got {}",
            spans.len()
        );
        let bold_span = spans.iter().find(|s| s.content.as_ref() == "bold");
        assert!(bold_span.is_some(), "should have a 'bold' span");
        assert!(
            bold_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn parse_inline_spans_italic() {
        let spans = parse_inline_spans("before *italic* after");
        let italic_span = spans.iter().find(|s| s.content.as_ref() == "italic");
        assert!(italic_span.is_some(), "should have an 'italic' span");
        assert!(
            italic_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn parse_inline_spans_inline_code() {
        let spans = parse_inline_spans("use `code` here");
        let code_span = spans.iter().find(|s| s.content.contains("code"));
        assert!(code_span.is_some(), "should have an inline code span");
        assert_eq!(code_span.unwrap().style.fg, Some(INLINE_CODE_FG));
    }

    #[test]
    fn parse_inline_spans_link() {
        let spans = parse_inline_spans("see [docs](https://example.com) here");
        let link_span = spans.iter().find(|s| s.content.as_ref() == "docs");
        assert!(link_span.is_some(), "should find the link text");
        assert!(
            link_span
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn parse_inline_spans_unmatched_bold() {
        let spans = parse_inline_spans("some **unmatched text");
        assert!(!spans.is_empty());
    }

    #[test]
    fn parse_inline_spans_empty() {
        let spans = parse_inline_spans("");
        assert!(spans.is_empty());
    }

    // ── parse_assistant_content tests ───────────────────────────────────

    #[test]
    fn parse_assistant_content_plain_text() {
        let lines = parse_assistant_content("Hello world");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn parse_assistant_content_code_block() {
        let content = "before\n```rust\nfn main() {}\n```\nafter";
        let lines = parse_assistant_content(content);
        assert!(lines.len() >= 4, "expected >= 4 lines, got {}", lines.len());
    }

    #[test]
    fn parse_assistant_content_unclosed_code_block() {
        let content = "text\n```python\ndef foo():\n    pass";
        let lines = parse_assistant_content(content);
        assert!(!lines.is_empty());
    }
}
