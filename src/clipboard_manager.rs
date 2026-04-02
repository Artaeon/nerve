use anyhow::Context;
use chrono::{DateTime, Utc};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub content: String,
    pub source: ClipboardSource,
    pub timestamp: DateTime<Utc>,
    pub preview: String, // First 100 chars, single line
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardSource {
    AiResponse,
    UserCopy,
    PromptTemplate,
    ManualCopy, // Ctrl+Y
}

impl ClipboardSource {
    pub fn badge(&self) -> &'static str {
        match self {
            ClipboardSource::AiResponse => "[AI]",
            ClipboardSource::UserCopy => "[Copy]",
            ClipboardSource::PromptTemplate => "[Prompt]",
            ClipboardSource::ManualCopy => "[Manual]",
        }
    }
}

pub struct ClipboardManager {
    entries: Vec<ClipboardEntry>,
    max_entries: usize,
}

impl ClipboardManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add a new entry to the clipboard history.
    pub fn add(&mut self, content: String, source: ClipboardSource) {
        let preview = make_preview(&content);
        let entry = ClipboardEntry {
            content,
            source,
            timestamp: Utc::now(),
            preview,
        };
        self.entries.insert(0, entry);
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Return all entries as a slice (newest first).
    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    /// Get a single entry by index.
    #[allow(dead_code)]
    pub fn get(&self, index: usize) -> Option<&ClipboardEntry> {
        self.entries.get(index)
    }

    /// Fuzzy-search through entries, returning `(index, entry)` pairs sorted by score.
    pub fn search(&self, query: &str) -> Vec<(usize, &ClipboardEntry)> {
        if query.is_empty() {
            return self.entries.iter().enumerate().collect();
        }
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, usize, &ClipboardEntry)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                matcher
                    .fuzzy_match(&entry.content, query)
                    .map(|score| (score, i, entry))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, i, e)| (i, e)).collect()
    }

    /// Copy the entry at `index` to the system clipboard.
    pub fn copy_to_system(&self, index: usize) -> anyhow::Result<()> {
        let entry = self
            .entries
            .get(index)
            .context("clipboard entry index out of bounds")?;
        crate::clipboard::copy_to_clipboard(&entry.content)
    }

    /// Remove an entry by index.
    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    /// Clear all entries.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Persist clipboard history to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = data_file_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create clipboard data directory")?;
        }
        let json =
            serde_json::to_string_pretty(&self.entries).context("failed to serialize clipboard")?;
        std::fs::write(&path, json).context("failed to write clipboard file")?;
        Ok(())
    }

    /// Load clipboard history from disk.
    pub fn load() -> anyhow::Result<Self> {
        let path = data_file_path()?;
        let json = std::fs::read_to_string(&path).context("failed to read clipboard file")?;
        let entries: Vec<ClipboardEntry> =
            serde_json::from_str(&json).context("failed to parse clipboard file")?;
        Ok(Self {
            entries,
            max_entries: 100,
        })
    }
}

/// Build a single-line preview from the first 100 characters.
fn make_preview(content: &str) -> String {
    let cleaned: String = content
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .take(100)
        .collect();
    cleaned
}

/// Return the path to `~/.local/share/nerve/clipboard.json`.
fn data_file_path() -> anyhow::Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine XDG data directory")?;
    Ok(data_dir.join("nerve").join("clipboard.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clipboard_manager_is_empty() {
        let cm = ClipboardManager::new(50);
        assert!(cm.entries().is_empty());
        assert_eq!(cm.max_entries, 50);
    }

    #[test]
    fn add_creates_entry_with_correct_fields() {
        let mut cm = ClipboardManager::new(10);
        cm.add("Hello, world!".to_string(), ClipboardSource::AiResponse);

        assert_eq!(cm.entries().len(), 1);
        let entry = &cm.entries()[0];
        assert_eq!(entry.content, "Hello, world!");
        assert_eq!(entry.preview, "Hello, world!");
        assert!(matches!(entry.source, ClipboardSource::AiResponse));
    }

    #[test]
    fn add_generates_preview_with_truncation() {
        let mut cm = ClipboardManager::new(10);
        let long_content = "a".repeat(200);
        cm.add(long_content.clone(), ClipboardSource::UserCopy);

        let entry = &cm.entries()[0];
        assert_eq!(entry.preview.len(), 100);
        assert_eq!(entry.preview, "a".repeat(100));
    }

    #[test]
    fn add_generates_preview_replacing_newlines() {
        let mut cm = ClipboardManager::new(10);
        cm.add("line1\nline2\rline3".to_string(), ClipboardSource::UserCopy);

        let entry = &cm.entries()[0];
        assert_eq!(entry.preview, "line1 line2 line3");
        assert!(!entry.preview.contains('\n'));
        assert!(!entry.preview.contains('\r'));
    }

    #[test]
    fn add_respects_max_entries_limit() {
        let mut cm = ClipboardManager::new(3);
        cm.add("first".to_string(), ClipboardSource::AiResponse);
        cm.add("second".to_string(), ClipboardSource::AiResponse);
        cm.add("third".to_string(), ClipboardSource::AiResponse);
        cm.add("fourth".to_string(), ClipboardSource::AiResponse);

        assert_eq!(cm.entries().len(), 3);
        // The oldest ("first") should have been removed
        assert_eq!(cm.entries()[0].content, "fourth");
        assert_eq!(cm.entries()[1].content, "third");
        assert_eq!(cm.entries()[2].content, "second");
    }

    #[test]
    fn entries_returns_newest_first() {
        let mut cm = ClipboardManager::new(10);
        cm.add("older".to_string(), ClipboardSource::AiResponse);
        cm.add("newer".to_string(), ClipboardSource::AiResponse);

        assert_eq!(cm.entries()[0].content, "newer");
        assert_eq!(cm.entries()[1].content, "older");
    }

    #[test]
    fn search_finds_matching_entries() {
        let mut cm = ClipboardManager::new(10);
        cm.add(
            "rust programming language".to_string(),
            ClipboardSource::AiResponse,
        );
        cm.add("python scripting".to_string(), ClipboardSource::UserCopy);
        cm.add(
            "rust cargo build".to_string(),
            ClipboardSource::PromptTemplate,
        );

        let results = cm.search("rust");
        assert!(
            results.len() >= 2,
            "Should find at least 2 entries containing 'rust', got {}",
            results.len()
        );
        for (_, entry) in &results {
            assert!(
                entry.content.contains("rust") || entry.content.contains("Rust"),
                "Search result should match query"
            );
        }
    }

    #[test]
    fn search_returns_empty_for_no_matches() {
        let mut cm = ClipboardManager::new(10);
        cm.add("hello world".to_string(), ClipboardSource::AiResponse);

        let results = cm.search("zzzznotfound");
        assert!(
            results.is_empty(),
            "Should find no matches for nonsense query"
        );
    }

    #[test]
    fn search_empty_query_returns_all() {
        let mut cm = ClipboardManager::new(10);
        cm.add("one".to_string(), ClipboardSource::AiResponse);
        cm.add("two".to_string(), ClipboardSource::UserCopy);

        let results = cm.search("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn remove_removes_correct_entry() {
        let mut cm = ClipboardManager::new(10);
        cm.add("first".to_string(), ClipboardSource::AiResponse);
        cm.add("second".to_string(), ClipboardSource::UserCopy);
        cm.add("third".to_string(), ClipboardSource::PromptTemplate);

        // Remove index 1 ("second", since newest-first: third=0, second=1, first=2)
        cm.remove(1);

        assert_eq!(cm.entries().len(), 2);
        assert_eq!(cm.entries()[0].content, "third");
        assert_eq!(cm.entries()[1].content, "first");
    }

    #[test]
    fn remove_out_of_bounds_is_noop() {
        let mut cm = ClipboardManager::new(10);
        cm.add("only".to_string(), ClipboardSource::AiResponse);
        cm.remove(99);
        assert_eq!(cm.entries().len(), 1);
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cm = ClipboardManager::new(10);
        cm.add("one".to_string(), ClipboardSource::AiResponse);
        cm.add("two".to_string(), ClipboardSource::UserCopy);
        cm.clear();
        assert!(cm.entries().is_empty());
    }

    #[test]
    fn make_preview_truncates_at_100_chars() {
        let long = "x".repeat(250);
        let preview = make_preview(&long);
        assert_eq!(preview.len(), 100);
    }

    #[test]
    fn make_preview_replaces_newlines_with_spaces() {
        let content = "hello\nworld\r!";
        let preview = make_preview(content);
        assert_eq!(preview, "hello world !");
    }

    #[test]
    fn clipboard_source_badge_display() {
        assert_eq!(ClipboardSource::AiResponse.badge(), "[AI]");
        assert_eq!(ClipboardSource::UserCopy.badge(), "[Copy]");
        assert_eq!(ClipboardSource::PromptTemplate.badge(), "[Prompt]");
        assert_eq!(ClipboardSource::ManualCopy.badge(), "[Manual]");
    }

    #[test]
    fn get_returns_entry_at_index() {
        let mut cm = ClipboardManager::new(10);
        cm.add("one".to_string(), ClipboardSource::AiResponse);
        cm.add("two".to_string(), ClipboardSource::UserCopy);

        assert_eq!(cm.get(0).unwrap().content, "two");
        assert_eq!(cm.get(1).unwrap().content, "one");
        assert!(cm.get(2).is_none());
    }

    #[test]
    fn clipboard_entry_serialization_roundtrip() {
        let entry = ClipboardEntry {
            content: "test content".to_string(),
            source: ClipboardSource::AiResponse,
            timestamp: Utc::now(),
            preview: "test content".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let deserialized: ClipboardEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.content, entry.content);
        assert_eq!(deserialized.preview, entry.preview);
    }

    #[test]
    fn add_generates_correct_preview() {
        let mut cm = ClipboardManager::new(10);
        cm.add(
            "This is a test message\nwith multiple lines\nand content".into(),
            ClipboardSource::AiResponse,
        );
        let entry = &cm.entries()[0];
        // Preview should replace newlines with spaces
        assert!(!entry.preview.contains('\n'));
        assert!(entry.preview.contains("This is a test message"));
    }

    #[test]
    fn search_with_fuzzy_match() {
        let mut cm = ClipboardManager::new(10);
        cm.add(
            "Rust programming language".into(),
            ClipboardSource::AiResponse,
        );
        cm.add(
            "Python scripting language".into(),
            ClipboardSource::AiResponse,
        );
        cm.add(
            "JavaScript web language".into(),
            ClipboardSource::AiResponse,
        );

        let results = cm.search("rust");
        assert!(!results.is_empty());
        // First result should be about Rust
        assert!(results[0].1.content.contains("Rust"));
    }

    #[test]
    fn clipboard_save_load_roundtrip() {
        let mut cm = ClipboardManager::new(10);
        cm.add("Test content for save".into(), ClipboardSource::UserCopy);
        cm.save().unwrap();

        let loaded = ClipboardManager::load().unwrap();
        assert!(!loaded.entries().is_empty());
    }

    #[test]
    fn remove_by_index_shifts_entries() {
        let mut cm = ClipboardManager::new(10);
        cm.add("first".into(), ClipboardSource::AiResponse);
        cm.add("second".into(), ClipboardSource::AiResponse);
        cm.add("third".into(), ClipboardSource::AiResponse);

        cm.remove(1); // Remove "second" (newest-first, so index 1 = "second")
        assert_eq!(cm.entries().len(), 2);
    }
}
