use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Parse a human-readable keybind string into a crossterm [`KeyEvent`].
///
/// Supported formats (case-insensitive):
///
/// ```text
/// "ctrl+k"
/// "ctrl+shift+p"
/// "alt+enter"
/// "f1"
/// "esc"
/// "enter"
/// "tab"
/// ```
///
/// Returns `None` if the string cannot be parsed.
pub fn parse_keybind(s: &str) -> Option<KeyEvent> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }

    let parts: Vec<&str> = s.split('+').map(str::trim).collect();

    let mut modifiers = KeyModifiers::NONE;
    let mut key_part: Option<&str> = None;

    for part in &parts {
        match *part {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "alt" | "option" | "meta" => modifiers |= KeyModifiers::ALT,
            other => {
                // The last non-modifier token is the key. If we already
                // captured one this is ambiguous -- take the latest.
                key_part = Some(other);
            }
        }
    }

    let key_str = key_part?;
    let code = parse_key_code(key_str)?;

    Some(KeyEvent::new(code, modifiers))
}

/// Map a single key name to a [`KeyCode`].
fn parse_key_code(s: &str) -> Option<KeyCode> {
    // Function keys: f1 .. f12
    if let Some(rest) = s.strip_prefix('f')
        && let Ok(n) = rest.parse::<u8>()
        && (1..=12).contains(&n)
    {
        return Some(KeyCode::F(n));
    }

    match s {
        // Special keys
        "esc" | "escape" => Some(KeyCode::Esc),
        "enter" | "return" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "backspace" | "bs" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" | "ins" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" | "pgup" => Some(KeyCode::PageUp),
        "pagedown" | "pgdn" | "pgdown" => Some(KeyCode::PageDown),

        // Arrow keys
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),

        // Space
        "space" | "spacebar" => Some(KeyCode::Char(' ')),

        // Single character (a-z, 0-9, punctuation)
        _ => {
            let mut chars = s.chars();
            let ch = chars.next()?;
            if chars.next().is_none() {
                Some(KeyCode::Char(ch))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_k() {
        let ev = parse_keybind("ctrl+k").unwrap();
        assert_eq!(ev.code, KeyCode::Char('k'));
        assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_ctrl_shift_p() {
        let ev = parse_keybind("ctrl+shift+p").unwrap();
        assert_eq!(ev.code, KeyCode::Char('p'));
        assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
        assert!(ev.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn parse_f1() {
        let ev = parse_keybind("f1").unwrap();
        assert_eq!(ev.code, KeyCode::F(1));
        assert_eq!(ev.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_f12() {
        let ev = parse_keybind("f12").unwrap();
        assert_eq!(ev.code, KeyCode::F(12));
    }

    #[test]
    fn parse_esc() {
        let ev = parse_keybind("esc").unwrap();
        assert_eq!(ev.code, KeyCode::Esc);
    }

    #[test]
    fn parse_enter() {
        let ev = parse_keybind("enter").unwrap();
        assert_eq!(ev.code, KeyCode::Enter);
    }

    #[test]
    fn parse_tab() {
        let ev = parse_keybind("tab").unwrap();
        assert_eq!(ev.code, KeyCode::Tab);
    }

    #[test]
    fn parse_alt_enter() {
        let ev = parse_keybind("alt+enter").unwrap();
        assert_eq!(ev.code, KeyCode::Enter);
        assert!(ev.modifiers.contains(KeyModifiers::ALT));
    }

    #[test]
    fn case_insensitive() {
        let ev = parse_keybind("Ctrl+Shift+N").unwrap();
        assert_eq!(ev.code, KeyCode::Char('n'));
        assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
        assert!(ev.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn simple_char_key() {
        let ev = parse_keybind("q").unwrap();
        assert_eq!(ev.code, KeyCode::Char('q'));
        assert!(ev.modifiers.is_empty());
    }

    #[test]
    fn empty_string_returns_none() {
        assert!(parse_keybind("").is_none());
    }

    #[test]
    fn nonsense_returns_none() {
        assert!(parse_keybind("ctrl+superduperkey").is_none());
    }

    #[test]
    fn parse_space() {
        let ev = parse_keybind("ctrl+space").unwrap();
        assert_eq!(ev.code, KeyCode::Char(' '));
        assert!(ev.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_pagedown() {
        let ev = parse_keybind("pagedown").unwrap();
        assert_eq!(ev.code, KeyCode::PageDown);
    }
}
