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
fn truncation_warning_catches_cutoff_typescript() {
    // The exact failure mode from the vollgebucht build: a .ts file cut off
    // mid-declaration. `node --check` never caught this.
    let w = truncation_warning("types.ts", "export interface DateRange {\n");
    assert!(w.is_some());
    assert!(w.unwrap().contains("TRUNCATED"));
}

#[test]
fn truncation_warning_passes_complete_code() {
    let complete = "export interface X {\n  a: number;\n}\nfunction f() { return [1, 2]; }\n";
    assert!(truncation_warning("x.ts", complete).is_none());
}

#[test]
fn truncation_warning_ignores_braces_in_strings_and_comments() {
    // Unbalanced braces that live inside strings/comments must NOT flag.
    let src = r#"const s = "a { b [ c (";
// a stray } ] ) in a comment
const t = 'another } unmatched';
function ok() { return 1; }
"#;
    assert!(truncation_warning("x.ts", src).is_none());
}

#[test]
fn truncation_warning_flags_unbalanced_closing() {
    let w = truncation_warning("x.rs", "fn main() { let x = 1; }}\n");
    assert!(w.unwrap().contains("unbalanced"));
}

#[test]
fn truncation_warning_skips_prose_extensions() {
    // Markdown / text can have unbalanced braces legitimately.
    assert!(truncation_warning("README.md", "use `foo {` here").is_none());
    assert!(truncation_warning("notes.txt", "a { b").is_none());
}

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

// -- looks_like_code_fragment: defense against mis-parsed tool calls --

#[test]
fn looks_like_code_fragment_catches_real_incidents() {
    // Job e8279faa, 2026-07-17: nerve wrote a file literally named this.
    assert!(looks_like_code_fragment("['serviceId'],"));
    // 2026-07-04, found 12 days later in vollgebucht's repo.
    assert!(looks_like_code_fragment("text('path').notNull(),"));
}

#[test]
fn looks_like_code_fragment_allows_normal_paths() {
    for path in [
        "lib/waitlist/service.ts",
        "src/agent/tools/fs.rs",
        "next.config.mjs",
        "my-file_v2.test.ts",
        ".nerve/verify.toml",
        "docs/A GUIDE.md",
    ] {
        assert!(
            !looks_like_code_fragment(path),
            "should NOT flag normal path: {path}"
        );
    }
}

#[test]
fn execute_write_file_rejects_code_fragment_path() {
    reset_tool_counter();
    let dir = tempfile::tempdir().unwrap();
    let bogus_path = dir.path().join("['serviceId'],");

    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(), bogus_path.to_string_lossy().to_string());
    args.insert("content".to_string(), "whatever".to_string());
    let call = ToolCall {
        tool: "write_file".to_string(),
        args,
    };

    let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
    assert!(!result.success);
    assert!(
        result.output.contains("['serviceId'],"),
        "error should name the offending path: {}",
        result.output
    );
    assert!(
        !bogus_path.exists(),
        "no file should have been written for a code-fragment path"
    );
}

#[test]
fn execute_write_file_normal_path_still_works() {
    reset_tool_counter();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("service.ts");

    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(), path.to_string_lossy().to_string());
    args.insert("content".to_string(), "export const x = 1;".to_string());
    let call = ToolCall {
        tool: "write_file".to_string(),
        args,
    };

    let result = execute_tool(&call, crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS);
    assert!(
        result.success,
        "normal write must still succeed: {}",
        result.output
    );
    assert!(path.exists());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "export const x = 1;"
    );
}
