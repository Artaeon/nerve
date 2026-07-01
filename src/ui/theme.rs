use ratatui::style::Color;

use crate::app::App;
use crate::config;

// ── Resolved theme colors ───────────────────────────────────────────────

/// Parsed theme colors ready for use in rendering.
pub struct ResolvedTheme {
    pub accent: Color,
    pub border: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub dim: Color,
    pub user_color: Color,
    #[allow(dead_code)]
    pub assistant_color: Color,
}

impl ResolvedTheme {
    /// Separator color derived from the border color (slightly brighter).
    pub fn separator(&self) -> Color {
        self.border
    }

    /// Active border = accent, inactive border = dim.
    pub fn active_border(&self) -> Color {
        self.accent
    }

    pub fn inactive_border(&self) -> Color {
        self.dim
    }
}

/// Parse a hex colour string (#rrggbb) into a ratatui Color.
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    Color::Rgb(r, g, b)
}

/// Resolve the current theme from the app's theme_index into parsed Colors.
pub fn resolve_theme(app: &App) -> ResolvedTheme {
    let presets = config::theme_presets();
    let theme = presets
        .get(app.theme_index)
        .map(|(_, t)| t.clone())
        .unwrap_or_default();

    ResolvedTheme {
        accent: hex_to_color(&theme.accent_color),
        border: hex_to_color(&theme.border_color),
        success: hex_to_color(&theme.success_color),
        error: hex_to_color(&theme.error_color),
        warning: hex_to_color(&theme.warning_color),
        dim: hex_to_color(&theme.dim_color),
        user_color: hex_to_color(&theme.user_color),
        assistant_color: hex_to_color(&theme.assistant_color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── hex_to_color ────────────────────────────────────────────────

    #[test]
    fn hex_to_color_valid_black() {
        assert_eq!(hex_to_color("#000000"), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn hex_to_color_valid_white() {
        assert_eq!(hex_to_color("#ffffff"), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn hex_to_color_valid_red() {
        assert_eq!(hex_to_color("#ff0000"), Color::Rgb(255, 0, 0));
    }

    #[test]
    fn hex_to_color_without_hash() {
        assert_eq!(hex_to_color("00ff00"), Color::Rgb(0, 255, 0));
    }

    #[test]
    fn hex_to_color_invalid_length() {
        assert_eq!(hex_to_color("#fff"), Color::White);
    }

    #[test]
    fn hex_to_color_empty_string() {
        assert_eq!(hex_to_color(""), Color::White);
    }

    #[test]
    fn hex_to_color_invalid_chars() {
        // Invalid hex chars get unwrap_or(255)
        assert_eq!(hex_to_color("#zzzzzz"), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn hex_to_color_uppercase() {
        assert_eq!(hex_to_color("#AABBCC"), Color::Rgb(170, 187, 204));
    }

    // ── ResolvedTheme helpers ───────────────────────────────────────

    #[test]
    fn resolved_theme_separator_returns_border() {
        let theme = ResolvedTheme {
            accent: Color::Red,
            border: Color::Blue,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            dim: Color::Gray,
            user_color: Color::Cyan,
            assistant_color: Color::White,
        };
        assert_eq!(theme.separator(), Color::Blue);
        assert_eq!(theme.active_border(), Color::Red);
        assert_eq!(theme.inactive_border(), Color::Gray);
    }
}
