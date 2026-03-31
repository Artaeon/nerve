use anyhow::Context;
use chrono::{DateTime, Utc};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
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
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Persist clipboard history to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = data_file_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("failed to create clipboard data directory")?;
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
    let data_dir =
        dirs::data_dir().context("could not determine XDG data directory")?;
    Ok(data_dir.join("nerve").join("clipboard.json"))
}
