//! Automatic context gathering for user messages.
//!
//! When a user asks about code (e.g. "what does handle_login do?", "fix the
//! error in config.rs"), this module detects relevant files and gathers their
//! contents so the AI can answer without needing a manual `/file` command.

use std::collections::HashSet;
use std::path::Path;

/// Maximum number of files to auto-include.
const MAX_FILES: usize = 3;

/// Maximum total characters of auto-gathered context.
const MAX_TOTAL_CHARS: usize = 12_000;

/// Result of auto-context analysis.
pub struct GatheredContext {
    /// Files included as context, with their contents.
    pub files: Vec<FileSnippet>,
}

pub struct FileSnippet {
    pub path: String,
    pub content: String,
    pub line_count: usize,
}

/// Extract file references from a user message and gather their contents.
///
/// Returns gathered context if relevant files are found, or an empty result.
/// Runs entirely on the filesystem — no AI calls.
pub fn gather_context(message: &str, workspace_root: Option<&Path>) -> GatheredContext {
    let root = workspace_root
        .map(std::path::Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();

    let mut candidates: Vec<String> = Vec::new();

    // 1. Extract explicit file paths from the message.
    extract_file_paths(message, &mut candidates);

    // 2. Search for symbol names (function, struct, etc.) in the workspace.
    extract_symbol_references(message, &root, &mut candidates);

    // Deduplicate while preserving order.
    let mut seen = HashSet::new();
    candidates.retain(|p| seen.insert(p.clone()));

    // 3. Read candidate files, respecting limits.
    let mut files = Vec::new();
    let mut total_chars = 0;

    for path_str in candidates.iter().take(MAX_FILES * 2) {
        if files.len() >= MAX_FILES || total_chars >= MAX_TOTAL_CHARS {
            break;
        }

        let full_path = if Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            root.join(path_str)
        };

        if !full_path.is_file() {
            continue;
        }

        // Skip large files.
        if let Ok(meta) = std::fs::metadata(&full_path) {
            if meta.len() > 100_000 {
                continue;
            }
        }

        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let line_count = content.lines().count();

            // Truncate if adding this file would exceed budget.
            let available = MAX_TOTAL_CHARS.saturating_sub(total_chars);
            let content = if content.len() > available {
                let truncated: String = content.chars().take(available).collect();
                format!("{truncated}\n... (truncated)")
            } else {
                content
            };

            total_chars += content.len();
            files.push(FileSnippet {
                path: path_str.clone(),
                content,
                line_count,
            });
        }
    }

    GatheredContext { files }
}

/// Format gathered context as a system message for the AI.
pub fn format_context(ctx: &GatheredContext) -> Option<String> {
    if ctx.files.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    for f in &ctx.files {
        parts.push(format!(
            "── {} ({} lines) ──\n{}",
            f.path, f.line_count, f.content
        ));
    }

    Some(format!(
        "The following files were automatically included as context \
         based on the user's message:\n\n{}",
        parts.join("\n\n")
    ))
}

// ── Path extraction ──────────────────────────────────────────────────────

/// Common source file extensions.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "rb", "ex", "exs", "c", "cpp", "h", "hpp",
    "cs", "zig", "toml", "yaml", "yml", "json", "md", "sh", "bash", "css", "html", "sql", "proto",
    "graphql", "svelte", "vue",
];

fn extract_file_paths(message: &str, candidates: &mut Vec<String>) {
    for token in message.split_whitespace() {
        // Strip punctuation from the token edges.
        let clean = token.trim_matches(|c: char| {
            c == ',' || c == '.' || c == ':' || c == ';' || c == '`' || c == '"' || c == '\''
        });

        if clean.is_empty() {
            continue;
        }

        // Tokens containing / are likely paths — skip URLs.
        if clean.contains('/')
            && clean.len() > 1
            && !clean.starts_with("http")
            && !clean.starts_with("ftp")
        {
            candidates.push(clean.to_string());
            continue;
        }

        // Skip URL-like tokens entirely.
        if clean.starts_with("http") || clean.starts_with("ftp") {
            continue;
        }

        // Tokens with a source file extension.
        if let Some(ext) = Path::new(clean).extension().and_then(|e| e.to_str()) {
            if SOURCE_EXTENSIONS.contains(&ext) {
                candidates.push(clean.to_string());
            }
        }
    }
}

fn extract_symbol_references(message: &str, root: &Path, candidates: &mut Vec<String>) {
    let lower = message.to_lowercase();

    // Look for patterns like "the X function", "struct Y", "fn Z", "module W"
    let symbol_patterns = [
        "function ",
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "class ",
        "module ",
        "mod ",
        "component ",
        "handler ",
        "endpoint ",
        "method ",
        "impl ",
    ];

    let mut symbols: Vec<String> = Vec::new();
    for pattern in symbol_patterns {
        if let Some(pos) = lower.find(pattern) {
            let after = &message[pos + pattern.len()..];
            // Take the next word as the symbol name.
            if let Some(sym) = after.split_whitespace().next() {
                let clean = sym.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if clean.len() >= 2 {
                    symbols.push(clean.to_string());
                }
            }
        }
    }

    if symbols.is_empty() {
        return;
    }

    // Search for these symbols in common source files using a lightweight grep.
    for sym in symbols.iter().take(3) {
        if let Ok(output) = std::process::Command::new("grep")
            .args([
                "-rl",
                "--include=*.rs",
                "--include=*.py",
                "--include=*.js",
                "--include=*.ts",
                "--include=*.go",
                "--include=*.java",
                "-m",
                "1",
                sym,
            ])
            .arg(root)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().take(2) {
                let relative = line
                    .strip_prefix(&format!("{}/", root.display()))
                    .unwrap_or(line);
                candidates.push(relative.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_file_paths_explicit() {
        let mut candidates = Vec::new();
        extract_file_paths("look at src/main.rs and config.toml", &mut candidates);
        assert!(candidates.contains(&"src/main.rs".to_string()));
        assert!(candidates.contains(&"config.toml".to_string()));
    }

    #[test]
    fn extract_file_paths_with_punctuation() {
        let mut candidates = Vec::new();
        extract_file_paths("check `src/lib.rs`, please", &mut candidates);
        assert!(candidates.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn extract_file_paths_ignores_urls() {
        let mut candidates = Vec::new();
        extract_file_paths("see https://example.com/path/file.rs", &mut candidates);
        assert!(candidates.is_empty());
    }

    #[test]
    fn extract_file_paths_no_match() {
        let mut candidates = Vec::new();
        extract_file_paths("explain how sorting works", &mut candidates);
        assert!(candidates.is_empty());
    }

    #[test]
    fn format_context_empty_returns_none() {
        let ctx = GatheredContext { files: vec![] };
        assert!(format_context(&ctx).is_none());
    }

    #[test]
    fn format_context_single_file() {
        let ctx = GatheredContext {
            files: vec![FileSnippet {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
                line_count: 1,
            }],
        };
        let formatted = format_context(&ctx).unwrap();
        assert!(formatted.contains("src/main.rs"));
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("automatically included"));
    }

    #[test]
    fn gather_context_finds_real_file() {
        // This test runs inside the nerve project, so Cargo.toml exists.
        let ctx = gather_context("look at Cargo.toml", None);
        assert!(
            !ctx.files.is_empty(),
            "Should find Cargo.toml in project root"
        );
        assert_eq!(ctx.files[0].path, "Cargo.toml");
    }

    #[test]
    fn gather_context_respects_max_files() {
        // Even with many file references, should cap at MAX_FILES.
        let msg =
            "check src/main.rs src/app.rs src/config.rs src/shell.rs src/files.rs src/workspace.rs";
        let ctx = gather_context(msg, None);
        assert!(ctx.files.len() <= MAX_FILES);
    }

    #[test]
    fn gather_context_nonexistent_file() {
        let ctx = gather_context("look at nonexistent_xyz_abc.rs", None);
        assert!(ctx.files.is_empty());
    }

    #[test]
    fn gather_context_no_file_references() {
        let ctx = gather_context("explain how TCP works", None);
        assert!(ctx.files.is_empty());
    }
}
