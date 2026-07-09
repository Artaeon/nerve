//! Deterministic design-consistency linter.
//!
//! The project's design principles live in `.nerve/design.md` (see
//! [`crate::project::ProjectStore::load_design`]). Principles alone are just
//! prose the model is asked to follow; this module makes a useful subset of
//! them *enforceable* so UI/design output stays consistent turn after turn.
//!
//! [`lint_design`] runs a small set of hygiene rules over a stylesheet or
//! component file (CSS/SCSS/TSX/JSX/TS/JS). Some rules are always on
//! (spacing off the 4px grid, hex-color sprawl); others are toggled by
//! keywords found in the project's principles (no gradients / no emoji /
//! minimal shadows). No regex crate is used — every scan is manual character
//! work so multi-byte UTF-8 (emoji!) is handled via `.chars()`.
//!
//! Surfaced to the user through the `/design-check [path]` command
//! (see `crate::commands::project`).

/// A single design-hygiene violation found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignFinding {
    /// 1-based line number the finding refers to.
    pub line: usize,
    /// Machine-readable rule id (e.g. "off-grid-spacing").
    pub rule: String,
    /// Human-readable explanation of the violation.
    pub message: String,
}

/// The 4px spacing scale design tokens are expected to live on.
const SPACING_SCALE: [u32; 18] = [
    0, 1, 2, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 128,
];

/// Lint a CSS/SCSS/TSX/JSX/TS/JS file for design-hygiene violations.
///
/// `principles` is the raw text of the project's design.md (if any); its
/// keywords toggle the gradient/emoji/shadow rules. Files with an unsupported
/// extension return an empty vec. Findings are sorted by line.
pub fn lint_design(path: &str, content: &str, principles: Option<&str>) -> Vec<DesignFinding> {
    if !is_supported(path) {
        return Vec::new();
    }

    let principles_lc = principles.map(|p| p.to_lowercase()).unwrap_or_default();

    let mut findings = Vec::new();
    findings.extend(off_grid_spacing(content));
    findings.extend(color_sprawl(content));
    if forbids_gradients(&principles_lc) {
        findings.extend(gradients(content));
    }
    if forbids_emoji(&principles_lc) {
        findings.extend(emoji(content));
    }
    if forbids_shadows(&principles_lc) {
        findings.extend(shadow_heavy(content));
    }

    findings.sort_by_key(|f| f.line);
    findings
}

/// True when `path`'s extension is one we lint.
fn is_supported(path: &str) -> bool {
    let ext = path
        .rsplit('.')
        .next()
        .filter(|_| path.contains('.'))
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "css" | "scss" | "tsx" | "jsx" | "ts" | "js")
}

// ── Principle keyword gates ────────────────────────────────────────────────

fn forbids_gradients(p: &str) -> bool {
    p.contains("no gradient")
        || p.contains("without gradient")
        || p.contains("keine gradient")
        || p.contains("keine verläufe")
}

fn forbids_emoji(p: &str) -> bool {
    p.contains("no emoji") || p.contains("kein emoji") || p.contains("keine emoji")
}

fn forbids_shadows(p: &str) -> bool {
    p.contains("no shadow") || p.contains("keine schatten")
}

// ── Rules ──────────────────────────────────────────────────────────────────

/// CSS properties whose pixel values are genuinely *spacing* and so must sit on
/// the 4px scale. Deliberately excludes font-size, width/height, border-radius,
/// line-height, letter-spacing etc. — those legitimately take off-scale values,
/// and flagging them would make the linter noisy and wrong. Normalized form:
/// lowercased, non-alphabetic chars stripped (so `padding-left` and `paddingLeft`
/// both become `paddingleft`).
const SPACING_PROPS: &[&str] = &[
    "padding",
    "paddingtop",
    "paddingright",
    "paddingbottom",
    "paddingleft",
    "paddinginline",
    "paddingblock",
    "margin",
    "margintop",
    "marginright",
    "marginbottom",
    "marginleft",
    "margininline",
    "marginblock",
    "gap",
    "rowgap",
    "columngap",
    "top",
    "right",
    "bottom",
    "left",
    "inset",
    "insetinline",
    "insetblock",
];

/// Normalize a CSS property name to its alphabetic, lowercase form.
fn normalize_prop(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The CSS/JSX property that owns the value starting at `value_start` on this
/// line: the identifier before the nearest preceding `:`. `None` when there is
/// no clear owning property (we then err toward NOT flagging).
fn property_for(chars: &[char], value_start: usize) -> Option<String> {
    let colon = chars[..value_start].iter().rposition(|&c| c == ':')?;
    let mut end = colon;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0
        && (chars[start - 1].is_ascii_alphanumeric()
            || chars[start - 1] == '-'
            || chars[start - 1] == '_')
    {
        start -= 1;
    }
    if start == end {
        return None;
    }
    Some(chars[start..end].iter().collect())
}

/// Flag integer `<N>px` values that are off the 4px spacing scale (N > 3) — but
/// ONLY when they belong to a spacing property, so font-sizes, widths, heights
/// and radii are never false-flagged.
fn off_grid_spacing(content: &str) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                // Skip fractional values like `1.5px` — the digit run after
                // the dot is not a whole-pixel value.
                let preceded_by_dot = i > 0 && chars[i - 1] == '.';
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                // Immediately followed by "px"?
                let followed_by_px = i + 1 < chars.len() && chars[i] == 'p' && chars[i + 1] == 'x';
                if followed_by_px && !preceded_by_dot {
                    let digits: String = chars[start..i].iter().collect();
                    let is_spacing = property_for(&chars, start)
                        .map(|p| SPACING_PROPS.contains(&normalize_prop(&p).as_str()))
                        .unwrap_or(false);
                    if is_spacing
                        && let Ok(n) = digits.parse::<u32>()
                        && n > 3
                        && !SPACING_SCALE.contains(&n)
                    {
                        findings.push(DesignFinding {
                            line: idx + 1,
                            rule: "off-grid-spacing".into(),
                            message: format!(
                                "spacing `{n}px` is off the 4px scale (use 4/8/12/16/24/32…)"
                            ),
                        });
                    }
                }
            } else {
                i += 1;
            }
        }
    }
    findings
}

/// Flag lines using CSS gradients when principles forbid them.
fn gradients(content: &str) -> Vec<DesignFinding> {
    let mut findings = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if line.contains("linear-gradient") || line.contains("radial-gradient") {
            findings.push(DesignFinding {
                line: idx + 1,
                rule: "gradients".into(),
                message: "gradient used but design principles forbid gradients".into(),
            });
        }
    }
    findings
}

/// Flag the first emoji-bearing line when principles forbid emoji.
fn emoji(content: &str) -> Vec<DesignFinding> {
    for (idx, line) in content.lines().enumerate() {
        if line.chars().any(is_emoji) {
            return vec![DesignFinding {
                line: idx + 1,
                rule: "emoji".into(),
                message: "emoji used but design principles forbid emoji".into(),
            }];
        }
    }
    Vec::new()
}

/// A char in the common emoji ranges.
fn is_emoji(c: char) -> bool {
    let u = c as u32;
    (0x1F300..=0x1FAFF).contains(&u) || (0x2600..=0x27BF).contains(&u)
}

/// Flag files that use too many distinct hex colors (design-token sprawl).
fn color_sprawl(content: &str) -> Vec<DesignFinding> {
    use std::collections::BTreeSet;
    let mut colors: BTreeSet<String> = BTreeSet::new();
    let mut first_line: Option<usize> = None;

    for (idx, line) in content.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j].is_ascii_hexdigit() {
                    j += 1;
                }
                let run = j - start;
                if run == 3 || run == 6 {
                    let hex: String = chars[start..j].iter().collect();
                    colors.insert(normalize_hex(&hex));
                    first_line.get_or_insert(idx + 1);
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
    }

    if colors.len() > 12 {
        vec![DesignFinding {
            line: first_line.unwrap_or(1),
            rule: "color-sprawl".into(),
            message: format!(
                "{} distinct hex colors — consider consolidating into design tokens",
                colors.len()
            ),
        }]
    } else {
        Vec::new()
    }
}

/// Normalize a 3- or 6-digit hex body to lowercase 6 digits.
fn normalize_hex(hex: &str) -> String {
    let lower = hex.to_lowercase();
    if lower.len() == 3 {
        lower.chars().flat_map(|c| [c, c]).collect()
    } else {
        lower
    }
}

/// Flag heavy `box-shadow` use when principles call for minimal/no shadows.
fn shadow_heavy(content: &str) -> Vec<DesignFinding> {
    let count = content.matches("box-shadow").count();
    if count > 2 {
        let line = content
            .lines()
            .position(|l| l.contains("box-shadow"))
            .map(|p| p + 1)
            .unwrap_or(1);
        vec![DesignFinding {
            line,
            rule: "shadow-heavy".into(),
            message: format!("{count} box-shadows but principles call for minimal/no shadows"),
        }]
    } else {
        Vec::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_extension_returns_empty() {
        let findings = lint_design("notes.md", "13px 26px #a #b", None);
        assert!(findings.is_empty());
        let findings = lint_design("Cargo.toml", "linear-gradient", Some("no gradient"));
        assert!(findings.is_empty());
    }

    #[test]
    fn off_grid_spacing_flags_off_scale_values() {
        let css = ".a { padding: 13px; margin: 26px; }";
        let findings = lint_design("app/globals.css", css, None);
        let msgs: Vec<_> = findings.iter().map(|f| f.message.clone()).collect();
        assert_eq!(findings.len(), 2);
        assert!(msgs.iter().any(|m| m.contains("13px")));
        assert!(msgs.iter().any(|m| m.contains("26px")));
        assert!(findings.iter().all(|f| f.rule == "off-grid-spacing"));
    }

    #[test]
    fn off_grid_spacing_allows_on_scale_values() {
        let css = ".a { padding: 16px; margin: 24px; gap: 8px; inset: 0px; }";
        let findings = lint_design("styles.css", css, None);
        assert!(
            findings.is_empty(),
            "on-scale values should be clean: {findings:?}"
        );
    }

    #[test]
    fn off_grid_spacing_ignores_small_and_fractional() {
        // N <= 3 is allowed even off-scale (3px), and fractional 1.5px is skipped.
        let css = ".a { border: 3px; width: 1.5px; }";
        let findings = lint_design("styles.css", css, None);
        assert!(findings.is_empty(), "got: {findings:?}");
    }

    #[test]
    fn off_grid_reports_correct_line() {
        let css = ".a {\n  padding: 16px;\n  margin: 13px;\n}";
        let findings = lint_design("styles.scss", css, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
    }

    #[test]
    fn off_grid_ignores_non_spacing_properties() {
        // Font-sizes, widths, heights, radii and line-heights legitimately take
        // off-scale px values — they must NOT be flagged, only spacing does.
        let css = "h1 { font-size: 17px; }\n\
             .box { width: 1040px; height: 46px; max-width: 680px; }\n\
             .r { border-radius: 6px; line-height: 22px; letter-spacing: 1px; }\n\
             .off { padding: 26px; }";
        let findings = lint_design("app/globals.css", css, None);
        // Only the padding:26px is a real spacing violation.
        assert_eq!(findings.len(), 1, "got: {findings:?}");
        assert!(findings[0].message.contains("26px"));
    }

    #[test]
    fn off_grid_handles_inline_jsx_style() {
        // Inline style with mixed properties on one line: only padding flags.
        let tsx = "<div style={{ padding: \"26px\", fontSize: 17, width: 300 }} />";
        let findings = lint_design("app/page.tsx", tsx, None);
        assert_eq!(findings.len(), 1, "got: {findings:?}");
        assert!(findings[0].message.contains("26px"));
    }

    #[test]
    fn gradients_flagged_only_when_forbidden() {
        let css = ".a { background: linear-gradient(#fff, #000); }";
        assert!(lint_design("a.css", css, None).is_empty());
        assert!(lint_design("a.css", css, Some("use flat colors")).is_empty());

        let findings = lint_design("a.css", css, Some("No gradients, use flat fills"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "gradients");
    }

    #[test]
    fn gradients_german_keyword() {
        let css = "background: radial-gradient(red, blue);";
        let findings = lint_design("a.scss", css, Some("Keine Verläufe im Design"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "gradients");
    }

    #[test]
    fn emoji_flagged_only_when_forbidden() {
        let tsx = "const label = 'Save 🚀';\nconst other = 'ok';";
        assert!(lint_design("a.tsx", tsx, None).is_empty());
        let findings = lint_design("a.tsx", tsx, Some("No emoji in UI copy"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "emoji");
        assert_eq!(findings[0].line, 1);
    }

    #[test]
    fn color_sprawl_over_threshold() {
        // 13 distinct colors > 12 → one finding.
        let mut css = String::new();
        for i in 0..13 {
            css.push_str(&format!(".c{i} {{ color: #{:06x}; }}\n", i * 0x111111 + 1));
        }
        let findings = lint_design("theme.css", &css, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "color-sprawl");
        assert!(findings[0].message.contains("13 distinct hex colors"));
    }

    #[test]
    fn color_sprawl_under_threshold_and_dedup() {
        // Same color repeated + 3-digit/6-digit equivalence → few distinct.
        let css = ".a{color:#fff}.b{color:#ffffff}.c{color:#000}.d{color:#123}";
        let findings = lint_design("theme.css", css, None);
        assert!(findings.is_empty(), "got: {findings:?}");
    }

    #[test]
    fn shadow_heavy_only_when_forbidden() {
        let css = "a{box-shadow:1}b{box-shadow:2}c{box-shadow:3}";
        assert!(lint_design("a.css", css, None).is_empty());
        // Two shadows is within budget.
        let two = "a{box-shadow:1}b{box-shadow:2}";
        assert!(lint_design("a.css", two, Some("almost no shadows")).is_empty());

        let findings = lint_design("a.css", css, Some("Almost no shadows please"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "shadow-heavy");
        assert!(findings[0].message.contains("3 box-shadows"));
    }

    #[test]
    fn clean_file_returns_empty() {
        let css = ".card {\n  padding: 16px;\n  margin: 24px;\n  color: #1a1a1a;\n  background: #ffffff;\n}";
        let findings = lint_design(
            "app/globals.css",
            css,
            Some("No gradients. No emoji. Almost no shadows."),
        );
        assert!(findings.is_empty(), "got: {findings:?}");
    }

    #[test]
    fn findings_sorted_by_line() {
        let css = ".a {\n  margin: 26px;\n}\n.b {\n  background: linear-gradient(#fff,#000);\n}";
        let findings = lint_design("a.css", css, Some("no gradients"));
        assert_eq!(findings.len(), 2);
        assert!(findings[0].line <= findings[1].line);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[1].line, 5);
    }
}
