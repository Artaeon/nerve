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
/// Validates the ID to prevent path traversal attacks.
fn conversation_path(id: &str) -> PathBuf {
    // Sanitize: only allow alphanumeric, hyphens, and underscores.
    let safe_id: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect();
    let safe_id = if safe_id.is_empty() {
        "invalid"
    } else {
        &safe_id
    };
    history_dir().join(format!("{safe_id}.json"))
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
#[allow(dead_code)]
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

    // === New comprehensive tests ================================================

    #[test]
    fn save_load_roundtrip_preserves_all_fields() {
        let now = Utc::now();
        let id = format!("test-full-roundtrip-{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "Full Roundtrip".into(),
            messages: vec![
                MessageRecord {
                    role: "user".into(),
                    content: "What is Rust?".into(),
                    timestamp: now,
                },
                MessageRecord {
                    role: "assistant".into(),
                    content: "Rust is a systems programming language.".into(),
                    timestamp: now,
                },
                MessageRecord {
                    role: "user".into(),
                    content: "Tell me more.".into(),
                    timestamp: now,
                },
            ],
            model: "opus-4".into(),
            provider: "openai".into(),
            created_at: now,
            updated_at: now,
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();

        assert_eq!(loaded.id, record.id);
        assert_eq!(loaded.title, record.title);
        assert_eq!(loaded.model, "opus-4");
        assert_eq!(loaded.provider, "openai");
        assert_eq!(loaded.messages.len(), 3);
        assert_eq!(loaded.messages[0].role, "user");
        assert_eq!(loaded.messages[0].content, "What is Rust?");
        assert_eq!(loaded.messages[2].content, "Tell me more.");

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn corrupted_json_skipped_in_list_conversations() {
        let dir = history_dir();
        std::fs::create_dir_all(&dir).unwrap();

        // Write a corrupted JSON file
        let corrupt_id = format!("test-corrupt-{}", uuid::Uuid::new_v4());
        let corrupt_path = dir.join(format!("{corrupt_id}.json"));
        std::fs::write(&corrupt_path, "NOT VALID JSON {{{{").unwrap();

        // Write a valid record
        let valid_id = format!("test-valid-{}", uuid::Uuid::new_v4());
        let valid_record = sample_record(&valid_id, "Valid Record");
        save_conversation(&valid_record).unwrap();

        let list = list_conversations().unwrap();

        // The valid one should be present
        assert!(
            list.iter().any(|r| r.id == valid_id),
            "Valid record must appear in list"
        );
        // The corrupted one should NOT be present
        assert!(
            !list.iter().any(|r| r.id == corrupt_id),
            "Corrupted record must be skipped"
        );

        // Cleanup
        let _ = std::fs::remove_file(&corrupt_path);
        delete_conversation(&valid_id).unwrap();
    }

    #[test]
    fn empty_history_dir_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let empty_dir = tmp.path().join("empty_history");
        // Don't even create the dir -- list_conversations checks if dir exists
        // We'll test by temporarily pointing at a dir that does exist but is empty
        std::fs::create_dir_all(&empty_dir).unwrap();
        assert!(
            std::fs::read_dir(&empty_dir).unwrap().next().is_none(),
            "Dir should be empty"
        );
        // We can't redirect history_dir(), but we can verify the behavior
        // by checking that the function handles a nonexistent dir path.
        // Since history_dir() may or may not exist, the best we can test is
        // that list_conversations does not panic and returns Ok.
        let result = list_conversations();
        assert!(result.is_ok());
    }

    #[test]
    fn history_record_with_unicode_title_and_content() {
        let id = format!("test-unicode-{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "\u{1F980} Ferris \u{2764}\u{FE0F} \u{4F60}\u{597D}".into(),
            messages: vec![
                MessageRecord {
                    role: "user".into(),
                    content: "\u{00E4}\u{00F6}\u{00FC}\u{00DF} German umlauts".into(),
                    timestamp: Utc::now(),
                },
                MessageRecord {
                    role: "assistant".into(),
                    content: "\u{0410}\u{0411}\u{0412}\u{0413} Cyrillic".into(),
                    timestamp: Utc::now(),
                },
                MessageRecord {
                    role: "user".into(),
                    content: "\u{1F600}\u{1F4BB}\u{1F680} emoji everywhere".into(),
                    timestamp: Utc::now(),
                },
            ],
            model: "sonnet".into(),
            provider: "claude_code".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();

        assert!(loaded.title.contains("\u{1F980}"));
        assert!(
            loaded.messages[0]
                .content
                .contains("\u{00E4}\u{00F6}\u{00FC}")
        );
        assert!(loaded.messages[1].content.contains("\u{0410}\u{0411}"));
        assert!(loaded.messages[2].content.contains("\u{1F680}"));

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn delete_conversation_removes_file_from_disk() {
        let id = format!("test-delete-verify-{}", uuid::Uuid::new_v4());
        let record = sample_record(&id, "Will be deleted");

        save_conversation(&record).unwrap();
        let path = conversation_path(&id);
        assert!(path.exists(), "File must exist after save");

        delete_conversation(&id).unwrap();
        assert!(!path.exists(), "File must not exist after delete");

        // Verify it's gone from list too
        let list = list_conversations().unwrap();
        assert!(
            !list.iter().any(|r| r.id == id),
            "Deleted record must not appear in list"
        );
    }

    #[test]
    fn save_overwrites_existing_conversation() {
        let id = format!("test-overwrite-{}", uuid::Uuid::new_v4());
        let mut record = sample_record(&id, "Original Title");
        save_conversation(&record).unwrap();

        // Update and save again
        record.title = "Updated Title".into();
        record.messages.push(MessageRecord {
            role: "user".into(),
            content: "New message".into(),
            timestamp: Utc::now(),
        });
        save_conversation(&record).unwrap();

        let loaded = load_conversation(&id).unwrap();
        assert_eq!(loaded.title, "Updated Title");
        assert_eq!(loaded.messages.len(), 3); // original 2 + 1 new

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn conversation_record_with_empty_messages() {
        let id = format!("test-empty-msgs-{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "Empty".into(),
            messages: vec![],
            model: "sonnet".into(),
            provider: "claude_code".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();
        assert_eq!(loaded.messages.len(), 0);

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn conversation_record_provider_defaults_to_empty() {
        // The `provider` field has #[serde(default)], so missing it in JSON
        // should default to ""
        let json = r#"{
            "id": "test-no-provider",
            "title": "No Provider",
            "messages": [],
            "model": "sonnet",
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z"
        }"#;
        let record: ConversationRecord = serde_json::from_str(json).unwrap();
        assert_eq!(
            record.provider, "",
            "Missing provider should default to empty string"
        );
    }

    #[test]
    fn list_conversations_ignores_non_json_files() {
        let dir = history_dir();
        std::fs::create_dir_all(&dir).unwrap();

        // Write a non-JSON file
        let non_json = dir.join("test_not_json.txt");
        std::fs::write(&non_json, "This is not JSON").unwrap();

        // Should not cause an error or appear in results
        let result = list_conversations();
        assert!(result.is_ok());

        // Cleanup
        let _ = std::fs::remove_file(&non_json);
    }

    #[test]
    fn conversation_path_uses_id_directly() {
        let path = conversation_path("my-conversation-id");
        assert!(
            path.to_string_lossy().ends_with("my-conversation-id.json"),
            "Path should end with id.json"
        );
    }

    #[test]
    fn save_multiple_then_list_returns_all() {
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = format!("test-multi-list-{i}-{}", uuid::Uuid::new_v4());
            let record = sample_record(&id, &format!("Multi {i}"));
            save_conversation(&record).unwrap();
            ids.push(id);
        }

        let list = list_conversations().unwrap();
        for id in &ids {
            assert!(
                list.iter().any(|r| r.id == *id),
                "Record {id} must appear in list"
            );
        }

        // Cleanup
        for id in &ids {
            delete_conversation(id).unwrap();
        }
    }

    #[test]
    fn message_record_roles() {
        let id = format!("test-roles-{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "Role Test".into(),
            messages: vec![
                MessageRecord {
                    role: "system".into(),
                    content: "You are a helpful assistant.".into(),
                    timestamp: Utc::now(),
                },
                MessageRecord {
                    role: "user".into(),
                    content: "Hello".into(),
                    timestamp: Utc::now(),
                },
                MessageRecord {
                    role: "assistant".into(),
                    content: "Hi!".into(),
                    timestamp: Utc::now(),
                },
            ],
            model: "sonnet".into(),
            provider: "claude_code".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();
        assert_eq!(loaded.messages[0].role, "system");
        assert_eq!(loaded.messages[1].role, "user");
        assert_eq!(loaded.messages[2].role, "assistant");

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn conversation_record_preserves_timestamps() {
        let fixed_time = chrono::DateTime::parse_from_rfc3339("2025-03-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let id = format!("test-timestamps-{}", uuid::Uuid::new_v4());
        let record = ConversationRecord {
            id: id.clone(),
            title: "Timestamp Test".into(),
            messages: vec![MessageRecord {
                role: "user".into(),
                content: "test".into(),
                timestamp: fixed_time,
            }],
            model: "sonnet".into(),
            provider: "claude_code".into(),
            created_at: fixed_time,
            updated_at: fixed_time,
        };

        save_conversation(&record).unwrap();
        let loaded = load_conversation(&id).unwrap();
        assert_eq!(loaded.created_at, fixed_time);
        assert_eq!(loaded.updated_at, fixed_time);
        assert_eq!(loaded.messages[0].timestamp, fixed_time);

        delete_conversation(&id).unwrap();
    }

    #[test]
    fn delete_idempotent() {
        let id = format!("test-idempotent-{}", uuid::Uuid::new_v4());
        // Delete something that was never saved
        let r1 = delete_conversation(&id);
        assert!(r1.is_ok());
        // Delete again
        let r2 = delete_conversation(&id);
        assert!(r2.is_ok());
    }

    // ── Conversation ID sanitization ──────────────────────────────────

    #[test]
    fn conversation_path_normal_id() {
        let path = conversation_path("abc-123-def");
        assert!(path.to_string_lossy().ends_with("abc-123-def.json"));
    }

    #[test]
    fn conversation_path_strips_slashes() {
        let path = conversation_path("../../../etc/passwd");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(!name.contains(".."), "should strip path separators: {name}");
        assert!(!name.contains('/'), "should strip slashes: {name}");
    }

    #[test]
    fn conversation_path_strips_dots() {
        let path = conversation_path("../../evil");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(!name.contains(".."));
    }

    #[test]
    fn conversation_path_empty_id_handled() {
        let path = conversation_path("");
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(name, "invalid.json");
    }

    #[test]
    fn conversation_path_all_special_chars() {
        let path = conversation_path("../../");
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(name, "invalid.json");
    }

    #[test]
    fn conversation_path_uuid_preserved() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let path = conversation_path(uuid);
        assert!(path.to_string_lossy().contains(uuid));
    }
}
