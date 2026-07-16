//! Project-declared EXTRA verify step.
//!
//! nerve's gate is `detect_verify_command` (a fast type-check/lint) chained
//! with `detect_test_command` (the suite). It deliberately NEVER runs a full
//! build: a build is slow and often needs environment variables that aren't
//! available in the job sandbox, so running one unconditionally would turn
//! the gate into a source of false failures on every job, not a safety net.
//!
//! But TWICE now — on real jobs, not hypotheticals — nerve produced code that
//! passed lint AND the full test suite, and would STILL have broken the
//! PRODUCTION BUILD of the app it was building:
//!
//!   1. `require("tsx/cjs")` inside `next.config.mjs`. Works fine under
//!      `npm run dev`, where devDependencies are installed. But a production
//!      install runs `npm ci --omit=dev`, `tsx` is gone, and `next build`
//!      throws on startup. Lint never loads next.config.mjs at runtime, and
//!      the test suite never runs a real production install, so nothing in
//!      the existing gate could ever have caught it.
//!   2. Reading a REQUIRED env var at MODULE SCOPE in a route that gets
//!      prerendered. That promotes the var from a runtime dependency to a
//!      BUILD-time requirement: `next build` evaluates the module while
//!      collecting page data, the var isn't set in that environment, and the
//!      build fails. Again: correct code, correct types, passing tests —
//!      the type-checker and the suite both run in an environment where the
//!      var (or the mock of it) happens to be set, so neither one is in a
//!      position to notice.
//!
//! In both cases the code was CORRECT. What was missing was the blast
//! radius: a full build exercises constraints (the real dependency graph,
//! the real environment, the real bundler) that no unit test and no
//! type-check re-creates. Only a human running the actual build caught it,
//! both times, after the fact.
//!
//! The fix is not to make nerve always build (see above — that trades one
//! failure mode for a worse, noisier one). The fix is to let the PROJECT
//! decide, once, that its gate is not complete without a specific extra
//! command — usually `npm run build` or equivalent — and have nerve run it
//! every time. This module is the pure parsing/loading/composition layer for
//! that opt-in: `.nerve/verify.toml` in the project root.
//!
//! `toml` is already a dependency of this crate (see Cargo.toml), so we use
//! it directly rather than hand-rolling a parser.
//!
//! This module intentionally does NOT touch `src/verify.rs`, `src/worker.rs`
//! or `src/config.rs` — wiring the composed gate into the actual job runner
//! is a separate change. This is the pure logic + tests layer only.

use std::path::Path;

/// The parsed, validated contents of a project's `.nerve/verify.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectVerify {
    /// Extra commands to run after typecheck+tests, in the order declared.
    pub extra: Vec<String>,
}

/// Parse the minimal `.nerve/verify.toml` shape:
///
/// ```toml
/// # every command here is appended to the gate, in order, after typecheck+tests
/// extra = ["npm run build"]
/// ///
/// A missing `extra` key or an empty array is NOT an error — most projects
/// have no extra checks, and that's the normal, expected case. Everything
/// else that could make the declared list silently mean less than it says
/// (a non-array `extra`, non-string entries, or an empty/whitespace-only
/// command) is rejected outright: a project that writes `extra = [""]`
/// almost certainly meant to declare a real check and typo'd it, and letting
/// that become a no-op command in the gate is exactly the "safeguard that
/// quietly does nothing" bug class this whole feature exists to prevent.
pub fn parse_project_verify(toml_str: &str) -> Result<ProjectVerify, String> {
    let value: toml::Value = toml_str
        .parse()
        .map_err(|e| format!("invalid TOML in verify.toml: {e}"))?;

    // No `extra` key at all: a project that hasn't opted in. Silent and free.
    let Some(extra_value) = value.get("extra") else {
        return Ok(ProjectVerify { extra: vec![] });
    };

    let array = extra_value.as_array().ok_or_else(|| {
        "`extra` must be an array of strings, e.g. extra = [\"npm run build\"]".to_string()
    })?;

    let mut extra = Vec::with_capacity(array.len());
    for (i, item) in array.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| {
            format!("`extra[{i}]` must be a string, e.g. extra = [\"npm run build\"]")
        })?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "`extra[{i}]` is empty or whitespace-only; remove it or replace it with a real command"
            ));
        }
        extra.push(trimmed.to_string());
    }

    Ok(ProjectVerify { extra })
}

/// Load `<root>/.nerve/verify.toml`, if it exists.
///
/// - No `.nerve` dir, or no `verify.toml` inside it: return empty, silently.
///   This is the overwhelmingly common case (a project with no extra verify
///   step) and must cost nothing and warn nothing.
/// - File exists but can't be read, or fails to parse: return empty, but
///   `tracing::warn!` the reason.
///
///   Why not just propagate the error and fail the job? Because a malformed
///   OPT-IN file must never wedge every job in the repo — that would make
///   the feature actively hostile the moment someone fat-fingers the TOML.
///   Why not silently swallow it either? Because a gate that quietly runs
///   *less* than the project author thinks it does is worse than a repo with
///   no extra gate at all — that's precisely the failure mode this module
///   exists to close (see module doc). So: degrade gracefully, but say so
///   loudly enough that it shows up in logs.
pub fn load_project_verify(root: &Path) -> ProjectVerify {
    let path = root.join(".nerve").join("verify.toml");

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return ProjectVerify { extra: vec![] };
        }
        Err(e) => {
            tracing::warn!(
                "could not read {} (project extra-verify step disabled): {e}",
                path.display()
            );
            return ProjectVerify { extra: vec![] };
        }
    };

    match parse_project_verify(&content) {
        Ok(pv) => pv,
        Err(e) => {
            tracing::warn!(
                "ignoring malformed {} (project extra-verify step disabled): {e}",
                path.display()
            );
            ProjectVerify { extra: vec![] }
        }
    }
}

/// Compose the final gate command: the base typecheck+test chain, followed
/// by every extra command the project declared, in order. Empty entries are
/// skipped defensively even though `parse_project_verify` should never hand
/// one back — belt and suspenders, since a silently-dropped `&&` segment
/// here would turn into a no-op step exactly like the bug this feature
/// exists to prevent.
pub fn compose_gate(base: &str, extra: &[String]) -> String {
    let mut parts: Vec<&str> = vec![base];
    for e in extra {
        if !e.trim().is_empty() {
            parts.push(e);
        }
    }
    parts.join(" && ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_project_verify -------------------------------------------

    #[test]
    fn missing_extra_key_is_empty_not_an_error() {
        let result = parse_project_verify("").unwrap();
        assert_eq!(result, ProjectVerify { extra: vec![] });

        let result = parse_project_verify("# just a comment\n").unwrap();
        assert_eq!(result, ProjectVerify { extra: vec![] });

        let result = parse_project_verify("other_key = \"whatever\"\n").unwrap();
        assert_eq!(result, ProjectVerify { extra: vec![] });
    }

    #[test]
    fn empty_array_is_empty_not_an_error() {
        let result = parse_project_verify("extra = []\n").unwrap();
        assert_eq!(result, ProjectVerify { extra: vec![] });
    }

    #[test]
    fn valid_extra_array_is_parsed_in_order() {
        let result = parse_project_verify(
            "# every command here is appended to the gate, in order, after typecheck+tests\nextra = [\"npm run build\"]\n",
        )
        .unwrap();
        assert_eq!(
            result,
            ProjectVerify {
                extra: vec!["npm run build".to_string()]
            }
        );
    }

    #[test]
    fn order_is_preserved() {
        let result =
            parse_project_verify(r#"extra = ["step one", "step two", "step three"]"#).unwrap();
        assert_eq!(
            result.extra,
            vec![
                "step one".to_string(),
                "step two".to_string(),
                "step three".to_string(),
            ]
        );
    }

    #[test]
    fn duplicates_are_kept_as_is() {
        // De-duping would surprise a project that genuinely wants the same
        // command run twice (e.g. a flaky external check); harmless either way.
        let result = parse_project_verify(r#"extra = ["npm run build", "npm run build"]"#).unwrap();
        assert_eq!(
            result.extra,
            vec!["npm run build".to_string(), "npm run build".to_string()]
        );
    }

    #[test]
    fn entries_are_trimmed() {
        let result = parse_project_verify(r#"extra = ["  npm run build  "]"#).unwrap();
        assert_eq!(result.extra, vec!["npm run build".to_string()]);
    }

    #[test]
    fn non_array_extra_is_rejected_with_a_clear_message() {
        let err = parse_project_verify(r#"extra = "npm run build""#).unwrap_err();
        assert!(
            err.contains("extra"),
            "error must name the offending key: {err}"
        );
        assert!(
            err.contains("array"),
            "error should explain what's expected: {err}"
        );
    }

    #[test]
    fn array_of_non_strings_is_rejected() {
        let err = parse_project_verify("extra = [1, 2, 3]").unwrap_err();
        assert!(
            err.contains("extra"),
            "error must name the offending key: {err}"
        );
    }

    #[test]
    fn empty_string_entry_is_rejected() {
        // A silent empty command would make that link in the `&&` chain a
        // no-op, quietly disabling the gate — exactly the bug class this
        // feature exists to prevent.
        let err = parse_project_verify(r#"extra = [""]"#).unwrap_err();
        assert!(err.contains("extra"));
    }

    #[test]
    fn whitespace_only_entry_is_rejected() {
        let err = parse_project_verify(r#"extra = ["   "]"#).unwrap_err();
        assert!(err.contains("extra"));
    }

    #[test]
    fn invalid_toml_is_rejected_with_underlying_error() {
        let err = parse_project_verify("this is not : valid toml [[[").unwrap_err();
        assert!(
            err.contains("invalid TOML"),
            "error should say it's a TOML parse failure: {err}"
        );
    }

    // ---- load_project_verify ---------------------------------------------

    #[test]
    fn load_with_no_nerve_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_project_verify(dir.path()),
            ProjectVerify { extra: vec![] }
        );
    }

    #[test]
    fn load_with_no_verify_toml_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".nerve")).unwrap();
        assert_eq!(
            load_project_verify(dir.path()),
            ProjectVerify { extra: vec![] }
        );
    }

    #[test]
    fn load_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".nerve")).unwrap();
        std::fs::write(
            dir.path().join(".nerve").join("verify.toml"),
            r#"extra = ["npm run build"]"#,
        )
        .unwrap();
        assert_eq!(
            load_project_verify(dir.path()),
            ProjectVerify {
                extra: vec!["npm run build".to_string()]
            }
        );
    }

    #[test]
    fn load_malformed_file_is_empty_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".nerve")).unwrap();
        std::fs::write(
            dir.path().join(".nerve").join("verify.toml"),
            "this is not : valid toml [[[",
        )
        .unwrap();
        assert_eq!(
            load_project_verify(dir.path()),
            ProjectVerify { extra: vec![] }
        );
    }

    #[test]
    fn load_file_with_bad_extra_shape_is_empty_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".nerve")).unwrap();
        std::fs::write(
            dir.path().join(".nerve").join("verify.toml"),
            r#"extra = ["   "]"#,
        )
        .unwrap();
        assert_eq!(
            load_project_verify(dir.path()),
            ProjectVerify { extra: vec![] }
        );
    }

    // ---- compose_gate -------------------------------------------------------

    #[test]
    fn compose_gate_no_extra_returns_base_unchanged() {
        assert_eq!(compose_gate("a && b", &[]), "a && b");
    }

    #[test]
    fn compose_gate_appends_extra_in_order() {
        assert_eq!(
            compose_gate("a", &["x".to_string(), "y".to_string()]),
            "a && x && y"
        );
    }

    #[test]
    fn compose_gate_skips_empty_entries_defensively() {
        assert_eq!(
            compose_gate("a", &["x".to_string(), "   ".to_string(), "y".to_string()]),
            "a && x && y"
        );
    }
}
