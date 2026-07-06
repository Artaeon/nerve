//! Per-project persistent memory: the `.nerve/` directory.
//!
//! Nerve remembers a project across sessions the way an engineer does — a
//! short brief of what the project is, accumulated facts and conventions, the
//! decisions that were made, and a backlog of improvement ideas. Everything is
//! plain text / JSONL inside the repository so it is inspectable, editable and
//! (if the user wants) versionable:
//!
//! ```text
//! .nerve/
//!   memory.md         # facts & conventions, one bullet per line (/remember)
//!   brief.md          # engineering brief of the repo (/init)
//!   decisions.jsonl   # append-only log of decisions {timestamp, text}
//!   journal.jsonl     # append-only change journal {timestamp, tool, path, summary}
//!   improvements.json # improvement backlog [{id, text, status, created}]
//!   tasks.json        # task backlog [{id, title, status, created, updated}]
//! ```
//!
//! Memory is *retrieved*, not force-fed: [`crate::memory_recall`] injects a
//! tiny always-on header (project headline + open tasks + a pointer) and pulls
//! only the facts/decisions relevant to each turn via a BM25 search over this
//! store. Token cost then scales with relevance, not with how much has
//! accumulated. No embeddings, no magic — grounded, cheap and predictable.
//!
//! Security: `.nerve/` is a protected write target for agent tools (see
//! `shell::is_protected_write_target`) so a prompt-injected model cannot
//! poison the project memory via `write_file`/`edit_file`. All writes go
//! through this module's API, reachable only from user commands and the
//! dedicated `remember` tool (which appends inert bullet text).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::files::atomic_write;

/// The statuses a task in the backlog may hold.
const TASK_STATUSES: [&str; 4] = ["pending", "in_progress", "done", "failed"];

/// A single recorded decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub timestamp: String,
    pub text: String,
}

/// A single journaled agent change (successful write/edit/mkdir).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub timestamp: String,
    pub tool: String,
    pub path: String,
    pub summary: String,
}

/// A single auto-captured record of what a turn worked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub timestamp: String,
    pub request: String,
    pub edited: bool,
    /// "passed" | "failed" | "none".
    pub verify: String,
}

/// A backlog entry in the improvements directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    pub id: u64,
    pub text: String,
    /// "open" or "done".
    pub status: String,
    pub created: String,
}

/// A task in the persistent per-project backlog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTask {
    pub id: u64,
    pub title: String,
    /// One of "pending" | "in_progress" | "done" | "failed".
    pub status: String,
    pub created: String,
    pub updated: String,
}

/// Handle to a project's `.nerve/` memory directory.
#[derive(Debug, Clone)]
pub struct ProjectStore {
    dir: PathBuf,
}

impl ProjectStore {
    /// Store for the workspace rooted at `root`. Does not touch the disk.
    pub fn for_workspace(root: &Path) -> Self {
        Self {
            dir: root.join(".nerve"),
        }
    }

    /// True when this project has any persisted memory at all.
    #[cfg(test)]
    pub fn exists(&self) -> bool {
        self.dir.is_dir()
    }

    fn ensure_dir(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    // ── memory.md ────────────────────────────────────────────────────────

    pub fn memory_path(&self) -> PathBuf {
        self.dir.join("memory.md")
    }

    /// The raw contents of `memory.md`, if present and non-empty.
    pub fn load_memory(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.memory_path()).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Append a fact/convention to `memory.md` (creating it with a header).
    pub fn remember(&self, fact: &str) -> anyhow::Result<()> {
        let fact = sanitize_line(fact);
        if fact.is_empty() {
            anyhow::bail!("cannot remember an empty fact");
        }
        self.ensure_dir()?;
        let path = self.memory_path();
        let mut content = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            "# Project memory\n\nFacts and conventions nerve has learned about this project.\n"
                .into()
        });
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("- {fact}\n"));
        atomic_write(&path, &content)
    }

    // ── brief.md ─────────────────────────────────────────────────────────

    pub fn brief_path(&self) -> PathBuf {
        self.dir.join("brief.md")
    }

    /// The engineering brief, if one was generated.
    pub fn load_brief(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.brief_path()).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Save/replace the engineering brief.
    /// Wired to the upcoming `/init` command; already exercised by tests.
    #[allow(dead_code)]
    pub fn save_brief(&self, brief: &str) -> anyhow::Result<()> {
        self.ensure_dir()?;
        atomic_write(&self.brief_path(), brief.trim())
    }

    // ── design.md ────────────────────────────────────────────────────────

    pub fn design_path(&self) -> PathBuf {
        self.dir.join("design.md")
    }

    /// The project's design principles / design-system notes, if any.
    pub fn load_design(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.design_path()).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Append a design principle to `design.md` (creating it with a header).
    pub fn append_design(&self, principle: &str) -> anyhow::Result<()> {
        let principle = sanitize_line(principle);
        if principle.is_empty() {
            anyhow::bail!("cannot add an empty design principle");
        }
        self.ensure_dir()?;
        let path = self.design_path();
        let mut content = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            "# Design principles\n\nHow UI/design work should be done in this project.\n".into()
        });
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("- {principle}\n"));
        atomic_write(&path, &content)
    }

    // ── decisions.jsonl ──────────────────────────────────────────────────

    pub fn decisions_path(&self) -> PathBuf {
        self.dir.join("decisions.jsonl")
    }

    /// Append a decision to the append-only log.
    pub fn record_decision(&self, text: &str) -> anyhow::Result<()> {
        let text = sanitize_line(text);
        if text.is_empty() {
            anyhow::bail!("cannot record an empty decision");
        }
        self.ensure_dir()?;
        let decision = Decision {
            timestamp: chrono::Utc::now().to_rfc3339(),
            text,
        };
        let line = serde_json::to_string(&decision)?;
        let path = self.decisions_path();
        let mut content = std::fs::read_to_string(&path).unwrap_or_default();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&line);
        content.push('\n');
        atomic_write(&path, &content)
    }

    /// The most recent `n` decisions, oldest first.
    pub fn recent_decisions(&self, n: usize) -> Vec<Decision> {
        let Ok(content) = std::fs::read_to_string(self.decisions_path()) else {
            return Vec::new();
        };
        let all: Vec<Decision> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let skip = all.len().saturating_sub(n);
        all.into_iter().skip(skip).collect()
    }

    /// Every recorded decision, oldest first. Used by memory retrieval to index
    /// the full decision history for on-demand recall.
    pub fn all_decisions(&self) -> Vec<Decision> {
        let Ok(content) = std::fs::read_to_string(self.decisions_path()) else {
            return Vec::new();
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    // ── journal.jsonl ────────────────────────────────────────────────────

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("journal.jsonl")
    }

    /// Append a change record to the append-only change journal.
    pub fn record_change(&self, tool: &str, path: &str, summary: &str) -> anyhow::Result<()> {
        let summary = sanitize_line(summary);
        if summary.is_empty() {
            anyhow::bail!("cannot record a change with an empty summary");
        }
        self.ensure_dir()?;
        let record = ChangeRecord {
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool: tool.to_string(),
            path: path.to_string(),
            summary,
        };
        let line = serde_json::to_string(&record)?;
        let journal = self.journal_path();
        let mut content = std::fs::read_to_string(&journal).unwrap_or_default();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&line);
        content.push('\n');
        atomic_write(&journal, &content)
    }

    /// The most recent `n` journaled changes, oldest first.
    pub fn recent_changes(&self, n: usize) -> Vec<ChangeRecord> {
        let Ok(content) = std::fs::read_to_string(self.journal_path()) else {
            return Vec::new();
        };
        let all: Vec<ChangeRecord> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let skip = all.len().saturating_sub(n);
        all.into_iter().skip(skip).collect()
    }

    // ── activity.jsonl ───────────────────────────────────────────────────

    pub fn activity_path(&self) -> PathBuf {
        self.dir.join("activity.jsonl")
    }

    /// Append an auto-captured record of what a turn worked on.
    pub fn record_activity(&self, request: &str, edited: bool, verify: &str) -> anyhow::Result<()> {
        let request = sanitize_line(request);
        self.ensure_dir()?;
        let record = ActivityRecord {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request,
            edited,
            verify: verify.to_string(),
        };
        let line = serde_json::to_string(&record)?;
        let path = self.activity_path();
        let mut content = std::fs::read_to_string(&path).unwrap_or_default();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&line);
        content.push('\n');
        atomic_write(&path, &content)
    }

    /// The most recent `n` activity records, oldest first.
    pub fn recent_activity(&self, n: usize) -> Vec<ActivityRecord> {
        let Ok(content) = std::fs::read_to_string(self.activity_path()) else {
            return Vec::new();
        };
        let all: Vec<ActivityRecord> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let skip = all.len().saturating_sub(n);
        all.into_iter().skip(skip).collect()
    }

    // ── improvements.json ────────────────────────────────────────────────

    pub fn improvements_path(&self) -> PathBuf {
        self.dir.join("improvements.json")
    }

    pub fn list_improvements(&self) -> Vec<Improvement> {
        std::fs::read_to_string(self.improvements_path())
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    /// Add an improvement idea to the backlog; returns its id.
    pub fn add_improvement(&self, text: &str) -> anyhow::Result<u64> {
        let text = sanitize_line(text);
        if text.is_empty() {
            anyhow::bail!("cannot add an empty improvement");
        }
        self.ensure_dir()?;
        let mut items = self.list_improvements();
        let id = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        items.push(Improvement {
            id,
            text,
            status: "open".into(),
            created: chrono::Utc::now().to_rfc3339(),
        });
        self.save_improvements(&items)?;
        Ok(id)
    }

    /// Mark an improvement as done. Returns false when the id is unknown.
    pub fn complete_improvement(&self, id: u64) -> anyhow::Result<bool> {
        let mut items = self.list_improvements();
        let Some(item) = items.iter_mut().find(|i| i.id == id) else {
            return Ok(false);
        };
        item.status = "done".into();
        self.save_improvements(&items)?;
        Ok(true)
    }

    fn save_improvements(&self, items: &[Improvement]) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(items)?;
        atomic_write(&self.improvements_path(), &json)
    }

    // ── tasks.json ───────────────────────────────────────────────────────

    pub fn tasks_path(&self) -> PathBuf {
        self.dir.join("tasks.json")
    }

    pub fn list_tasks(&self) -> Vec<ProjectTask> {
        std::fs::read_to_string(self.tasks_path())
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    /// Add a task to the backlog with status "pending"; returns its id.
    pub fn add_task(&self, title: &str) -> anyhow::Result<u64> {
        let title = sanitize_line(title);
        if title.is_empty() {
            anyhow::bail!("cannot add an empty task");
        }
        self.ensure_dir()?;
        let mut tasks = self.list_tasks();
        let id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let now = chrono::Utc::now().to_rfc3339();
        tasks.push(ProjectTask {
            id,
            title,
            status: "pending".into(),
            created: now.clone(),
            updated: now,
        });
        self.save_tasks(&tasks)?;
        Ok(id)
    }

    /// Set a task's status. Returns false when the id is unknown; errors on
    /// a status outside "pending" | "in_progress" | "done" | "failed".
    pub fn set_task_status(&self, id: u64, status: &str) -> anyhow::Result<bool> {
        if !TASK_STATUSES.contains(&status) {
            anyhow::bail!(
                "invalid task status '{status}' (expected one of: {})",
                TASK_STATUSES.join(", ")
            );
        }
        let mut tasks = self.list_tasks();
        let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
            return Ok(false);
        };
        task.status = status.into();
        task.updated = chrono::Utc::now().to_rfc3339();
        self.save_tasks(&tasks)?;
        Ok(true)
    }

    /// The next pending task (lowest id — FIFO), if any.
    /// Wired to upcoming agent auto-pickup; already exercised by tests.
    #[allow(dead_code)]
    pub fn next_pending_task(&self) -> Option<ProjectTask> {
        self.list_tasks()
            .into_iter()
            .filter(|t| t.status == "pending")
            .min_by_key(|t| t.id)
    }

    fn save_tasks(&self, tasks: &[ProjectTask]) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(tasks)?;
        atomic_write(&self.tasks_path(), &json)
    }
}

/// Collapse a possibly multi-line input into one clean line so appended
/// entries can't forge extra bullets/JSONL rows or markdown structure.
fn sanitize_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ProjectStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProjectStore::for_workspace(dir.path());
        (dir, store)
    }

    #[test]
    fn remember_creates_and_appends() {
        let (_d, s) = store();
        s.remember("uses tokio for async").unwrap();
        s.remember("tests live next to code").unwrap();
        let memory = s.load_memory().unwrap();
        assert!(memory.starts_with("# Project memory"));
        assert!(memory.contains("- uses tokio for async"));
        assert!(memory.contains("- tests live next to code"));
    }

    #[test]
    fn remember_flattens_multiline_input() {
        let (_d, s) = store();
        s.remember("line one\nline two\n- fake bullet").unwrap();
        let memory = s.load_memory().unwrap();
        assert!(memory.contains("- line one line two - fake bullet"));
        // Only ONE new bullet line was added.
        assert_eq!(memory.lines().filter(|l| l.starts_with("- ")).count(), 1);
    }

    #[test]
    fn remember_rejects_empty() {
        let (_d, s) = store();
        assert!(s.remember("   ").is_err());
        assert!(s.load_memory().is_none());
    }

    #[test]
    fn decisions_append_and_recent_returns_last_n() {
        let (_d, s) = store();
        for i in 1..=7 {
            s.record_decision(&format!("decision {i}")).unwrap();
        }
        let recent = s.recent_decisions(5);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent.first().unwrap().text, "decision 3");
        assert_eq!(recent.last().unwrap().text, "decision 7");
    }

    #[test]
    fn decisions_skip_corrupt_lines() {
        let (_d, s) = store();
        s.record_decision("good one").unwrap();
        // Corrupt the file with a bad line in the middle.
        let path = s.decisions_path();
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("not json\n");
        std::fs::write(&path, content).unwrap();
        s.record_decision("good two").unwrap();
        let recent = s.recent_decisions(10);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn changes_append_and_recent_returns_last_n() {
        let (_d, s) = store();
        for i in 1..=7 {
            s.record_change("write_file", &format!("src/file{i}.rs"), "wrote 10 bytes")
                .unwrap();
        }
        let recent = s.recent_changes(5);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent.first().unwrap().path, "src/file3.rs");
        assert_eq!(recent.last().unwrap().path, "src/file7.rs");
        assert!(recent.iter().all(|c| c.tool == "write_file"));
        assert!(recent.iter().all(|c| c.summary == "wrote 10 bytes"));
    }

    #[test]
    fn changes_skip_corrupt_lines() {
        let (_d, s) = store();
        s.record_change("write_file", "a.rs", "wrote 1 bytes")
            .unwrap();
        // Corrupt the file with a bad line in the middle.
        let path = s.journal_path();
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("not json\n");
        std::fs::write(&path, content).unwrap();
        s.record_change("edit_file", "b.rs", "replaced snippet")
            .unwrap();
        let recent = s.recent_changes(10);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn changes_reject_empty_and_flatten_multiline_summary() {
        let (_d, s) = store();
        assert!(s.record_change("write_file", "a.rs", "   ").is_err());
        assert!(s.recent_changes(10).is_empty());
        s.record_change("write_file", "a.rs", "line one\nline two")
            .unwrap();
        assert_eq!(s.recent_changes(10)[0].summary, "line one line two");
    }

    #[test]
    fn activity_append_and_recent_returns_last_n() {
        let (_d, s) = store();
        for i in 1..=7 {
            s.record_activity(&format!("request {i}"), i % 2 == 0, "none")
                .unwrap();
        }
        let recent = s.recent_activity(5);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent.first().unwrap().request, "request 3");
        assert_eq!(recent.last().unwrap().request, "request 7");
    }

    #[test]
    fn activity_skip_corrupt_lines() {
        let (_d, s) = store();
        s.record_activity("first", true, "passed").unwrap();
        // Corrupt the file with a bad line in the middle.
        let path = s.activity_path();
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("not json\n");
        std::fs::write(&path, content).unwrap();
        s.record_activity("second", false, "failed").unwrap();
        let recent = s.recent_activity(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].request, "first");
        assert!(recent[0].edited);
        assert_eq!(recent[0].verify, "passed");
        assert_eq!(recent[1].request, "second");
        assert!(!recent[1].edited);
        assert_eq!(recent[1].verify, "failed");
    }

    #[test]
    fn activity_flattens_multiline_request() {
        let (_d, s) = store();
        s.record_activity("line one\nline two", false, "none")
            .unwrap();
        assert_eq!(s.recent_activity(10)[0].request, "line one line two");
    }

    #[test]
    fn improvements_backlog_roundtrip() {
        let (_d, s) = store();
        let id1 = s.add_improvement("add integration tests").unwrap();
        let id2 = s.add_improvement("speed up startup").unwrap();
        assert_ne!(id1, id2);
        assert!(s.complete_improvement(id1).unwrap());
        assert!(!s.complete_improvement(999).unwrap());
        let items = s.list_improvements();
        assert_eq!(items.len(), 2);
        assert_eq!(items.iter().find(|i| i.id == id1).unwrap().status, "done");
        assert_eq!(items.iter().find(|i| i.id == id2).unwrap().status, "open");
    }

    #[test]
    fn tasks_backlog_roundtrip() {
        let (_d, s) = store();
        let id1 = s.add_task("write integration tests").unwrap();
        let id2 = s.add_task("wire up CI").unwrap();
        assert_ne!(id1, id2);
        let tasks = s.list_tasks();
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().all(|t| t.status == "pending"));
        assert!(tasks.iter().all(|t| t.created == t.updated));
        assert!(s.set_task_status(id1, "done").unwrap());
        let tasks = s.list_tasks();
        let done = tasks.iter().find(|t| t.id == id1).unwrap();
        assert_eq!(done.status, "done");
        assert_eq!(
            tasks.iter().find(|t| t.id == id2).unwrap().status,
            "pending"
        );
    }

    #[test]
    fn tasks_next_pending_is_fifo() {
        let (_d, s) = store();
        let first = s.add_task("first").unwrap();
        let second = s.add_task("second").unwrap();
        assert_eq!(s.next_pending_task().unwrap().id, first);
        s.set_task_status(first, "in_progress").unwrap();
        assert_eq!(s.next_pending_task().unwrap().id, second);
        s.set_task_status(second, "done").unwrap();
        assert!(s.next_pending_task().is_none());
    }

    #[test]
    fn tasks_invalid_status_rejected() {
        let (_d, s) = store();
        let id = s.add_task("something").unwrap();
        assert!(s.set_task_status(id, "bogus").is_err());
        // Status untouched after the rejected update.
        assert_eq!(s.list_tasks()[0].status, "pending");
    }

    #[test]
    fn tasks_unknown_id_returns_false() {
        let (_d, s) = store();
        assert!(!s.set_task_status(999, "done").unwrap());
    }

    #[test]
    fn tasks_reject_empty_and_flatten_multiline_title() {
        let (_d, s) = store();
        assert!(s.add_task("   ").is_err());
        s.add_task("line one\nline two\n- fake bullet").unwrap();
        let tasks = s.list_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "line one line two - fake bullet");
    }

    #[test]
    fn brief_save_and_load() {
        let (_d, s) = store();
        assert!(s.load_brief().is_none());
        s.save_brief("A Rust TUI.\n").unwrap();
        assert_eq!(s.load_brief().unwrap(), "A Rust TUI.");
    }

    #[test]
    fn design_creates_and_appends() {
        let (_d, s) = store();
        assert!(s.load_design().is_none());
        s.append_design("use an 8px spacing scale").unwrap();
        s.append_design("prefer system fonts over web fonts")
            .unwrap();
        let design = s.load_design().unwrap();
        assert!(design.starts_with("# Design principles"));
        assert!(design.contains("- use an 8px spacing scale"));
        assert!(design.contains("- prefer system fonts over web fonts"));
    }

    #[test]
    fn design_rejects_empty_and_flattens_multiline() {
        let (_d, s) = store();
        assert!(s.append_design("   ").is_err());
        assert!(s.load_design().is_none());
        s.append_design("line one\nline two\n- fake bullet")
            .unwrap();
        let design = s.load_design().unwrap();
        assert!(design.contains("- line one line two - fake bullet"));
        assert_eq!(design.lines().filter(|l| l.starts_with("- ")).count(), 1);
    }

    #[test]
    fn all_decisions_returns_full_history() {
        let (_d, s) = store();
        for i in 1..=7 {
            s.record_decision(&format!("decision {i}")).unwrap();
        }
        let all = s.all_decisions();
        assert_eq!(all.len(), 7);
        assert_eq!(all.first().unwrap().text, "decision 1");
        assert_eq!(all.last().unwrap().text, "decision 7");
    }

    #[test]
    fn exists_only_after_first_write() {
        let (_d, s) = store();
        assert!(!s.exists());
        s.remember("something").unwrap();
        assert!(s.exists());
    }
}
