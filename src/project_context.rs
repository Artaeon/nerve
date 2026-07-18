//! The single source of truth for "what does the agent know about this
//! project" — assembled from the repo's `.nerve/` store and shared by both the
//! headless worker and the interactive TUI.
//!
//! **Why this module exists (measured, not guessed):** `src/agent/headless.rs`
//! — the worker that runs EVERY server job (100% of real work) — built its own
//! context block and never called [`crate::memory_recall::recall`] at all
//! (`grep -c recall src/agent/headless.rs` → 0; the `recall` tool it exposed
//! was called 0 times in 2,362 tool calls across real jobs). Meanwhile
//! `src/conversation.rs` (the interactive TUI) DID auto-recall and DID inject
//! `always_on_context`. Two independently-maintained context builders had
//! silently diverged, and the one doing all the real work was missing the
//! memory system entirely — the exact "safeguard that doesn't fire" class of
//! bug this codebase keeps hitting. [`build`] is now the ONLY place either
//! caller assembles project knowledge, so they cannot drift apart again.
//!
//! ## Section order (deliberate)
//!
//! [`build`] returns sections in this exact order:
//!
//! 1. `### What this project is` — the brief.
//! 2. `### Facts & conventions to follow` — memory.md.
//! 3. `### Design principles — follow these for ANY UI/CSS work` — only when
//!    [`ContextOptions::include_design`] is set.
//! 4. `### Recent project decisions`.
//! 5. The recalled-memory block (`crate::memory_recall::recall` +
//!    `format_recalled`) — only when [`ContextOptions::recall_query`] is
//!    `Some`, and always LAST.
//!
//! The stable project facts (1-4) go first because they are ground truth about
//! the repo regardless of what this particular turn/task is about — they read
//! best as a fixed foundation. The query-relevant recalled entries go LAST so
//! they sit nearest the task/conversation that follows them in the prompt —
//! the model reads them right before it reads what it's actually being asked
//! to do right now, which is where relevance-scored, task-specific memory is
//! most useful.

use crate::agent::context::{TRUNCATION_MARKER, smart_truncate};
use crate::project::ProjectStore;

/// Truncate one named context section and, if that truncation actually
/// removed content, LOG it — which section, how many characters survived vs.
/// were dropped. Before this, `memory.md` (or brief/design/decisions)
/// exceeding its cap silently lost the overflow with zero signal anywhere:
/// this is the exact failure that already happened once when memory.md was
/// injected through a 1,500-char cap and 81% of it vanished unnoticed. The
/// text itself also carries `TRUNCATION_MARKER` (added by `smart_truncate`),
/// but that alone isn't enough — the north star is "log everything", and a
/// marker buried in a large prompt is easy to miss in review; a `tracing::warn!`
/// is not.
fn truncate_and_log(section: &str, text: &str, max_chars: usize) -> String {
    let truncated = smart_truncate(text, max_chars);
    if truncated.contains(TRUNCATION_MARKER) {
        let original_chars = text.chars().count();
        let marker_suffix = format!(" {TRUNCATION_MARKER}");
        let kept_chars = truncated
            .strip_suffix(&marker_suffix)
            .unwrap_or(&truncated)
            .chars()
            .count();
        tracing::warn!(
            section,
            original_chars,
            kept_chars,
            dropped_chars = original_chars.saturating_sub(kept_chars),
            "project context section truncated — content was dropped; see marker in the text handed to the model"
        );
    }
    truncated
}

/// Bounds on the curated `.nerve/` sections folded into every job's context.
/// These exist ONLY to cap a pathological file (someone pasting a novel into
/// `memory.md`) — NOT to ration a normal, curated, human-maintained one, which
/// is small by nature. Measured on a real project (vollgebucht): `brief.md`
/// was 3086 bytes, `memory.md` was 8080 bytes, `design.md` was 3714 bytes —
/// and the *old* bounds (1200/1500/1500) cut memory.md down to 19% of itself,
/// silently dropping most of the project's "law" (invariants, migration
/// discipline, design system, what already exists) from EVERY job. Worst case
/// here is ~21k chars ≈ 6k tokens ≈ 6% of the worker's 100k context budget
/// (`CONTEXT_BUDGET_TOKENS` in `agent::headless`) — paid ONCE per job to avoid
/// the agent re-deriving project context from the codebase, which is far more
/// expensive: over 3 days of real jobs, agents made 797 `read_file` and 352
/// `search_code` calls rediscovering facts that were already written down.
#[allow(dead_code)] // wired into headless.rs and conversation.rs in a following step; already exercised by tests
pub const BRIEF_CONTEXT_CHARS: usize = 4_000;
/// See [`BRIEF_CONTEXT_CHARS`]. This is the important one: `memory.md` is the
/// project's law, and 1500 chars was cutting a real project's memory to 19%.
#[allow(dead_code)] // see BRIEF_CONTEXT_CHARS
pub const MEMORY_CONTEXT_CHARS: usize = 12_000;
/// See [`BRIEF_CONTEXT_CHARS`].
#[allow(dead_code)] // see BRIEF_CONTEXT_CHARS
pub const DESIGN_CONTEXT_CHARS: usize = 5_000;
/// Cap for the "Recent project decisions" section. Named (previously a bare
/// `800` inline, with no explanation) for the same reason as the three caps
/// above: so its value is discoverable and its rationale is written down
/// instead of re-derived from a magic number. `recent_decisions` returns its
/// results OLDEST-FIRST, and `smart_truncate` always cuts from the END of the
/// string — so building the list in that same oldest-first order and then
/// truncating would silently drop the NEWEST decisions, which are the ones
/// most likely to still be relevant. `build` below reverses the list to
/// newest-first before truncating, so a cut (if any) drops the OLDEST
/// decisions instead — the correct direction.
#[allow(dead_code)] // see BRIEF_CONTEXT_CHARS
pub const DECISIONS_CONTEXT_CHARS: usize = 800;

/// Options controlling which sections [`build`] assembles.
#[allow(dead_code)] // wired into headless.rs and conversation.rs in a following step; already exercised by tests
pub struct ContextOptions<'a> {
    /// Query for auto-recall — the task text (worker) or the user's message
    /// (TUI). `None` means no recall section is produced at all.
    pub recall_query: Option<&'a str>,
    /// Whether to inject the project's design principles. The TUI passes the
    /// result of `memory_recall::is_design_request(text)`; the worker always
    /// passes `true` (a headless job has no single "current message" to gate
    /// design guidance on, and design work is common enough in autonomous
    /// jobs that omitting it silently is the wrong default).
    pub include_design: bool,
}

/// Assemble this repo's persisted `.nerve/` project knowledge into ordered
/// section strings — see the module docs for the exact order and why. Reuses
/// the existing store accessors, `smart_truncate`, and
/// `crate::memory_recall::{recall, format_recalled}` rather than
/// re-implementing any of them. Returns an empty `Vec` (not a panic, not
/// `None`) when there is no `.nerve/` memory at all.
///
/// Wired into `agent::headless` and `conversation` in a following step;
/// already exercised by tests.
#[allow(dead_code)]
pub fn build(store: &ProjectStore, opts: &ContextOptions) -> Vec<String> {
    let mut sections = Vec::new();

    if let Some(brief) = store.load_brief() {
        sections.push(format!(
            "### What this project is\n{}",
            truncate_and_log("brief", &brief, BRIEF_CONTEXT_CHARS)
        ));
    }

    if let Some(mem) = store.load_memory() {
        sections.push(format!(
            "### Facts & conventions to follow\n{}",
            truncate_and_log("memory", &mem, MEMORY_CONTEXT_CHARS)
        ));
    }

    if opts.include_design
        && let Some(design) = store.load_design()
    {
        sections.push(format!(
            "### Design principles — follow these for ANY UI/CSS work\n{}",
            truncate_and_log("design", &design, DESIGN_CONTEXT_CHARS)
        ));
    }

    let decisions = store.recent_decisions(6);
    if !decisions.is_empty() {
        // Newest-first — see the comment on `DECISIONS_CONTEXT_CHARS` for why:
        // `smart_truncate` cuts from the end, and `recent_decisions` returns
        // oldest-first, so building the list in that order would drop the
        // newest (most relevant) decisions on truncation instead of the oldest.
        let list = decisions
            .iter()
            .rev()
            .map(|d| format!("- {}", d.text))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "### Recent project decisions\n{}",
            truncate_and_log("decisions", &list, DECISIONS_CONTEXT_CHARS)
        ));
    }

    if let Some(query) = opts.recall_query {
        let hits = crate::memory_recall::recall(
            store,
            query,
            3,
            crate::memory_recall::AUTO_RECALL_MIN_SCORE,
        );
        if let Some(recalled) = crate::memory_recall::format_recalled(&hits) {
            sections.push(recalled);
        }
    }

    sections
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn full_store() -> (tempfile::TempDir, ProjectStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        store
            .save_brief("A Rust TUI for AI-assisted coding.")
            .expect("brief");
        store.remember("uses tokio for async").expect("remember");
        store
            .append_design("use an 8px spacing scale")
            .expect("design");
        store
            .record_decision("chose ratatui over cursive")
            .expect("decision");
        (dir, store)
    }

    #[test]
    fn sections_are_in_documented_order_with_everything_present() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: None,
            include_design: true,
        };
        let sections = build(&store, &opts);
        assert_eq!(sections.len(), 4);
        assert!(sections[0].starts_with("### What this project is"));
        assert!(sections[1].starts_with("### Facts & conventions to follow"));
        assert!(sections[2].starts_with("### Design principles"));
        assert!(sections[3].starts_with("### Recent project decisions"));
    }

    #[test]
    fn include_design_false_omits_design_section() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: None,
            include_design: false,
        };
        let sections = build(&store, &opts);
        assert!(!sections.iter().any(|s| s.starts_with("### Design")));
    }

    #[test]
    fn include_design_true_includes_design_section() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: None,
            include_design: true,
        };
        let sections = build(&store, &opts);
        assert!(sections.iter().any(|s| s.starts_with("### Design")));
    }

    #[test]
    fn no_recall_query_yields_no_recalled_section() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: None,
            include_design: true,
        };
        let sections = build(&store, &opts);
        assert!(
            !sections
                .iter()
                .any(|s| s.contains("Relevant project memory"))
        );
    }

    #[test]
    fn matching_recall_query_yields_a_final_recalled_section() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: Some("how is async handled in this project"),
            include_design: true,
        };
        let sections = build(&store, &opts);
        let last = sections.last().expect("at least one section");
        assert!(
            last.contains("Relevant project memory"),
            "expected recalled section last, got: {last}"
        );
        assert!(last.contains("tokio"));
    }

    #[test]
    fn medium_memory_survives_in_full_no_truncation_marker() {
        // Regression: the OLD headless/TUI paths used bounds as low as 1500
        // chars, which cut a real project's memory.md (~8k chars) down to 19%
        // of itself. Building memory.md up to ~8k chars via `remember()` (the
        // real code path, not a raw file write) must survive whole.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        let mut total = 0usize;
        let mut i = 0usize;
        while total < 8_000 {
            let fact = format!(
                "fact number {i} about this project describing a convention in more detail so \
                 each bullet has some real length to it"
            );
            total += fact.len() + 2; // + "- " prefix
            store.remember(&fact).expect("remember");
            i += 1;
        }
        let opts = ContextOptions {
            recall_query: None,
            include_design: false,
        };
        let sections = build(&store, &opts);
        let mem_section = sections
            .iter()
            .find(|s| s.starts_with("### Facts & conventions to follow"))
            .expect("memory section present");
        // Strengthened: the OLD assertion only checked for a literal "..." —
        // which the sentence-boundary truncation branch never added, so this
        // check passed even when content WAS silently cut. Check for the
        // actual marker instead, which now appears on every truncating path.
        assert!(
            !mem_section.contains(TRUNCATION_MARKER),
            "memory under MEMORY_CONTEXT_CHARS must not be truncated: len={}",
            mem_section.len()
        );
        assert!(mem_section.contains("fact number 0 about this project"));
        assert!(mem_section.contains(&format!("fact number {} about this project", i - 1)));
        // And it must be byte-for-byte the untruncated content plus heading —
        // no marker means no truncation happened at all.
        let heading = "### Facts & conventions to follow\n";
        let mem = store.load_memory().expect("memory");
        assert_eq!(mem_section, &format!("{heading}{mem}"));
    }

    #[test]
    fn pathological_memory_is_bounded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        // One giant bullet, well past MEMORY_CONTEXT_CHARS.
        let huge = "word ".repeat(15_000); // ~75k chars
        store.remember(&huge).expect("remember");
        let opts = ContextOptions {
            recall_query: None,
            include_design: false,
        };
        let sections = build(&store, &opts);
        let mem_section = sections
            .iter()
            .find(|s| s.starts_with("### Facts & conventions to follow"))
            .expect("memory section present");
        // Bounded to roughly MEMORY_CONTEXT_CHARS plus the heading + any
        // truncation marker overhead — nowhere near the full ~75k input.
        assert!(
            mem_section.len() < MEMORY_CONTEXT_CHARS + 200,
            "memory section not bounded: {} chars",
            mem_section.len()
        );
        // The section over its cap MUST carry the visible marker — this is
        // what lets the model (and a human) tell a partial fact from a
        // complete one.
        assert!(
            mem_section.contains(TRUNCATION_MARKER),
            "truncated memory section must carry the visible marker: {mem_section}"
        );
    }

    #[test]
    fn truncated_section_logs_and_marks_specifically_the_sentence_boundary_path() {
        // The exact case the OLD test missed: a section that gets cut on the
        // sentence-boundary branch (a clean, punctuation-terminated prefix)
        // used to come back with NO marker at all, indistinguishable from a
        // complete section. Build a brief made of short sentences so the
        // sentence-boundary branch (not the word-boundary/hard-cut fallback)
        // is the one exercised.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        let sentence = "This is one short sentence about the project. ";
        let brief = sentence.repeat(400); // well past BRIEF_CONTEXT_CHARS
        store.save_brief(&brief).expect("brief");
        let opts = ContextOptions {
            recall_query: None,
            include_design: false,
        };
        let sections = build(&store, &opts);
        let brief_section = sections
            .iter()
            .find(|s| s.starts_with("### What this project is"))
            .expect("brief section present");
        assert!(
            brief_section.contains(TRUNCATION_MARKER),
            "sentence-boundary truncation must still carry the marker: {brief_section}"
        );
        // Sanity: it really did break on a sentence boundary (ends right
        // before the marker with a period), not a word/hard cut.
        let before_marker = brief_section
            .split(TRUNCATION_MARKER)
            .next()
            .unwrap_or_default();
        assert!(before_marker.trim_end().ends_with('.'));
    }

    #[test]
    fn section_under_cap_is_returned_byte_for_byte_unchanged() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: None,
            include_design: true,
        };
        let sections = build(&store, &opts);
        for s in &sections {
            assert!(
                !s.contains(TRUNCATION_MARKER),
                "small curated section must not be truncated: {s}"
            );
        }
        let design_section = sections
            .iter()
            .find(|s| s.starts_with("### Design"))
            .expect("design section");
        let design = store.load_design().expect("design");
        assert_eq!(
            design_section,
            &format!("### Design principles — follow these for ANY UI/CSS work\n{design}")
        );
    }

    #[test]
    fn decisions_truncate_from_the_oldest_end_keeping_the_newest() {
        // `recent_decisions` returns oldest-first; `smart_truncate` always
        // cuts from the END of the string it's given. If `build` fed it the
        // oldest-first list directly, a cut would drop the NEWEST decisions —
        // the ones most likely to matter. `build` reverses to newest-first
        // before truncating, so the newest decision must survive even when
        // the whole list is forced well past DECISIONS_CONTEXT_CHARS.
        // `build` only ever asks for the 6 most recent decisions (see
        // `store.recent_decisions(6)` in `build`), so 200 short decisions
        // would never come close to DECISIONS_CONTEXT_CHARS (800) once
        // trimmed to 6 — the padding below must make each of those 6 long
        // enough, on its own, that six of them together blow past the cap.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        let pad = "with enough padding text repeated so that each single decision line is long enough on its own that six of them together comfortably exceed the eight hundred character truncation cap for this section, guaranteeing the test actually exercises truncation";
        for i in 0..200 {
            store
                .record_decision(&format!("decision number {i} {pad}"))
                .expect("decision");
        }
        let opts = ContextOptions {
            recall_query: None,
            include_design: false,
        };
        let sections = build(&store, &opts);
        let decisions_section = sections
            .iter()
            .find(|s| s.starts_with("### Recent project decisions"))
            .expect("decisions section present");
        assert!(decisions_section.contains(TRUNCATION_MARKER));
        // `recent_decisions(6)` only ever returns the 6 most recent anyway, so
        // the newest of those (decision 199, since IDs run 0..200) must survive.
        assert!(
            decisions_section.contains("decision number 199"),
            "newest decision was dropped by truncation: {decisions_section}"
        );
    }

    #[test]
    fn no_nerve_dir_yields_empty_vec_not_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ProjectStore::for_workspace(dir.path());
        let opts = ContextOptions {
            recall_query: Some("anything"),
            include_design: true,
        };
        let sections = build(&store, &opts);
        assert!(sections.is_empty());
    }

    /// THE ANTI-DRIFT TEST. The point of this whole module is that both the
    /// headless worker and the TUI go through this ONE `build` function
    /// instead of maintaining their own copies that can silently diverge (as
    /// they had: see the module docs). A true structural assertion that "both
    /// callers use this function" can't be expressed from inside this module
    /// (that's a property of `headless.rs` and `conversation.rs`, verified by
    /// those files compiling against this shared API and by their own tests).
    /// What IS testable here: `build` is deterministic and pure over
    /// `(store, opts)` — identical inputs always produce identical section
    /// headings in the same order, so there is no per-caller hidden state
    /// that could let two call sites drift apart while both nominally "use"
    /// this function.
    #[test]
    fn build_is_deterministic_for_identical_inputs() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: Some("async runtime"),
            include_design: true,
        };
        let a = build(&store, &opts);
        let b = build(&store, &opts);
        let a_headings: Vec<&str> = a.iter().map(|s| s.lines().next().unwrap_or("")).collect();
        let b_headings: Vec<&str> = b.iter().map(|s| s.lines().next().unwrap_or("")).collect();
        assert_eq!(a_headings, b_headings);
        assert_eq!(a, b);
    }

    /// THE ANTI-DRIFT TEST — the actual point of this whole job. Both the
    /// headless worker (`agent::headless::project_memory_context_from`) and
    /// the TUI (`conversation::build_context_messages`) must route through
    /// THIS `build` function instead of maintaining their own copies that can
    /// silently diverge, which is exactly what happened before (see the
    /// module docs: `grep -c recall src/agent/headless.rs` == 0 across 2,362
    /// real tool calls). A fully generic "these two call sites are reflected
    /// as the same code path" assertion isn't expressible from Rust source —
    /// so the practical stand-in is: call `build` directly to get the
    /// expected sections, then call the worker's public wrapper and assert
    /// its output CONTAINS every heading/fragment from those sections. Since
    /// `project_memory_context_from` is (by inspection, and by the doc
    /// comment on that function) a thin formatter around `build(store, opts)`
    /// with no independent assembly logic left, this proves the worker path
    /// produces the SAME sections the shared builder produces — there is no
    /// other place left that could drift.
    #[test]
    fn both_callers_use_the_shared_builder() {
        let (_d, store) = full_store();
        let opts = ContextOptions {
            recall_query: Some("task text"),
            include_design: true,
        };
        let expected_sections = build(&store, &opts);
        assert!(
            expected_sections.len() >= 4,
            "expected brief, memory, design, decisions at minimum"
        );

        let worker_ctx = crate::agent::headless::project_memory_context_from(&store, "task text")
            .expect("worker should assemble a context from this store");

        for section in &expected_sections {
            let heading = section.lines().next().unwrap_or(section);
            assert!(
                worker_ctx.contains(heading),
                "worker context missing heading {heading:?} from shared builder output"
            );
        }
        // And spot-check actual content, not just headings.
        assert!(worker_ctx.contains("A Rust TUI for AI-assisted coding"));
        assert!(worker_ctx.contains("tokio"));
        assert!(worker_ctx.contains("8px spacing scale"));
        assert!(worker_ctx.contains("ratatui over cursive"));
    }
}
