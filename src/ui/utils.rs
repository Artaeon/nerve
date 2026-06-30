use ratatui::layout::{Constraint, Direction, Layout, Rect};
use unicode_width::UnicodeWidthStr;

/// Calculate a centered rect within the given area using percentage dimensions.
#[allow(dead_code)]
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Calculate a centered rect with fixed dimensions.
pub fn centered_rect_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Calculate the display width of a string, accounting for wide Unicode characters
/// (e.g. CJK ideographs, emoji) that occupy two terminal columns.
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Strip control characters from text before it is rendered to the terminal.
///
/// Assistant messages, web-scraped content, file contents, and command output
/// can contain raw ANSI escape sequences (`\x1b[...`), carriage returns, BEL,
/// and other C0/C1 control codes. Passed straight through to the terminal they
/// can move the cursor, change colors, or clear the screen (ANSI-escape
/// injection). Callers split on `\n` first, so newlines are handled elsewhere;
/// here we drop every other control char (tabs become a single space).
pub fn sanitize_display(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect()
}

/// Truncate a string to fit within a given display width, adding ellipsis.
/// Correctly handles wide Unicode characters that occupy two columns.
pub fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
    if display_width(s) <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        // Build up the string char by char, tracking display width.
        let mut result = String::new();
        let mut width = 0;
        for ch in s.chars() {
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if width + ch_width + 1 > max_width {
                // +1 for the ellipsis character
                break;
            }
            result.push(ch);
            width += ch_width;
        }
        result.push('\u{2026}'); // ellipsis
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize_display ──────────────────────────────────────────────

    #[test]
    fn sanitize_display_strips_escapes_and_controls() {
        // ANSI color/clear sequences lose their ESC so they can't take effect.
        assert!(!sanitize_display("\x1b[31mred\x1b[0m").contains('\x1b'));
        assert_eq!(sanitize_display("a\x07b\rc"), "abc"); // BEL, CR removed
        assert_eq!(sanitize_display("x\x1b[2J\x1b[Hy"), "x[2J[Hy"); // no ESC, no effect
        assert_eq!(sanitize_display("tab\there"), "tab here"); // tab → space
        // Normal text (incl. multibyte) is preserved.
        assert_eq!(sanitize_display("héllo 日本 🎉"), "héllo 日本 🎉");
    }

    // ── display_width ─────────────────────────────────────────────────

    #[test]
    fn display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn display_width_empty() {
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn display_width_cjk() {
        // CJK ideographs are 2 columns wide
        assert_eq!(display_width("漢字"), 4);
    }

    #[test]
    fn display_width_mixed() {
        assert_eq!(display_width("hi漢"), 4); // 2 + 2
    }

    // ── truncate_with_ellipsis ────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_with_ellipsis("abc", 10), "abc");
    }

    #[test]
    fn truncate_exact_fit() {
        assert_eq!(truncate_with_ellipsis("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        let result = truncate_with_ellipsis("hello world", 6);
        assert!(result.ends_with('\u{2026}'));
        assert!(display_width(&result) <= 6);
    }

    #[test]
    fn truncate_very_small_width() {
        assert_eq!(truncate_with_ellipsis("hello", 1), ".");
        assert_eq!(truncate_with_ellipsis("hello", 2), "..");
        assert_eq!(truncate_with_ellipsis("hello", 3), "...");
    }

    #[test]
    fn truncate_zero_width() {
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
    }

    #[test]
    fn truncate_wide_chars() {
        // "漢字テスト" = 10 columns, truncating to 6 should cut correctly.
        let result = truncate_with_ellipsis("漢字テスト", 6);
        assert!(display_width(&result) <= 6);
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_with_ellipsis("", 5), "");
    }

    // ── centered_rect_fixed ───────────────────────────────────────────

    #[test]
    fn centered_rect_fixed_basic() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect_fixed(40, 10, area);
        assert_eq!(r.x, 20);
        assert_eq!(r.y, 7);
        assert_eq!(r.width, 40);
        assert_eq!(r.height, 10);
    }

    #[test]
    fn centered_rect_fixed_clamps_to_area() {
        let area = Rect::new(0, 0, 20, 10);
        let r = centered_rect_fixed(40, 20, area);
        assert_eq!(r.width, 20); // clamped to area width
        assert_eq!(r.height, 10); // clamped to area height
    }

    #[test]
    fn centered_rect_fixed_zero_area() {
        let area = Rect::new(5, 5, 0, 0);
        let r = centered_rect_fixed(10, 10, area);
        assert_eq!(r.width, 0);
        assert_eq!(r.height, 0);
    }
}
