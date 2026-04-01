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
    /// Provider used for this conversation (e.g. "claude_code", "ollama").
    #[serde(default)]
    pub provider: String,
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
        if path.extension().is_some_and(|ext| ext == "json")
            && let Ok(contents) = fs::read_to_string(&path)
            && let Ok(record) = serde_json::from_str::<ConversationRecord>(&contents)
        {
            records.push(record);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    /// Create a sample conversation record for testing.
    fn sample_record(id: &str, title: &str) -> ConversationRecord {
        let now = Utc::now();
        ConversationRecord {
            id: id.to_string(),
            title: title.to_string(),
            messages: vec![
                MessageRecord {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    timestamp: now,
                },
                MessageRecord {
                    role: "assistant".to_string(),
                    content: "Hi there!".to_string(),
                    timestamp: now,
                },
            ],
            model: "test-model".to_string(),
            provider: "claude_code".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn conversation_record_serialization_roundtrip() {
        let record = sample_record("test-ser-1", "Serialization Test");
        let json = serde_json::to_string_pretty(&record).expect("serialize");
        let deserialized: ConversationRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, record.id);
        assert_eq!(deserialized.title, record.title);
        assert_eq!(deserialized.messages.len(), 2);
        assert_eq!(deserialized.model, record.model);
    }

    #[test]
    fn message_record_serialization_roundtrip() {
        let msg = MessageRecord {
            role: "user".to_string(),
            content: "Test message content".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let deserialized: MessageRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.role, msg.role);
        assert_eq!(deserialized.content, msg.content);
    }

    #[test]
    fn save_load_roundtrip() {
        let id = format!("test-roundtrip-{}", uuid::Uuid::new_v4());
        let record = sample_record(&id, "Roundtrip Test");

        save_conversation(&record).expect("save");
        let loaded = load_conversation(&id).expect("load");

        assert_eq!(loaded.id, record.id);
        assert_eq!(loaded.title, record.title);
        assert_eq!(loaded.messages.len(), record.messages.len());
        assert_eq!(loaded.messages[0].role, "user");
        assert_eq!(loaded.messages[1].content, "Hi there!");

        // Cleanup
        delete_conversation(&id).expect("cleanup");
    }

    #[test]
    fn list_conversations_includes_saved() {
        let id = format!("test-list-{}", uuid::Uuid::new_v4());
        let record = sample_record(&id, "List Test");
        save_conversation(&record).expect("save");

        let all = list_conversations().expect("list");
        assert!(
            all.iter().any(|r| r.id == id),
            "Saved conversation should appear in list"
        );

        // Cleanup
        delete_conversation(&id).expect("cleanup");
    }

    #[test]
    fn list_conversations_sorted_by_date_descending() {
        let id1 = format!("test-sort-1-{}", uuid::Uuid::new_v4());
        let id2 = format!("test-sort-2-{}", uuid::Uuid::new_v4());

        let mut record1 = sample_record(&id1, "Older");
        record1.updated_at = Utc::now() - chrono::Duration::hours(1);
        save_conversation(&record1).expect("save record1");

        let record2 = sample_record(&id2, "Newer");
        save_conversation(&record2).expect("save record2");

        let all = list_conversations().expect("list");

        // Find the positions of our two records
        let pos1 = all.iter().position(|r| r.id == id1);
        let pos2 = all.iter().position(|r| r.id == id2);

        if let (Some(p1), Some(p2)) = (pos1, pos2) {
            assert!(p2 < p1, "Newer conversation should appear before older one");
        }

        // Cleanup
        delete_conversation(&id1).expect("cleanup");
        delete_conversation(&id2).expect("cleanup");
    }

    #[test]
    fn delete_conversation_removes_file() {
        let id = format!("test-delete-{}", uuid::Uuid::new_v4());
        let record = sample_record(&id, "Delete Test");

        save_conversation(&record).expect("save");
        assert!(conversation_path(&id).exists());

        delete_conversation(&id).expect("delete");
        assert!(!conversation_path(&id).exists());
    }

    #[test]
    fn load_missing_conversation_returns_error() {
        let result = load_conversation("nonexistent-id-that-does-not-exist");
        assert!(
            result.is_err(),
            "Loading a missing conversation should error"
        );
    }

    #[test]
    fn delete_missing_conversation_is_ok() {
        let result = delete_conversation("nonexistent-id-that-does-not-exist");
        assert!(
            result.is_ok(),
            "Deleting a missing conversation should be Ok"
        );
    }

    #[test]
    fn save_and_list_multiple() {
        let mut ids = Vec::new();
        // Save 3 conversations
        for i in 0..3 {
            let id = format!("test_multi_{i}_{}", uuid::Uuid::new_v4());
            let record = ConversationRecord {
                id: id.clone(),
                title: format!("Test Conv {i}"),
                messages: vec![MessageRecord {
                    role: "user".into(),
                    content: format!("Message {i}"),
                    timestamp: chrono::Utc::now(),
                }],
                model: "sonnet".into(),
                provider: "claude_code".into(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            save_conversation(&record).unwrap();
            ids.push(id);
        }

        let list = list_conversations().unwrap();
        assert!(list.len() >= 3);

        // Clean up
        for id in &ids {
            delete_conversation(id).unwrap();
        }
    }

    #[test]
    fn load_nonexistent_fails() {
        let result = load_conversation("totally_nonexistent_id_xyz_123");
        assert!(result.is_err());
    }

    #[test]
    fn conversation_record_with_many_messages() {
        let messages: Vec<MessageRecord> = (0..100)
            .map(|i| MessageRecord {
                role: if i % 2 == 0 { "user" } else { "assistant" }.into(),
                content: format!("Message number {i}"),
                timestamp: chrono::Utc::now(),
            })
            .collect();

        let id = format!("test_large_{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "Large Conversation".into(),
            messages,
            model: "opus".into(),
            provider: "claude_code".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();
        assert_eq!(loaded.messages.len(), 100);

        delete_conversation(&id).unwrap();
    }
}
