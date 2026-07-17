//! Filesystem tools for the agent: read/write/edit files, line-range reads,
//! directory listing/creation, glob find — plus the path-security helpers
//! (`normalize_path`, `validate_write_path`) shared by every write path.

use std::path::{Path, PathBuf};

use super::{ToolCall, ToolResult, require_arg};

/// The single source of truth for how many characters of a tool result reach
/// the model. `read_file` caps a large file at this size, and the agent runners
/// (interactive + headless) cap the tool-result feedback at the SAME size — so a
/// file the model just read is never re-truncated to a smaller window on the way
/// back. Previously the feedback cap (5,000) was 10x smaller than this read cap,
/// silently dropping the tail of every large read.
pub const MAX_TOOL_OUTPUT_CHARS: usize = 50_000;

pub(super) fn execute_read_file(call: &ToolCall) -> ToolResult {
    let path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };

    // Security: block reading sensitive files
    if crate::shell::is_sensitive_file(path) {
        return ToolResult {
            tool: "read_file".into(),
            success: false,
            output: "Blocked: this file may contain secrets".into(),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Truncate very large files. Byte-slicing here would panic
            // on any file whose byte 50_000 lies mid UTF-8 char (common
            // for CJK source, JSON with non-ASCII strings, etc.).
            let truncated = if content.len() > MAX_TOOL_OUTPUT_CHARS {
                let head: String = content.chars().take(MAX_TOOL_OUTPUT_CHARS).collect();
                format!("{head}...\n\n[Truncated: file is {} bytes]", content.len())
            } else {
                content
            };
            ToolResult {
                tool: "read_file".into(),
                success: true,
                output: truncated,
            }
        }
        Err(e) => ToolResult {
            tool: "read_file".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

/// Best-effort syntax verification for common file types.
/// Returns `Some(error_message)` if a syntax issue is detected, `None` otherwise.
fn verify_file_syntax(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str());

    let escaped = crate::shell::shell_escape(path);
    let check_cmd = match ext {
        Some("rs") => Some(format!("rustfmt --check {escaped} 2>&1 | head -5")),
        Some("py") => Some(format!(
            "python3 -c \"import ast; ast.parse(open({escaped}).read())\" 2>&1",
        )),
        Some("js" | "ts" | "jsx" | "tsx") => Some(format!("node --check {escaped} 2>&1 | head -5")),
        Some("json") => Some(format!(
            "python3 -c \"import json; json.load(open({escaped}))\" 2>&1",
        )),
        Some("yaml" | "yml") => Some(format!(
            "python3 -c \"import yaml; yaml.safe_load(open({escaped}))\" 2>&1",
        )),
        Some("toml") => Some(format!(
            "python3 -c \"import tomllib; tomllib.load(open({escaped}, 'rb'))\" 2>&1",
        )),
        _ => None,
    };

    if let Some(cmd) = check_cmd
        && let Ok(result) = crate::shell::run_command(&cmd)
        && !result.success
    {
        return Some(format!(
            "Syntax check failed:\n{}{}",
            result.stdout, result.stderr
        ));
    }

    None
}

/// Extensions whose bracket delimiters must balance in a complete file.
const BRACED_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "go", "java", "c", "h", "cpp", "hpp", "cs",
    "css", "scss", "json", "jsonc",
];

/// Detect a likely-TRUNCATED write by checking that `{}`, `[]` and `()` balance,
/// ignoring delimiters inside strings and comments. This runs in-process with no
/// external tools — unlike `verify_file_syntax` (which shells out to `node
/// --check`, a tool that can't even parse TypeScript), so it reliably catches
/// the failure mode where the model cuts a file off mid-declaration (e.g. a
/// file ending with `export interface Foo {`). Advisory only: the write still
/// succeeds, but the model is told to rewrite the whole file.
fn truncation_warning(path: &str, content: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?;
    if !BRACED_EXTS.contains(&ext) {
        return None;
    }

    let (mut curly, mut square, mut paren) = (0i32, 0i32, 0i32);
    let mut in_line = false;
    let mut in_block = false;
    let mut string: Option<char> = None;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if in_line {
            if c == '\n' {
                in_line = false;
            }
            continue;
        }
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if let Some(delim) = string {
            match c {
                '\\' => {
                    chars.next(); // skip the escaped char
                }
                _ if c == delim => string = None,
                _ => {}
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                in_line = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block = true;
            }
            '"' | '\'' | '`' => string = Some(c),
            '{' => curly += 1,
            '}' => curly -= 1,
            '[' => square += 1,
            ']' => square -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            _ => {}
        }
        if curly < 0 || square < 0 || paren < 0 {
            return Some(format!(
                "'{path}' has an unbalanced CLOSING delimiter — the file looks malformed. \
                 Rewrite the entire file correctly."
            ));
        }
    }

    if curly != 0 || square != 0 || paren != 0 {
        let mut parts = Vec::new();
        if curly != 0 {
            parts.push(format!("{curly} unclosed {{}}"));
        }
        if square != 0 {
            parts.push(format!("{square} unclosed []"));
        }
        if paren != 0 {
            parts.push(format!("{paren} unclosed ()"));
        }
        return Some(format!(
            "'{path}' appears TRUNCATED ({}). You likely stopped mid-file — \
             rewrite the ENTIRE file in one write_file call, do not cut it off.",
            parts.join(", ")
        ));
    }
    None
}

/// Record a successful mutating tool call in the project change journal
/// (`.nerve/journal.jsonl`). Best-effort: journaling failures are logged and
/// must never fail the tool itself.
fn journal_change(tool: &str, path: &str, summary: &str) {
    // Unit tests run with the nerve repo itself as CWD; journaling there
    // would write into the real repo's .nerve/. The journal API is covered
    // directly by the ProjectStore tests in `crate::project`.
    if cfg!(test) {
        return;
    }
    let Some(ws) = crate::workspace::detect_workspace() else {
        return;
    };
    let store = crate::project::ProjectStore::for_workspace(&ws.root);
    if let Err(e) = store.record_change(tool, path, summary) {
        tracing::warn!("change journal: could not record {tool} on {path}: {e}");
    }
}

/// Resolve a path and check it (and its canonical form) against the protected
/// list.  Returns the resolved [`PathBuf`] on success, or a blocking
/// [`ToolResult`] on failure.
/// Normalize a path by resolving `.` and `..` segments without touching the
/// filesystem.  This catches traversal attempts like `/tmp/x/../../etc/passwd`
/// even when intermediate directories don't exist.
pub(super) fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    let mut is_absolute = false;
    for comp in path.components() {
        match comp {
            Component::RootDir => {
                is_absolute = true;
                parts.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(c) => {
                parts.push(c);
            }
            Component::Prefix(p) => {
                parts.clear();
                parts.push(p.as_os_str());
            }
        }
    }
    let mut result = if is_absolute {
        PathBuf::from("/")
    } else {
        PathBuf::new()
    };
    for p in parts {
        result.push(p);
    }
    result
}

/// True when `path` cannot plausibly be a file path a human meant to write,
/// i.e. it looks like a fragment of source code that a mis-parse handed us.
///
/// WHY this exists: nerve has twice silently written a file whose NAME was a
/// line of source code, because the tool-call parser re-scanned file CONTENT
/// as arguments -- `['serviceId'],` (job e8279faa, 2026-07-17) and
/// `text('path').notNull(),` (2026-07-04, found 12 days later in
/// vollgebucht's repo). Both incidents were invisible: no error, the
/// intended file simply never got written, and a human found the debris
/// days later. The root cause is being fixed in the parser separately; this
/// is the safety net for when that fix is imperfect or a new parser bug
/// appears -- fail LOUDLY at the moment of damage instead of silently
/// creating junk.
///
/// We reject a path containing a newline, a single quote, a double quote,
/// `(`, `)`, `[`, `]`, `{`, `}`, `;`, `,`, or `=`. In a real code repository,
/// filenames containing these are vanishingly rare, whereas every known
/// mis-parse contained several. This deliberately trades a theoretical
/// false rejection (some exotic filename with a bracket in it) for catching
/// a silent corruption that has already happened twice. Dots, dashes,
/// underscores, nested slashes, and spaces are all fine -- real files use
/// them constantly (`next.config.mjs`, `my-file_v2.test.ts`,
/// `docs/A GUIDE.md`, `.nerve/verify.toml`).
pub(crate) fn looks_like_code_fragment(path: &str) -> bool {
    const SUSPICIOUS: [char; 11] = ['\n', '\'', '"', '(', ')', '[', ']', '{', '}', ';', ','];
    path.contains(SUSPICIOUS.as_slice()) || path.contains('=')
}

pub(super) fn validate_write_path(path: &str, tool: &str) -> Result<PathBuf, ToolResult> {
    // Defense in depth against mis-parsed tool calls: reject before any
    // other check so it can't be forgotten by a future write-path tool.
    // See `looks_like_code_fragment` for the two real incidents this guards.
    if looks_like_code_fragment(path) {
        let msg = format!(
            "refusing to write \"{path}\": that is not a plausible file path -- \
             it looks like a line of code. The tool call was probably malformed; \
             re-send it with the path on its own line and the content last."
        );
        return Err(ToolResult {
            tool: tool.to_string(),
            success: false,
            output: msg,
        });
    }

    // First check the raw path.
    if crate::shell::is_protected_path(path) {
        return Err(ToolResult {
            tool: tool.into(),
            success: false,
            output: format!("Blocked: cannot write to protected path: {path}"),
        });
    }

    let buf = PathBuf::from(path);

    // Normalize ".." segments without touching the filesystem — catches
    // traversal attempts like /tmp/x/../../etc/passwd.
    let normalized = normalize_path(&buf);
    let norm_str = normalized.to_string_lossy();
    if crate::shell::is_protected_path(&norm_str) {
        return Err(ToolResult {
            tool: tool.into(),
            success: false,
            output: format!("Blocked: normalized path is protected: {norm_str}"),
        });
    }

    // If the file (or a parent) already exists, canonicalize to defeat
    // symlink attacks that redirect writes to protected locations.
    let canonical = if buf.exists() {
        buf.canonicalize().unwrap_or_else(|_| normalized.clone())
    } else if let Some(parent) = buf.parent() {
        if parent.exists() {
            parent
                .canonicalize()
                .map(|p| p.join(buf.file_name().unwrap_or_default()))
                .unwrap_or_else(|_| normalized.clone())
        } else {
            normalized.clone()
        }
    } else {
        normalized.clone()
    };

    let canon_str = canonical.to_string_lossy();
    if crate::shell::is_protected_path(&canon_str) {
        return Err(ToolResult {
            tool: tool.into(),
            success: false,
            output: format!("Blocked: resolved path is protected: {canon_str}"),
        });
    }

    // Block persistence/exfiltration write targets in the user's home or repo
    // (SSH keys, shell rc files, git hooks, credential files) — the vectors a
    // prompt-injected model would use. Check normalized + canonical forms.
    if crate::shell::is_protected_write_target(&norm_str)
        || crate::shell::is_protected_write_target(&canon_str)
    {
        return Err(ToolResult {
            tool: tool.into(),
            success: false,
            output: format!(
                "Blocked: {canon_str} is a protected target (SSH keys, shell rc, \
                 git hooks, and credential files can't be written by the agent)"
            ),
        });
    }

    Ok(buf)
}

pub(super) fn execute_write_file(call: &ToolCall) -> ToolResult {
    let path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let content = call
        .args
        .get("content")
        .map(std::string::String::as_str)
        .unwrap_or("");

    // Security: block writing to protected system paths (including symlinks).
    if let Err(blocked) = validate_write_path(path, "write_file") {
        return blocked;
    }

    // Create parent directories
    if let Some(parent) = Path::new(path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return ToolResult {
                tool: "write_file".into(),
                success: false,
                output: format!("Failed to create parent directory: {e}"),
            };
        }
    }

    match std::fs::write(path, content) {
        Ok(()) => {
            journal_change(
                "write_file",
                path,
                &format!("wrote {} bytes", content.len()),
            );
            // Auto-verify: a dependency-free truncation check first (reliably
            // catches files cut off mid-write, incl. TypeScript), then the
            // language syntax check where a tool is available.
            let warning = truncation_warning(path, content).or_else(|| verify_file_syntax(path));
            if let Some(error) = warning {
                return ToolResult {
                    tool: "write_file".into(),
                    success: true,
                    output: format!(
                        "Written {} bytes to {path}\n\nWARNING: {error}",
                        content.len()
                    ),
                };
            }
            ToolResult {
                tool: "write_file".into(),
                success: true,
                output: format!("Written {} bytes to {path}", content.len()),
            }
        }
        Err(e) => ToolResult {
            tool: "write_file".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

pub(super) fn execute_edit_file(call: &ToolCall) -> ToolResult {
    let path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let old_text = match require_arg(call, "old_text") {
        Ok(t) => t,
        Err(e) => return e,
    };
    let new_text = call
        .args
        .get("new_text")
        .map(std::string::String::as_str)
        .unwrap_or("");

    // Security: block editing protected system paths (including symlinks).
    if let Err(blocked) = validate_write_path(path, "edit_file") {
        return blocked;
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            if !content.contains(old_text) {
                return ToolResult {
                    tool: "edit_file".into(),
                    success: false,
                    output: "old_text not found in file".into(),
                };
            }
            let new_content = content.replacen(old_text, new_text, 1);
            match std::fs::write(path, &new_content) {
                Ok(()) => {
                    journal_change(
                        "edit_file",
                        path,
                        &format!(
                            "replaced {}-char snippet with {}-char snippet",
                            old_text.chars().count(),
                            new_text.chars().count()
                        ),
                    );
                    // Auto-verify: truncation (dependency-free) then syntax.
                    let warning =
                        truncation_warning(path, &new_content).or_else(|| verify_file_syntax(path));
                    if let Some(error) = warning {
                        return ToolResult {
                            tool: "edit_file".into(),
                            success: true,
                            output: format!("Edited {path}\n\nWARNING: {error}"),
                        };
                    }
                    ToolResult {
                        tool: "edit_file".into(),
                        success: true,
                        output: format!("Edited {path}"),
                    }
                }
                Err(e) => ToolResult {
                    tool: "edit_file".into(),
                    success: false,
                    output: format!("Write error: {e}"),
                },
            }
        }
        Err(e) => ToolResult {
            tool: "edit_file".into(),
            success: false,
            output: format!("Read error: {e}"),
        },
    }
}

pub(super) fn execute_list_files(call: &ToolCall) -> ToolResult {
    let path = call
        .args
        .get("path")
        .map(std::string::String::as_str)
        .unwrap_or(".");
    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut items: Vec<String> = entries
                .filter_map(std::result::Result::ok)
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect();
            items.sort();
            ToolResult {
                tool: "list_files".into(),
                success: true,
                output: items.join("\n"),
            }
        }
        Err(e) => ToolResult {
            tool: "list_files".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

pub(super) fn execute_create_dir(call: &ToolCall) -> ToolResult {
    let path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };

    // Security: use the same path validation as write_file/edit_file, which
    // also normalizes `..` traversal and canonicalizes symlinks. A raw
    // is_protected_path check alone is bypassed by e.g. `/tmp/x/../../etc`.
    if let Err(blocked) = validate_write_path(path, "create_directory") {
        return blocked;
    }

    match std::fs::create_dir_all(path) {
        Ok(()) => {
            journal_change("create_directory", path, "created directory");
            ToolResult {
                tool: "create_directory".into(),
                success: true,
                output: format!("Created {path}"),
            }
        }
        Err(e) => ToolResult {
            tool: "create_directory".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

pub(super) fn execute_find_files(call: &ToolCall) -> ToolResult {
    let pattern = call
        .args
        .get("pattern")
        .map(std::string::String::as_str)
        .unwrap_or("*");
    let path = call
        .args
        .get("path")
        .map(std::string::String::as_str)
        .unwrap_or(".");

    let cmd = format!(
        "find {} -name {} -type f | head -100",
        crate::shell::shell_escape(path),
        crate::shell::shell_escape(pattern)
    );
    match crate::shell::run_command(&cmd) {
        Ok(result) => ToolResult {
            tool: "find_files".into(),
            success: true,
            output: if result.stdout.is_empty() {
                "No files found".into()
            } else {
                result.stdout
            },
        },
        Err(e) => ToolResult {
            tool: "find_files".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

pub(super) fn execute_read_lines(call: &ToolCall) -> ToolResult {
    let path = match require_arg(call, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let start: usize = call
        .args
        .get("start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let end: usize = call
        .args
        .get("end")
        .and_then(|s| s.parse().ok())
        .unwrap_or(start.saturating_add(50));

    if crate::shell::is_sensitive_file(path) {
        return ToolResult {
            tool: "read_lines".into(),
            success: false,
            output: "Blocked: file may contain secrets".into(),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            let s = start.saturating_sub(1).min(total);
            // Clamp end to at least `s` so an LLM-supplied reversed range
            // (e.g. start: 50, end: 10) yields an empty slice, not a panic.
            let e = end.min(total).max(s);

            let mut output = String::new();
            for (i, line) in lines[s..e].iter().enumerate() {
                output.push_str(&format!("{:>4} | {}\n", s + i + 1, line));
            }
            output.push_str(&format!("\n[Lines {}-{} of {} total]", s + 1, e, total));

            ToolResult {
                tool: "read_lines".into(),
                success: true,
                output,
            }
        }
        Err(e) => ToolResult {
            tool: "read_lines".into(),
            success: false,
            output: format!("Error: {e}"),
        },
    }
}

#[cfg(test)]
#[path = "fs_tests.rs"]
mod tests;
