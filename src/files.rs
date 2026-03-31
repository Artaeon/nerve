use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// Result of reading a file for context
#[derive(Debug, Clone)]
pub struct FileContext {
    pub path: String,
    pub content: String,
    pub language: String,
    pub line_count: usize,
    pub size_bytes: u64,
}

/// Read a file and prepare it as conversation context
pub fn read_file_context(path: &str) -> anyhow::Result<FileContext> {
    let path_buf = resolve_path(path);
    let metadata = fs::metadata(&path_buf)
        .with_context(|| format!("Cannot access: {}", path_buf.display()))?;

    if metadata.is_dir() {
        return read_directory_listing(&path_buf);
    }

    // Reject files over 1MB
    if metadata.len() > 1_048_576 {
        anyhow::bail!("File too large ({} bytes). Max 1MB.", metadata.len());
    }

    let content = fs::read_to_string(&path_buf)
        .with_context(|| format!("Cannot read: {}", path_buf.display()))?;

    let language = detect_language(&path_buf);
    let line_count = content.lines().count();

    Ok(FileContext {
        path: path_buf.display().to_string(),
        content,
        language,
        line_count,
        size_bytes: metadata.len(),
    })
}

/// Read multiple files
pub fn read_files_context(paths: &[&str]) -> Vec<anyhow::Result<FileContext>> {
    paths.iter().map(|p| read_file_context(p)).collect()
}

/// Format a FileContext as a message string for the AI
pub fn format_file_for_context(fc: &FileContext) -> String {
    if fc.language == "directory" {
        format!("Directory listing of `{}`:\n\n{}", fc.path, fc.content)
    } else {
        format!(
            "File: `{}` ({} lines, {})\n```{}\n{}\n```",
            fc.path,
            fc.line_count,
            format_size(fc.size_bytes),
            fc.language,
            fc.content
        )
    }
}

/// Read a file with line range (start..end, 1-indexed)
pub fn read_file_range(path: &str, start: usize, end: usize) -> anyhow::Result<FileContext> {
    let path_buf = resolve_path(path);
    let content = fs::read_to_string(&path_buf)
        .with_context(|| format!("Cannot read: {}", path_buf.display()))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start = start.saturating_sub(1).min(total); // 1-indexed to 0-indexed
    let end = end.min(total);

    let selected = lines[start..end].join("\n");
    let language = detect_language(&path_buf);

    Ok(FileContext {
        path: format!("{}:{}-{}", path_buf.display(), start + 1, end),
        content: selected,
        language,
        line_count: end - start,
        size_bytes: 0,
    })
}

/// Expand @file references in a message.
///
/// Words starting with `@` that look like file paths (contain `.` or `/`) are
/// resolved and replaced with the formatted file content.  The original text is
/// left unchanged for display; the returned string should only be sent to the AI.
pub fn expand_file_references(text: &str) -> String {
    let mut result = text.to_string();
    let words: Vec<&str> = text.split_whitespace().collect();
    for word in &words {
        if word.starts_with('@') && word.len() > 1 {
            let path = &word[1..];
            // Check if it looks like a file path (has a dot or slash)
            if path.contains('.') || path.contains('/') {
                if let Ok(fc) = read_file_context(path) {
                    let formatted = format_file_for_context(&fc);
                    result = result.replace(word, &format!("\n\n{formatted}\n\n"));
                }
            }
        }
    }
    result
}

/// Resolve path: expand ~ and make relative paths absolute
fn resolve_path(path: &str) -> PathBuf {
    let path = if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&path[2..])
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    // If relative, resolve against CWD
    if path.is_relative() {
        std::env::current_dir().unwrap_or_default().join(path)
    } else {
        path
    }
}

/// Detect programming language from file extension
fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("tsx") => "tsx",
        Some("jsx") => "jsx",
        Some("go") => "go",
        Some("rb") => "ruby",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp" | "cc" | "cxx") => "cpp",
        Some("h" | "hpp") => "cpp",
        Some("cs") => "csharp",
        Some("swift") => "swift",
        Some("kt") => "kotlin",
        Some("scala") => "scala",
        Some("php") => "php",
        Some("sh" | "bash" | "zsh") => "bash",
        Some("sql") => "sql",
        Some("html" | "htm") => "html",
        Some("css") => "css",
        Some("scss" | "sass") => "scss",
        Some("json") => "json",
        Some("yaml" | "yml") => "yaml",
        Some("toml") => "toml",
        Some("xml") => "xml",
        Some("md" | "markdown") => "markdown",
        Some("dockerfile") => "dockerfile",
        Some("lua") => "lua",
        Some("r") => "r",
        Some("zig") => "zig",
        Some("nim") => "nim",
        Some("ex" | "exs") => "elixir",
        Some("erl") => "erlang",
        Some("hs") => "haskell",
        Some("ml" | "mli") => "ocaml",
        Some("v") => "v",
        Some("dart") => "dart",
        Some("tf") => "hcl",
        _ => "text",
    }
    .to_string()
}

/// Format file size human-readably
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1}KB", bytes as f64 / 1024.0);
    }
    format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
}

/// Read a directory listing (non-recursive, first level)
fn read_directory_listing(path: &Path) -> anyhow::Result<FileContext> {
    let mut entries: Vec<String> = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        } // skip hidden

        if file_type.is_dir() {
            entries.push(format!("{name}/"));
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            entries.push(format!("{name}  ({})", format_size(size)));
        }
    }

    entries.sort();

    Ok(FileContext {
        path: path.display().to_string(),
        content: entries.join("\n"),
        language: "directory".into(),
        line_count: entries.len(),
        size_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_path ───────────────────────────────────────────────────

    #[test]
    fn resolve_path_absolute() {
        let p = resolve_path("/tmp/test.txt");
        assert_eq!(p, PathBuf::from("/tmp/test.txt"));
    }

    #[test]
    fn resolve_path_relative_becomes_absolute() {
        let p = resolve_path("foo/bar.rs");
        assert!(p.is_absolute());
        assert!(p.ends_with("foo/bar.rs"));
    }

    #[test]
    fn resolve_path_tilde() {
        let p = resolve_path("~/Documents/notes.md");
        // Should not start with ~
        assert!(!p.to_string_lossy().starts_with('~'));
        assert!(p.ends_with("Documents/notes.md"));
    }

    // ── detect_language ────────────────────────────────────────────────

    #[test]
    fn detect_language_rust() {
        assert_eq!(detect_language(Path::new("main.rs")), "rust");
    }

    #[test]
    fn detect_language_python() {
        assert_eq!(detect_language(Path::new("script.py")), "python");
    }

    #[test]
    fn detect_language_typescript() {
        assert_eq!(detect_language(Path::new("app.ts")), "typescript");
        assert_eq!(detect_language(Path::new("App.tsx")), "tsx");
    }

    #[test]
    fn detect_language_cpp_variants() {
        assert_eq!(detect_language(Path::new("main.cpp")), "cpp");
        assert_eq!(detect_language(Path::new("main.cc")), "cpp");
        assert_eq!(detect_language(Path::new("main.cxx")), "cpp");
        assert_eq!(detect_language(Path::new("main.h")), "cpp");
        assert_eq!(detect_language(Path::new("main.hpp")), "cpp");
    }

    #[test]
    fn detect_language_shell() {
        assert_eq!(detect_language(Path::new("run.sh")), "bash");
        assert_eq!(detect_language(Path::new("run.bash")), "bash");
        assert_eq!(detect_language(Path::new("run.zsh")), "bash");
    }

    #[test]
    fn detect_language_config_files() {
        assert_eq!(detect_language(Path::new("config.json")), "json");
        assert_eq!(detect_language(Path::new("config.yaml")), "yaml");
        assert_eq!(detect_language(Path::new("config.yml")), "yaml");
        assert_eq!(detect_language(Path::new("Cargo.toml")), "toml");
    }

    #[test]
    fn detect_language_unknown() {
        assert_eq!(detect_language(Path::new("README")), "text");
        assert_eq!(detect_language(Path::new("Makefile")), "text");
    }

    #[test]
    fn detect_language_various() {
        assert_eq!(detect_language(Path::new("main.go")), "go");
        assert_eq!(detect_language(Path::new("app.rb")), "ruby");
        assert_eq!(detect_language(Path::new("Main.java")), "java");
        assert_eq!(detect_language(Path::new("main.c")), "c");
        assert_eq!(detect_language(Path::new("main.swift")), "swift");
        assert_eq!(detect_language(Path::new("main.kt")), "kotlin");
        assert_eq!(detect_language(Path::new("main.zig")), "zig");
        assert_eq!(detect_language(Path::new("main.hs")), "haskell");
        assert_eq!(detect_language(Path::new("main.dart")), "dart");
        assert_eq!(detect_language(Path::new("main.ex")), "elixir");
        assert_eq!(detect_language(Path::new("main.exs")), "elixir");
        assert_eq!(detect_language(Path::new("main.erl")), "erlang");
        assert_eq!(detect_language(Path::new("infra.tf")), "hcl");
    }

    // ── format_size ────────────────────────────────────────────────────

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(2048), "2.0KB");
        assert_eq!(format_size(1536), "1.5KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1_048_576), "1.0MB");
        assert_eq!(format_size(2_097_152), "2.0MB");
    }

    // ── read_file_context ──────────────────────────────────────────────

    #[test]
    fn read_file_context_success() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let fc = read_file_context(file_path.to_str().unwrap()).unwrap();
        assert_eq!(fc.language, "rust");
        assert_eq!(fc.line_count, 3);
        assert!(fc.content.contains("fn main()"));
        assert!(fc.size_bytes > 0);
    }

    #[test]
    fn read_file_context_not_found() {
        let result = read_file_context("/tmp/nerve_nonexistent_file_abc123.txt");
        assert!(result.is_err());
    }

    #[test]
    fn read_file_context_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("hello.txt"), "hi").unwrap();
        fs::write(dir.path().join("world.rs"), "fn main() {}").unwrap();

        let fc = read_file_context(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(fc.language, "directory");
        assert!(fc.content.contains("hello.txt"));
        assert!(fc.content.contains("world.rs"));
        assert!(fc.content.contains("subdir/"));
    }

    #[test]
    fn read_file_context_hidden_files_excluded_from_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".hidden"), "secret").unwrap();
        fs::write(dir.path().join("visible.txt"), "public").unwrap();

        let fc = read_file_context(dir.path().to_str().unwrap()).unwrap();
        assert!(!fc.content.contains(".hidden"));
        assert!(fc.content.contains("visible.txt"));
    }

    #[test]
    fn read_file_context_too_large() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("big.txt");
        // Create a file just over 1MB
        let data = vec![b'x'; 1_048_577];
        fs::write(&file_path, &data).unwrap();

        let result = read_file_context(file_path.to_str().unwrap());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too large"));
    }

    // ── read_file_range ────────────────────────────────────────────────

    #[test]
    fn read_file_range_specific_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("lines.txt");
        fs::write(&file_path, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let fc = read_file_range(file_path.to_str().unwrap(), 2, 4).unwrap();
        assert_eq!(fc.line_count, 3);
        assert_eq!(fc.content, "line2\nline3\nline4");
        assert!(fc.path.contains(":2-4"));
    }

    #[test]
    fn read_file_range_beyond_end() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("short.txt");
        fs::write(&file_path, "a\nb\nc\n").unwrap();

        let fc = read_file_range(file_path.to_str().unwrap(), 1, 100).unwrap();
        assert_eq!(fc.line_count, 3);
        assert_eq!(fc.content, "a\nb\nc");
    }

    #[test]
    fn read_file_range_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("single.txt");
        fs::write(&file_path, "alpha\nbeta\ngamma\n").unwrap();

        let fc = read_file_range(file_path.to_str().unwrap(), 2, 2).unwrap();
        assert_eq!(fc.line_count, 1);
        assert_eq!(fc.content, "beta");
    }

    // ── format_file_for_context ────────────────────────────────────────

    #[test]
    fn format_file_for_context_code() {
        let fc = FileContext {
            path: "/tmp/test.rs".into(),
            content: "fn main() {}".into(),
            language: "rust".into(),
            line_count: 1,
            size_bytes: 12,
        };
        let formatted = format_file_for_context(&fc);
        assert!(formatted.contains("File: `/tmp/test.rs`"));
        assert!(formatted.contains("```rust"));
        assert!(formatted.contains("fn main() {}"));
        assert!(formatted.contains("1 lines"));
        assert!(formatted.contains("12B"));
    }

    #[test]
    fn format_file_for_context_directory() {
        let fc = FileContext {
            path: "/tmp/mydir".into(),
            content: "file1.txt\nfile2.rs".into(),
            language: "directory".into(),
            line_count: 2,
            size_bytes: 0,
        };
        let formatted = format_file_for_context(&fc);
        assert!(formatted.contains("Directory listing of `/tmp/mydir`"));
        assert!(formatted.contains("file1.txt"));
        assert!(formatted.contains("file2.rs"));
        // Should NOT contain code fence
        assert!(!formatted.contains("```"));
    }

    // ── read_files_context ─────────────────────────────────────────────

    #[test]
    fn read_files_context_mixed() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.py");
        let f2 = dir.path().join("b.rs");
        fs::write(&f1, "print('hi')").unwrap();
        fs::write(&f2, "fn main() {}").unwrap();

        let results = read_files_context(&[f1.to_str().unwrap(), f2.to_str().unwrap()]);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert_eq!(results[0].as_ref().unwrap().language, "python");
        assert_eq!(results[1].as_ref().unwrap().language, "rust");
    }

    #[test]
    fn read_files_context_with_errors() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("exists.txt");
        fs::write(&f1, "content").unwrap();

        let results =
            read_files_context(&[f1.to_str().unwrap(), "/tmp/nerve_no_such_file_999.txt"]);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
    }

    // ── expand_file_references ─────────────────────────────────────────

    #[test]
    fn expand_file_references_with_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("ctx.rs");
        fs::write(&file_path, "fn hello() {}").unwrap();

        let input = format!("Explain this: @{}", file_path.display());
        let expanded = expand_file_references(&input);
        assert!(expanded.contains("fn hello() {}"));
        assert!(expanded.contains("```rust"));
        // The @path token should be replaced
        assert!(!expanded.contains(&format!("@{}", file_path.display())));
    }

    #[test]
    fn expand_file_references_nonexistent_file() {
        let input = "Look at @/tmp/nerve_nonexistent_xyz.rs please";
        let expanded = expand_file_references(input);
        // Should leave the @reference in place since the file does not exist
        assert!(expanded.contains("@/tmp/nerve_nonexistent_xyz.rs"));
    }

    #[test]
    fn expand_file_references_no_references() {
        let input = "Just a normal message with no file references";
        let expanded = expand_file_references(input);
        assert_eq!(expanded, input);
    }

    #[test]
    fn expand_file_references_at_sign_not_a_path() {
        // @ followed by something without dot or slash should be left alone
        let input = "Hey @username what do you think?";
        let expanded = expand_file_references(input);
        assert_eq!(expanded, input);
    }

    #[test]
    fn expand_file_references_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("one.py");
        let f2 = dir.path().join("two.py");
        fs::write(&f1, "print(1)").unwrap();
        fs::write(&f2, "print(2)").unwrap();

        let input = format!("Compare @{} and @{}", f1.display(), f2.display());
        let expanded = expand_file_references(&input);
        assert!(expanded.contains("print(1)"));
        assert!(expanded.contains("print(2)"));
    }

    #[test]
    fn read_file_with_unicode() {
        let dir = std::env::temp_dir().join("nerve_test_unicode");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let content = "Hello \u{4e16}\u{754c}\nRust \u{1f980}\n";
        fs::write(dir.join("unicode.txt"), content).unwrap();
        let fc = read_file_context(&dir.join("unicode.txt").to_string_lossy()).unwrap();
        assert!(fc.content.contains("\u{4e16}\u{754c}"));
        assert!(fc.content.contains("\u{1f980}"));
        assert_eq!(fc.line_count, 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detect_language_dockerfile() {
        // Dockerfile has no extension, should fall back to "text"
        let path = std::path::Path::new("Dockerfile");
        let lang = detect_language(path);
        assert_eq!(lang, "text");
    }

    #[test]
    fn format_size_boundaries() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(1023), "1023B");
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(1048576), "1.0MB");
    }

    #[test]
    fn expand_file_references_preserves_nonfile_at() {
        // @username should not be expanded
        let text = "hello @john how are you";
        let expanded = expand_file_references(text);
        assert_eq!(expanded, text);
    }
}
