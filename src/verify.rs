//! Auto-verification gate.
//!
//! Real-world finding from building a full app with nerve: the agent writes
//! correct-looking code, but truncated files, type errors, and broken
//! integrations only surfaced when a *human* ran the build. This module closes
//! that gap: after an agent turn that edited files, nerve runs the project's
//! verify command (a fast type-check / build) and feeds any failure straight
//! back into the agent loop, so it fixes its own mistakes before handing back.
//!
//! The command is run as an ordinary `run_command` tool call through the
//! existing async runner, so the UI stays live and the compiler output is fed
//! to the model exactly like any other tool result — it simply sees the errors
//! and corrects them. A per-turn round cap stops a persistently-failing build
//! from looping the agent forever.

use std::path::Path;

/// Maximum verify → fix rounds per user turn.
pub const MAX_VERIFY_ROUNDS: u8 = 2;

/// Infer a sensible verify command for a project root, or `None` when we can't.
/// Prefers a fast type-check over a full build.
pub fn detect_verify_command(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").exists() {
        return Some("cargo check --quiet 2>&1".into());
    }
    if root.join("package.json").exists() {
        // Prefer an explicit type-check/lint script; these are the cheap,
        // high-signal checks a project already defines. We must look ONLY at
        // the `scripts` object: a naive substring search over the whole file
        // used to match a *dependency* named e.g. "check" (or the word
        // appearing anywhere else, such as in a "description"), so we'd hand
        // back `npm run -s check` for a script that doesn't exist — npm exits
        // non-zero with "Missing script", and the gate then blamed the agent
        // for a build error it could never fix, burning every fix round.
        let pkg = std::fs::read_to_string(root.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg).ok()?;
        let scripts = json.get("scripts")?;
        for script in ["typecheck", "type-check", "lint", "check"] {
            if scripts.get(script).is_some() {
                return Some(format!("npm run -s {script} 2>&1"));
            }
        }
    }
    None
}

/// Detect the project's TEST command, so the worker gate can run the suite in
/// addition to the type-check. A type-check alone is not enough: this session a
/// job committed a *failing test* that `tsc`/`lint` happily passed, and only
/// human review caught it. Runs after the type-check (chained), never instead.
///
/// Returns `None` (skip tests) when the test script is a WATCHER — a watch-mode
/// runner never exits and would hang the job until the timeout. We only run a
/// test command we can be confident terminates.
pub fn detect_test_command(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").exists() {
        return Some("cargo test --quiet 2>&1".into());
    }
    if root.join("package.json").exists() {
        let pkg = std::fs::read_to_string(root.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&pkg).ok()?;
        let test = json.get("scripts")?.get("test")?.as_str()?;
        if is_watch_script(test) || test.trim().is_empty() {
            return None;
        }
        return Some("npm test --silent 2>&1".into());
    }
    None
}

/// Whether an npm test script runs a WATCHER (never exits) rather than a
/// one-shot run. `run_verify` has no timeout, so running a watcher would hang the
/// job forever — but skipping a suite that would have run is just as bad: the gate
/// silently degrades to type-check-only, which is the exact hole the test gate was
/// added to close.
///
/// So decide on TOKENS, not substrings. A naive `contains("watch")` also matched
/// perfectly good one-shot commands like `vitest run src/watcher.test.ts` or
/// `node --test test/watchdog.test.js` — a project with a file named "watcher"
/// silently lost its whole suite.
fn is_watch_script(script: &str) -> bool {
    script.split_whitespace().any(|tok| {
        let t = tok.to_lowercase();
        // `--watch`, `--watchAll`, `--watch=true`, and the `-w` short flag.
        t == "-w" || t == "--watch" || t.starts_with("--watch=") || t.starts_with("--watchall")
            // A dedicated file-watcher runner.
            || t == "nodemon"
    }) || script.to_lowercase().contains("no test specified")
}

/// Whether to run the verify step now: it's enabled, we have a command, the
/// agent actually edited files this turn, and rounds remain.
pub fn should_run_verify(
    enabled: bool,
    command: Option<&str>,
    made_edits: bool,
    rounds_used: u8,
) -> bool {
    enabled && command.is_some() && made_edits && rounds_used < MAX_VERIFY_ROUNDS
}

/// True if a tool name mutates files — used to decide whether a turn is worth
/// verifying (a read-only Q&A turn is not).
pub fn is_write_tool(tool: &str) -> bool {
    matches!(tool, "write_file" | "edit_file" | "create_directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cargo_and_npm() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_verify_command(dir.path()), None);

        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            detect_verify_command(dir.path()).as_deref(),
            Some("cargo check --quiet 2>&1")
        );
    }

    #[test]
    fn detect_test_command_npm_run_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "vitest run", "lint": "tsc --noEmit"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_test_command(dir.path()).as_deref(),
            Some("npm test --silent 2>&1")
        );
    }

    #[test]
    fn detect_test_command_skips_watchers_and_placeholders() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "vitest --watch"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_test_command(dir.path()),
            None,
            "watch mode must be skipped"
        );

        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"test": "echo \"Error: no test specified\" && exit 1"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_test_command(dir.path()),
            None,
            "placeholder must be skipped"
        );
    }

    #[test]
    fn a_test_file_named_watcher_does_not_disable_the_suite() {
        // THE REGRESSION: `contains("watch")` matched the PATH, not a flag, so a
        // project with a watcher test silently ran no tests at all and the gate
        // quietly degraded to type-check-only.
        let dir = tempfile::tempdir().unwrap();
        for script in [
            "vitest run src/watcher.test.ts",
            "node --test test/watchdog.test.js",
            "jest src/watchlist",
        ] {
            std::fs::write(
                dir.path().join("package.json"),
                format!(r#"{{"scripts": {{"test": "{script}"}}}}"#),
            )
            .unwrap();
            assert_eq!(
                detect_test_command(dir.path()).as_deref(),
                Some("npm test --silent 2>&1"),
                "one-shot run must NOT be treated as a watcher: {script}"
            );
        }
    }

    #[test]
    fn real_watchers_are_still_skipped() {
        let dir = tempfile::tempdir().unwrap();
        for script in [
            "vitest --watch",
            "jest --watchAll",
            "vitest -w",
            "nodemon --exec vitest",
            "jest --watch=true",
        ] {
            std::fs::write(
                dir.path().join("package.json"),
                format!(r#"{{"scripts": {{"test": "{script}"}}}}"#),
            )
            .unwrap();
            assert_eq!(
                detect_test_command(dir.path()),
                None,
                "a real watcher would hang the job forever: {script}"
            );
        }
    }

    #[test]
    fn detect_test_command_cargo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            detect_test_command(dir.path()).as_deref(),
            Some("cargo test --quiet 2>&1")
        );
    }

    #[test]
    fn detect_test_command_none_without_test_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"lint": "tsc --noEmit"}}"#,
        )
        .unwrap();
        assert_eq!(detect_test_command(dir.path()), None);
    }

    #[test]
    fn detects_npm_lint_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"dev": "next dev", "lint": "tsc --noEmit"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_verify_command(dir.path()).as_deref(),
            Some("npm run -s lint 2>&1")
        );
    }

    #[test]
    fn a_dependency_named_check_does_not_become_the_verify_command() {
        // THE REGRESSION. Detection used to substring-search the WHOLE file, so a
        // *dependency* called "check" matched and we returned `npm run -s check`
        // for a script that doesn't exist. npm then failed with "Missing script",
        // the gate blamed the agent, and both fix rounds burned on every job.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"description": "we lint and check things",
                "devDependencies": {"check": "^1.0.0", "typecheck": "^2.0.0"},
                "scripts": {"dev": "next dev"}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_verify_command(dir.path()),
            None,
            "only the scripts object may select the verify command"
        );
    }

    #[test]
    fn verify_command_respects_script_priority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"typecheck": "tsc --noEmit", "lint": "eslint ."}}"#,
        )
        .unwrap();
        assert_eq!(
            detect_verify_command(dir.path()).as_deref(),
            Some("npm run -s typecheck 2>&1"),
            "typecheck outranks lint"
        );
    }

    #[test]
    fn malformed_package_json_is_none_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{not valid json").unwrap();
        assert_eq!(detect_verify_command(dir.path()), None);
        assert_eq!(detect_test_command(dir.path()), None);
    }

    #[test]
    fn npm_without_check_script_is_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"dev": "next dev"}}"#,
        )
        .unwrap();
        assert_eq!(detect_verify_command(dir.path()), None);
    }

    #[test]
    fn should_run_gates_on_every_condition() {
        // Happy path.
        assert!(should_run_verify(true, Some("cargo check"), true, 0));
        // Disabled.
        assert!(!should_run_verify(false, Some("cargo check"), true, 0));
        // No command.
        assert!(!should_run_verify(true, None, true, 0));
        // No edits this turn (read-only Q&A).
        assert!(!should_run_verify(true, Some("cargo check"), false, 0));
        // Out of rounds.
        assert!(!should_run_verify(
            true,
            Some("cargo check"),
            true,
            MAX_VERIFY_ROUNDS
        ));
    }

    #[test]
    fn write_tools_recognised() {
        assert!(is_write_tool("write_file"));
        assert!(is_write_tool("edit_file"));
        assert!(is_write_tool("create_directory"));
        assert!(!is_write_tool("read_file"));
        assert!(!is_write_tool("run_command"));
    }
}
