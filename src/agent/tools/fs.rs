//! Filesystem tools for the agent: read/write/edit files, line-range reads,
//! directory listing/creation, glob find — plus the path-security helpers
//! (`normalize_path`, `validate_write_path`) shared by every write path.

use std::path::{Path, PathBuf};

use super::{ToolCall, ToolResult, require_arg};

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
            let truncated = if content.len() > 50_000 {
                let head: String = content.chars().take(50_000).collect();
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

pub(super) fn validate_write_path(path: &str, tool: &str) -> Result<PathBuf, ToolResult> {
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
            // Auto-verify syntax
            if let Some(error) = verify_file_syntax(path) {
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
                    // Auto-verify syntax
                    if let Some(error) = verify_file_syntax(path) {
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
mod tests {
    use super::*;
    use crate::agent::tools::{execute_tool, reset_tool_counter};

    #[test]
    fn execute_list_files_current_dir() {
        let call = ToolCall {
            tool: "list_files".into(),
            args: [("path".into(), ".".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert!(result.output.contains("Cargo.toml"));
    }

    #[test]
    fn execute_read_file_not_found() {
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/nonexistent/file.txt".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn execute_create_and_write_file() {
        let dir = std::env::temp_dir().join("nerve_agent_test");
        let file_path = dir.join("test.txt");
        let _ = std::fs::remove_dir_all(&dir);

        // Create directory
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), dir.to_string_lossy().into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);

        // Write file
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), file_path.to_string_lossy().into()),
                ("content".into(), "hello world".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);

        // Read file
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), file_path.to_string_lossy().into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert_eq!(result.output, "hello world");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_edit_file_not_found() {
        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "/nonexistent.rs".into()),
                ("old_text".into(), "foo".into()),
                ("new_text".into(), "bar".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn execute_edit_file_text_not_found() {
        let dir = std::env::temp_dir().join("nerve_edit_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello world").unwrap();

        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), path.to_string_lossy().into()),
                ("old_text".into(), "nonexistent".into()),
                ("new_text".into(), "replacement".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("not found"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_edit_file_success() {
        let dir = std::env::temp_dir().join("nerve_edit_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, "hello world").unwrap();

        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), path.to_string_lossy().into()),
                ("old_text".into(), "hello".into()),
                ("new_text".into(), "goodbye".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "goodbye world");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Security tests ──────────────────────────────────────────────────

    #[test]
    fn agent_blocks_write_to_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), "/etc/passwd".into()),
                ("content".into(), "bad".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
        assert!(result.output.contains("protected"));
    }

    #[test]
    fn agent_blocks_edit_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "/usr/bin/something".into()),
                ("old_text".into(), "a".into()),
                ("new_text".into(), "b".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    #[test]
    fn agent_blocks_create_dir_in_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), "/etc/nerve_test".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    #[test]
    fn agent_blocks_reading_sensitive_files() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/home/user/.ssh/id_rsa".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
        assert!(result.output.contains("secrets"));
    }

    #[test]
    fn agent_blocks_reading_env_file() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), ".env".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(result.output.contains("Blocked"));
    }

    // ── Security: write_file to protected paths ────────────────────────

    #[test]
    fn write_file_to_etc_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), "/etc/test_nerve".into()),
                ("content".into(), "malicious".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(
            result.output.contains("protected") || result.output.contains("Blocked"),
            "Expected 'protected' or 'Blocked' in: {}",
            result.output
        );
    }

    // ── Security: read sensitive files ──────────────────────────────────

    #[test]
    fn read_sensitive_env_file_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), ".env".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
        assert!(
            result.output.contains("secrets") || result.output.contains("Blocked"),
            "Expected 'secrets' or 'Blocked' in: {}",
            result.output
        );
    }

    #[test]
    fn read_ssh_key_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/home/user/.ssh/id_rsa".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn read_env_production_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), ".env.production".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn read_aws_credentials_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/home/user/.aws/credentials".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    // ── Security: create_directory in system paths ─────────────────────

    #[test]
    fn create_dir_in_usr_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), "/usr/local/nerve_test".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn create_dir_in_var_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), "/var/nerve_test".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    // ── Security: additional injection & hardening tests ──────────────────

    #[test]
    fn write_file_path_traversal_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), "/etc/../etc/passwd_test".into()),
                ("content".into(), "malicious".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn edit_file_protected_path_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "edit_file".into(),
            args: [
                ("path".into(), "/usr/local/bin/test".into()),
                ("old_text".into(), "old".into()),
                ("new_text".into(), "new".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn read_aws_credentials_path_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "/home/user/.aws/credentials".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn read_env_production_file_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_file".into(),
            args: [("path".into(), "project/.env.production".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn create_dir_in_boot_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".into(), "/boot/malicious".into())].into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    #[test]
    fn write_file_creates_parent_dirs() {
        reset_tool_counter();
        let dir = std::env::temp_dir().join("nerve_write_test");
        let path = dir.join("subdir").join("test.txt");
        let _ = std::fs::remove_dir_all(&dir);

        let call = ToolCall {
            tool: "write_file".into(),
            args: [
                ("path".into(), path.to_string_lossy().into()),
                ("content".into(), "hello".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert!(path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn execute_find_files_in_src() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "find_files".into(),
            args: [
                ("pattern".into(), "*.rs".into()),
                ("path".into(), "src".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert!(result.output.contains("main.rs"));
    }

    #[test]
    fn execute_read_lines_range() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_lines".into(),
            args: [
                ("path".into(), "Cargo.toml".into()),
                ("start".into(), "1".into()),
                ("end".into(), "5".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert!(result.output.contains("[package]"));
        assert!(result.output.contains("1 |"));
    }

    #[test]
    fn execute_read_lines_reversed_range_no_panic() {
        // Regression: an LLM-supplied reversed range (start > end) used to
        // panic on `lines[s..e]`. It must yield an empty selection instead.
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_lines".into(),
            args: [
                ("path".into(), "Cargo.toml".into()),
                ("start".into(), "50".into()),
                ("end".into(), "10".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success); // no panic; empty range reported
    }

    #[test]
    fn execute_read_lines_sensitive_blocked() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_lines".into(),
            args: [
                ("path".into(), ".env".into()),
                ("start".into(), "1".into()),
                ("end".into(), "10".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(!result.success);
    }

    // ── Syntax verification tests ──────────────────────────────────────

    #[test]
    fn verify_file_syntax_valid_json() {
        let dir = std::env::temp_dir().join("nerve_verify_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("valid.json");
        std::fs::write(&path, r#"{"key": "value"}"#).unwrap();
        let result = verify_file_syntax(&path.to_string_lossy());
        assert!(result.is_none()); // Valid JSON, no error
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verify_file_syntax_unknown_ext() {
        let result = verify_file_syntax("test.unknown_extension");
        assert!(result.is_none()); // Unknown extension, no check
    }

    #[test]
    fn execute_find_files_with_no_matches() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "find_files".into(),
            args: [
                ("pattern".into(), "*.nonexistent_extension_xyz".into()),
                ("path".into(), "src".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        assert!(result.output.contains("No files found") || result.output.trim().is_empty());
    }

    #[test]
    fn execute_read_lines_start_beyond_end() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "read_lines".into(),
            args: [
                ("path".into(), "Cargo.toml".into()),
                ("start".into(), "999".into()),
                ("end".into(), "1000".into()),
            ]
            .into(),
        };
        let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
        assert!(result.success);
        // Should return empty or a note about the range
    }

    #[test]
    fn verify_file_syntax_invalid_json() {
        let dir = std::env::temp_dir().join(format!("nerve_verify_invalid_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "{invalid json}").unwrap();
        let result = verify_file_syntax(&path.to_string_lossy());
        // May or may not detect the error (depends on python3 availability)
        // Just verify no panic
        let _ = result;
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── .nerve/ write protection ──────────────────────────────────────

    #[test]
    fn validate_write_path_blocks_project_memory() {
        for path in [
            ".nerve/memory.md",
            ".nerve/decisions.jsonl",
            "sub/.nerve/brief.md",
        ] {
            assert!(
                validate_write_path(path, "write_file").is_err(),
                "agent write to {path} must be blocked (prompt-injection persistence)"
            );
        }
    }

    // ── normalize_path ────────────────────────────────────────────────

    #[test]
    fn normalize_resolves_dotdot() {
        let p = normalize_path(Path::new("/tmp/safe/../../etc/passwd"));
        assert_eq!(p, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn normalize_resolves_dot() {
        let p = normalize_path(Path::new("/tmp/./safe/./file"));
        assert_eq!(p, PathBuf::from("/tmp/safe/file"));
    }

    #[test]
    fn normalize_absolute_stays_absolute() {
        let p = normalize_path(Path::new("/usr/bin/ls"));
        assert_eq!(p, PathBuf::from("/usr/bin/ls"));
    }

    #[test]
    fn normalize_relative() {
        let p = normalize_path(Path::new("src/../Cargo.toml"));
        assert_eq!(p, PathBuf::from("Cargo.toml"));
    }

    #[test]
    fn normalize_excessive_dotdot_clamps_at_root() {
        let p = normalize_path(Path::new("/tmp/../../../../etc/shadow"));
        assert_eq!(p, PathBuf::from("/etc/shadow"));
    }

    // ── validate_write_path traversal prevention ──────────────────────

    #[test]
    fn validate_write_path_blocks_dotdot_to_etc() {
        reset_tool_counter();
        let result = validate_write_path("/tmp/x/../../etc/passwd", "write_file");
        assert!(result.is_err(), "Should block traversal to /etc/passwd");
    }

    #[test]
    fn validate_write_path_blocks_dotdot_to_usr() {
        reset_tool_counter();
        let result = validate_write_path("/home/user/../../usr/bin/evil", "write_file");
        assert!(result.is_err(), "Should block traversal to /usr/bin");
    }

    #[test]
    fn validate_write_path_allows_safe_paths() {
        reset_tool_counter();
        let result = validate_write_path("/tmp/safe_file.txt", "write_file");
        assert!(result.is_ok(), "Should allow /tmp writes");
    }

    #[test]
    fn validate_write_path_blocks_persistence_targets() {
        reset_tool_counter();
        // SSH authorized_keys, shell rc, and git hooks in a (non-system) path
        // must be blocked as persistence/exfil targets.
        for p in [
            "/tmp/fakehome/.ssh/authorized_keys",
            "/tmp/fakehome/.bashrc",
            "/tmp/fakerepo/.git/hooks/pre-commit",
        ] {
            assert!(
                validate_write_path(p, "write_file").is_err(),
                "should block protected write target: {p}"
            );
        }
    }

    #[test]
    fn validate_write_path_blocks_direct_etc() {
        reset_tool_counter();
        let result = validate_write_path("/etc/evil.conf", "write_file");
        assert!(result.is_err(), "Should block direct /etc/ writes");
    }

    #[test]
    fn create_dir_blocks_protected_path() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".to_string(), "/etc/nerve_evil".to_string())]
                .into_iter()
                .collect(),
        };
        let result = execute_create_dir(&call);
        assert!(
            !result.success,
            "create_directory into /etc must be blocked"
        );
    }

    #[test]
    fn create_dir_blocks_dotdot_traversal() {
        reset_tool_counter();
        let call = ToolCall {
            tool: "create_directory".into(),
            args: [("path".to_string(), "/tmp/x/../../etc/evil".to_string())]
                .into_iter()
                .collect(),
        };
        let result = execute_create_dir(&call);
        assert!(
            !result.success,
            "create_directory `..` traversal to /etc must be blocked"
        );
    }

    // ── validate_write_path edge cases ──────────────────────────────

    #[test]
    fn validate_write_path_empty_string_returns_ok() {
        reset_tool_counter();
        // Empty path is not a protected system path, so validate_write_path
        // lets it through. The actual write will fail at the fs::write level.
        let result = validate_write_path("", "write_file");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_write_path_blocks_etc_passwd() {
        reset_tool_counter();
        let result = validate_write_path("/etc/passwd", "write_file");
        assert!(result.is_err(), "Should block writing to /etc/");
    }

    #[test]
    fn validate_write_path_blocks_traversal() {
        reset_tool_counter();
        let result = validate_write_path("/tmp/x/../../etc/shadow", "write_file");
        assert!(result.is_err(), "Should block path traversal to /etc/");
    }
}
