use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A persisted conversation (one JSON file per conversation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRecord {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Human-readable title (often auto-generated from the first message).
    pub title: String,
    /// The ordered list of messages in this conversation.
    pub messages: Vec<MessageRecord>,
    /// Model identifier used for this conversation.
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single message within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    /// `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Return the history storage directory (`~/.local/share/nerve/history/`).
pub fn history_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nerve")
        .join("history")
}

/// Build the file path for a given conversation ID.
fn conversation_path(id: &str) -> PathBuf {
    history_dir().join(format!("{id}.json"))
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Save (create or update) a conversation to disk.
pub fn save_conversation(record: &ConversationRecord) -> anyhow::Result<()> {
    let dir = history_dir();
    fs::create_dir_all(&dir)?;

    let path = conversation_path(&record.id);
    let json = serde_json::to_string_pretty(record)?;
    fs::write(path, json)?;
    Ok(())
}

/// Load a single conversation by its ID.
pub fn load_conversation(id: &str) -> anyhow::Result<ConversationRecord> {
    let path = conversation_path(id);
    let contents = fs::read_to_string(&path)?;
    let record: ConversationRecord = serde_json::from_str(&contents)?;
    Ok(record)
}

/// List all conversations, sorted by `updated_at` descending (most recent
/// first).
///
/// Files that fail to parse are silently skipped so that one corrupt file
/// does not prevent the rest from loading.
pub fn list_conversations() -> anyhow::Result<Vec<ConversationRecord>> {
    let dir = history_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&dir)?;
    let mut records = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(record) = serde_json::from_str::<ConversationRecord>(&contents) {
                    records.push(record);
                }
            }
        }
    }

    // Most recently updated first.
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(records)
}

/// Delete a conversation by its ID.
///
/// Returns `Ok(())` if the file was removed or did not exist.
pub fn delete_conversation(id: &str) -> anyhow::Result<()> {
    let path = conversation_path(id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
