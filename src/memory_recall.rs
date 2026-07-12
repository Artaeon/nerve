//! Retrieval over per-project memory — the "principles database".
//!
//! The naive approach (force-feeding all of `.nerve/` into every prompt) grows
//! without bound: the more nerve learns about a project, the more expensive
//! *every* turn becomes, until it silently truncates and drops facts. This
//! module flips that model from *push* to *pull*:
//!
//!   * [`always_on_context`] injects only a tiny, fixed header every turn — a
//!     one-paragraph project headline, open tasks, and a pointer to the rest.
//!   * [`recall`] searches the stored facts/decisions/improvements and returns
//!     only the entries relevant to the query, so token cost scales with
//!     *relevance*, not with how much memory has accumulated.
//!
//! Retrieval reuses the same BM25-style engine that powers the knowledge base
//! ([`crate::knowledge::search`]) — we build an ephemeral in-memory index over
//! the memory entries rather than maintaining a second search implementation.

use crate::knowledge::search::search_knowledge;
use crate::knowledge::store::{Chunk, Document, KnowledgeBase};
use crate::project::ProjectStore;

/// Minimum relevance score for a memory entry to be auto-injected on a turn.
/// A single solid keyword match on a short fact scores ~3; this keeps unrelated
/// memory out of the prompt while still surfacing genuine matches. Tunable.
pub const AUTO_RECALL_MIN_SCORE: f64 = 2.5;

/// Longest brief headline (chars) kept in the always-on header.
const BRIEF_HEADLINE_CHARS: usize = 400;

/// The category a memory entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Fact,
    Decision,
    Improvement,
}

impl EntryKind {
    fn label(self) -> &'static str {
        match self {
            EntryKind::Fact => "fact",
            EntryKind::Decision => "decision",
            EntryKind::Improvement => "improvement",
        }
    }

    fn from_label(label: &str) -> Self {
        match label {
            "decision" => EntryKind::Decision,
            "improvement" => EntryKind::Improvement,
            _ => EntryKind::Fact,
        }
    }
}

/// One searchable unit of project memory.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub kind: EntryKind,
    pub text: String,
}

/// A memory entry that matched a recall query. Results are returned best match
/// first; the numeric score is used only for ranking/thresholding internally.
#[derive(Debug, Clone)]
pub struct RecalledEntry {
    pub kind: EntryKind,
    pub text: String,
}

/// Collect every searchable memory entry from the store: facts (the bullets in
/// `memory.md`), all recorded decisions, and still-open improvements.
pub fn collect_entries(store: &ProjectStore) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();

    if let Some(memory) = store.load_memory() {
        for line in memory.lines() {
            if let Some(rest) = line.trim().strip_prefix("- ") {
                let text = rest.trim();
                if !text.is_empty() {
                    entries.push(MemoryEntry {
                        kind: EntryKind::Fact,
                        text: text.to_string(),
                    });
                }
            }
        }
    }

    for decision in store.all_decisions() {
        entries.push(MemoryEntry {
            kind: EntryKind::Decision,
            text: decision.text,
        });
    }

    for improvement in store.list_improvements() {
        if improvement.status == "open" {
            entries.push(MemoryEntry {
                kind: EntryKind::Improvement,
                text: improvement.text,
            });
        }
    }

    entries
}

/// Search project memory for entries relevant to `query`. Returns up to
/// `max_results` entries whose relevance score is at least `min_score`, best
/// match first. An empty query or empty store yields no results.
pub fn recall(
    store: &ProjectStore,
    query: &str,
    max_results: usize,
    min_score: f64,
) -> Vec<RecalledEntry> {
    recall_in(&collect_entries(store), query, max_results, min_score)
}

/// Common English words stripped from a recall query so matching is driven by
/// meaningful terms. Without this, a query like "what is the database pool size"
/// scores flat hits on "the"/"is" against *every* stored fact, so unrelated
/// entries clear the relevance threshold and pollute the prompt.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "did", "do", "does", "for", "from",
    "get", "got", "how", "i", "in", "is", "it", "its", "me", "my", "of", "on", "or", "our",
    "should", "that", "the", "this", "to", "was", "we", "were", "what", "when", "where", "which",
    "who", "why", "will", "with", "would", "you", "your",
];

/// Drop stopwords from a query. Falls back to the original when every word is a
/// stopword, so a terse query never becomes empty (and matches nothing).
fn meaningful_query(query: &str) -> String {
    let kept: Vec<&str> = query
        .split_whitespace()
        .filter(|w| {
            let bare = w
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            !bare.is_empty() && !STOPWORDS.contains(&bare.as_str())
        })
        .collect();
    if kept.is_empty() {
        query.to_string()
    } else {
        kept.join(" ")
    }
}

/// Signal words that mark a request as UI/design work. Kept lowercase; matched
/// on whole words so "designer" or "restyle" don't accidentally hide/expose the
/// design principles.
const DESIGN_SIGNALS: &[&str] = &[
    "design",
    "ui",
    "css",
    "layout",
    "style",
    "styling",
    "component",
    "page",
    "tailwind",
    "html",
    "color",
    "colour",
    "font",
    "typography",
    "spacing",
    "theme",
    "button",
    "header",
    "footer",
    "hero",
    "landing",
    "bento",
    "responsive",
];

/// True when `text` looks like a UI/design request, so the project's design
/// principles are worth injecting. Matches whole words only (after lowercasing),
/// so backend-only turns don't waste tokens on design guidance.
pub fn is_design_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| DESIGN_SIGNALS.contains(&word))
}

/// The searchable core of [`recall`], separated so it can be unit-tested
/// without touching the filesystem.
fn recall_in(
    entries: &[MemoryEntry],
    query: &str,
    max_results: usize,
    min_score: f64,
) -> Vec<RecalledEntry> {
    if entries.is_empty() || max_results == 0 {
        return Vec::new();
    }
    let query = meaningful_query(query);
    let kb = build_index(entries);
    search_knowledge(&kb, &query, max_results)
        .into_iter()
        .filter(|r| r.score >= min_score)
        .map(|r| RecalledEntry {
            kind: EntryKind::from_label(&r.document_title),
            text: r.chunk.content,
        })
        .collect()
}

/// Build an ephemeral knowledge base with one document/chunk per memory entry,
/// tagging each document's title with the entry's kind so the kind survives the
/// round-trip through [`search_knowledge`].
fn build_index(entries: &[MemoryEntry]) -> KnowledgeBase {
    let mut kb = KnowledgeBase::new("__project_memory__".into());
    for (i, entry) in entries.iter().enumerate() {
        let id = i.to_string();
        let word_count = entry.text.split_whitespace().count();
        kb.add_document(
            Document {
                id: id.clone(),
                title: entry.kind.label().to_string(),
                source_path: String::new(),
                ingested_at: chrono::Utc::now(),
                word_count,
            },
            vec![Chunk {
                id: id.clone(),
                document_id: id,
                content: entry.text.clone(),
                index: 0,
                word_count,
            }],
        );
    }
    kb
}

/// Format recalled entries as a compact system message for prompt injection.
/// Returns `None` when there is nothing to show.
pub fn format_recalled(entries: &[RecalledEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let lines = entries
        .iter()
        .map(|e| format!("- ({}) {}", e.kind.label(), e.text))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "Relevant project memory (retrieved for this turn):\n{lines}"
    ))
}

/// The small, always-injected memory header: a one-paragraph project headline,
/// any open tasks, and a pointer to the searchable memory. Heavy content (every
/// fact and decision) is deliberately excluded — it is retrieved on demand via
/// [`recall`]. Returns `None` when there is nothing worth injecting.
pub fn always_on_context(store: &ProjectStore, max_chars: usize) -> Option<String> {
    let mut sections: Vec<String> = Vec::new();

    if let Some(brief) = store.load_brief() {
        let headline = brief_headline(&brief, BRIEF_HEADLINE_CHARS);
        if !headline.is_empty() {
            sections.push(format!("## Project\n{headline}"));
        }
    }

    let open_tasks: Vec<_> = store
        .list_tasks()
        .into_iter()
        .filter(|t| t.status == "pending" || t.status == "in_progress")
        .collect();
    if !open_tasks.is_empty() {
        let lines = open_tasks
            .iter()
            .map(|t| format!("- [#{}] {} ({})", t.id, t.title, t.status))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Open tasks\n{lines}"));
    }

    // Auto-captured record of what recent turns worked on, so a fresh session
    // opens with a sense of what was just being done. Kept tiny — last 3 only.
    let activity = store.recent_activity(3);
    if !activity.is_empty() {
        let lines = activity
            .iter()
            .rev()
            .map(|a| {
                let edited = if a.edited { "yes" } else { "no" };
                let mut line = format!("- {} (edited: {edited})", a.request);
                // Prefer the semantic summary when present — it says what
                // actually happened, not just what was asked.
                if !a.summary.is_empty() {
                    line.push_str(&format!("\n  → {}", first_sentence(&a.summary)));
                }
                if !a.files.is_empty() {
                    line.push_str(&format!("\n  files: {}", a.files.join(", ")));
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Recent activity\n{lines}"));
    }

    // Index line: advertise what is retrievable without spending the tokens to
    // include it. The model pulls the relevant pieces via the `recall` tool.
    let fact_count = collect_entries(store)
        .iter()
        .filter(|e| e.kind != EntryKind::Improvement)
        .count();
    if fact_count > 0 {
        sections.push(format!(
            "## Memory\n{fact_count} remembered fact(s) and past decision(s) are on file \
             for this project. Use the `recall` tool with a query to retrieve the ones \
             relevant to your current task instead of guessing."
        ));
    }

    if sections.is_empty() {
        return None;
    }

    let mut ctx = format!(
        "Project memory (persisted knowledge about this repository — treat as \
         ground truth unless the code contradicts it):\n\n{}",
        sections.join("\n\n")
    );
    if ctx.len() > max_chars {
        let truncated: String = ctx.chars().take(max_chars).collect();
        ctx = format!("{truncated}\n[truncated]");
    }
    Some(ctx)
}

/// The first meaningful paragraph of a brief, collapsed to a single line and
/// capped to `max_chars` (at a char boundary, never mid-UTF-8). Markdown
/// heading lines are dropped, so a brief that opens with a `# Title` still
/// yields the first real sentence of prose.
fn brief_headline(brief: &str, max_chars: usize) -> String {
    // Strip heading lines within each paragraph, then take the first paragraph
    // that still has prose — a heading-only paragraph (e.g. a leading `# Title`)
    // is skipped rather than returned empty.
    let cleaned = brief
        .split("\n\n")
        .map(|para| {
            para.lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .find(|p| !p.is_empty())
        .unwrap_or_default();

    if cleaned.chars().count() > max_chars {
        let head: String = cleaned.chars().take(max_chars).collect();
        format!("{}…", head.trim_end())
    } else {
        cleaned
    }
}

/// First sentence (or first line) of a summary, capped, for a one-line preview
/// in the always-on activity list. Keeps the journal detail rich on disk while
/// the header stays tiny.
fn first_sentence(summary: &str) -> String {
    let flat = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    let end = flat
        .find(". ")
        .map(|i| i + 1)
        .unwrap_or_else(|| flat.len().min(160));
    let s = flat[..end.min(flat.len())].trim_end();
    if s.chars().count() > 160 {
        let head: String = s.chars().take(160).collect();
        format!("{}…", head.trim_end())
    } else {
        s.to_string()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<MemoryEntry> {
        vec![
            MemoryEntry {
                kind: EntryKind::Fact,
                text: "the project uses tokio for all async runtime work".into(),
            },
            MemoryEntry {
                kind: EntryKind::Fact,
                text: "tests live in the same file as the code they cover".into(),
            },
            MemoryEntry {
                kind: EntryKind::Decision,
                text: "chose ratatui over cursive for the terminal ui".into(),
            },
            MemoryEntry {
                kind: EntryKind::Improvement,
                text: "add integration tests for the websocket layer".into(),
            },
        ]
    }

    #[test]
    fn recall_surfaces_the_relevant_entry() {
        let hits = recall_in(&entries(), "how is async handled", 3, 0.0);
        assert!(!hits.is_empty());
        assert!(hits[0].text.contains("tokio"));
        assert_eq!(hits[0].kind, EntryKind::Fact);
    }

    #[test]
    fn recall_matches_decisions_by_keyword() {
        let hits = recall_in(&entries(), "which ui library did we pick", 3, 0.0);
        assert!(hits.iter().any(|h| h.text.contains("ratatui")));
        assert!(hits.iter().any(|h| h.kind == EntryKind::Decision));
    }

    #[test]
    fn recall_respects_the_result_cap() {
        let hits = recall_in(&entries(), "the project tests tokio ratatui", 2, 0.0);
        assert!(hits.len() <= 2);
    }

    #[test]
    fn stopwords_do_not_pollute_recall() {
        // Query about the pool must NOT surface the unrelated svelte fact just
        // because both share the stopwords "the"/"is".
        let hits = recall_in(
            &entries(),
            "what is the async runtime",
            3,
            AUTO_RECALL_MIN_SCORE,
        );
        assert!(hits.iter().any(|h| h.text.contains("tokio")));
        assert!(!hits.iter().any(|h| h.text.contains("svelte")));
    }

    #[test]
    fn meaningful_query_strips_stopwords_but_keeps_terms() {
        assert_eq!(
            meaningful_query("what is the database pool"),
            "database pool"
        );
        // All-stopword queries fall back to the original.
        assert_eq!(meaningful_query("what is the"), "what is the");
    }

    #[test]
    fn is_design_request_detects_ui_work_only() {
        assert!(is_design_request("redesign the header"));
        assert!(is_design_request("fix the css layout"));
        assert!(is_design_request("build a landing page hero"));
        assert!(!is_design_request("fix the booking engine bug"));
        assert!(!is_design_request("add a database migration"));
    }

    #[test]
    fn recall_threshold_filters_weak_matches() {
        // A query with no meaningful overlap should clear nothing at the
        // auto-recall threshold.
        let hits = recall_in(
            &entries(),
            "quantum blockchain cryptography",
            3,
            AUTO_RECALL_MIN_SCORE,
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn recall_empty_query_or_store_is_empty() {
        assert!(recall_in(&entries(), "", 3, 0.0).is_empty());
        assert!(recall_in(&[], "tokio", 3, 0.0).is_empty());
        assert!(recall_in(&entries(), "tokio", 0, 0.0).is_empty());
    }

    #[test]
    fn collect_entries_reads_all_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        store.remember("uses tokio for async").unwrap();
        store.record_decision("chose ratatui").unwrap();
        let open = store.add_improvement("speed up startup").unwrap();
        let done = store.add_improvement("already handled").unwrap();
        store.complete_improvement(done).unwrap();

        let collected = collect_entries(&store);
        assert!(
            collected
                .iter()
                .any(|e| e.kind == EntryKind::Fact && e.text.contains("tokio"))
        );
        assert!(
            collected
                .iter()
                .any(|e| e.kind == EntryKind::Decision && e.text.contains("ratatui"))
        );
        // Only the OPEN improvement is included.
        let improvements: Vec<_> = collected
            .iter()
            .filter(|e| e.kind == EntryKind::Improvement)
            .collect();
        assert_eq!(improvements.len(), 1);
        assert_eq!(improvements[0].text, "speed up startup");
        let _ = open;
    }

    #[test]
    fn format_recalled_labels_each_entry() {
        let out = format_recalled(&[
            RecalledEntry {
                kind: EntryKind::Fact,
                text: "uses tokio".into(),
            },
            RecalledEntry {
                kind: EntryKind::Decision,
                text: "chose ratatui".into(),
            },
        ])
        .unwrap();
        assert!(out.contains("- (fact) uses tokio"));
        assert!(out.contains("- (decision) chose ratatui"));
    }

    #[test]
    fn format_recalled_empty_is_none() {
        assert!(format_recalled(&[]).is_none());
    }

    #[test]
    fn always_on_context_is_small_and_advertises_recall() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        store
            .save_brief("# My Project\n\nA Rust TUI for AI-assisted coding. It streams responses and manages context.\n\nMore detail here that should NOT appear in the headline.")
            .unwrap();
        store.remember("uses tokio for async").unwrap();
        store.record_decision("chose ratatui").unwrap();
        store.add_task("port the renderer").unwrap();

        let ctx = always_on_context(&store, 1200).unwrap();
        // Headline present, later paragraphs excluded.
        assert!(ctx.contains("A Rust TUI for AI-assisted coding"));
        assert!(!ctx.contains("should NOT appear"));
        // Open task present.
        assert!(ctx.contains("[#1] port the renderer"));
        // Memory advertised, not dumped.
        assert!(ctx.contains("recall"));
        assert!(!ctx.contains("uses tokio"));
        assert!(!ctx.contains("chose ratatui"));
    }

    #[test]
    fn always_on_context_surfaces_recent_activity() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        store
            .record_activity("first request", false, "none")
            .unwrap();
        store
            .record_activity("add the feature", true, "none")
            .unwrap();

        let ctx = always_on_context(&store, 1200).unwrap();
        assert!(ctx.contains("## Recent activity"));
        // Newest first, with edited flag rendered.
        assert!(ctx.contains("- add the feature (edited: yes)"));
        assert!(ctx.contains("- first request (edited: no)"));
        assert!(
            ctx.find("add the feature").unwrap() < ctx.find("first request").unwrap(),
            "most recent activity should be listed first"
        );
    }

    #[test]
    fn always_on_context_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        assert!(always_on_context(&store, 1200).is_none());
    }

    #[test]
    fn always_on_context_omits_tasks_when_none_open() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        store.save_brief("A Rust TUI.").unwrap();
        let id = store.add_task("finish this").unwrap();
        store.set_task_status(id, "done").unwrap();
        let ctx = always_on_context(&store, 1200).unwrap();
        assert!(!ctx.contains("Open tasks"));
    }

    #[test]
    fn brief_headline_strips_heading_and_collapses() {
        let h = brief_headline(
            "# Title\n\nFirst para line one.\nLine two.\n\nSecond para.",
            400,
        );
        assert_eq!(h, "First para line one. Line two.");
    }

    #[test]
    fn brief_headline_caps_length() {
        let h = brief_headline(&"word ".repeat(200), 50);
        assert!(h.chars().count() <= 51); // 50 + ellipsis
        assert!(h.ends_with('…'));
    }
}
