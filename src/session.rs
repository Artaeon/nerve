use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Serializable session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub conversations: Vec<SessionConversation>,
    pub active_conversation: usize,
    pub selected_model: String,
    pub selected_provider: String,
    pub agent_mode: bool,
    pub code_mode: bool,
    pub saved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<(String, String)>,
    pub created_at: DateTime<Utc>,
}

/// Directory for session files
fn sessions_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nerve")
        .join("sessions")
}

/// Path to the "last session" file
fn last_session_path() -> PathBuf {
    sessions_dir().join("last_session.json")
}

/// Write JSON to a file atomically (write to .tmp, then rename).
/// Prevents corruption if the app crashes mid-write.
fn atomic_write(path: &std::path::Path, json: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Save the current session
pub fn save_session(session: &Session) -> anyhow::Result<()> {
    let dir = sessions_dir();
    fs::create_dir_all(&dir)?;

    let path = last_session_path();
    let json = serde_json::to_string_pretty(session)?;
    atomic_write(&path, &json)?;

    // Also save a named copy
    let named_path = dir.join(format!(
        "session_{}.json",
        session.id.chars().take(8).collect::<String>()
    ));
    atomic_write(&named_path, &serde_json::to_string(session)?)?;

    Ok(())
}

/// Load the last session
pub fn load_last_session() -> anyhow::Result<Session> {
    let path = last_session_path();
    let content = fs::read_to_string(&path)?;
    let session: Session = serde_json::from_str(&content)?;
    Ok(session)
}

/// List all saved sessions
pub fn list_sessions() -> anyhow::Result<Vec<(String, DateTime<Utc>, usize)>> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("last_session.json") {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(session) = serde_json::from_str::<Session>(&content)
        {
            let conv_count = session.conversations.len();
            sessions.push((session.id, session.saved_at, conv_count));
        }
    }

    sessions.sort_by(|a, b| b.1.cmp(&a.1)); // newest first
    Ok(sessions)
}

/// Delete a session
#[allow(dead_code)]
pub fn delete_session(id: &str) -> anyhow::Result<()> {
    let dir = sessions_dir();
    let prefix = format!("session_{}", id.chars().take(8).collect::<String>());
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Build a Session from current App state
pub fn session_from_app(app: &crate::app::App) -> Session {
    Session {
        id: uuid::Uuid::new_v4().to_string(),
        conversations: app
            .conversations
            .iter()
            .map(|conv| SessionConversation {
                id: conv.id.clone(),
                title: conv.title.clone(),
                messages: conv.messages.clone(),
                created_at: conv.created_at,
            })
            .collect(),
        active_conversation: app.active_conversation,
        selected_model: app.selected_model.clone(),
        selected_provider: app.selected_provider.clone(),
        agent_mode: app.agent_mode,
        code_mode: app.code_mode,
        saved_at: chrono::Utc::now(),
    }
}

/// Restore App state from a Session
pub fn restore_session_to_app(session: &Session, app: &mut crate::app::App) {
    app.conversations.clear();
    for conv in &session.conversations {
        app.conversations.push(crate::app::Conversation {
            id: conv.id.clone(),
            title: conv.title.clone(),
            messages: conv.messages.clone(),
            created_at: conv.created_at,
        });
    }
    if app.conversations.is_empty() {
        app.conversations.push(crate::app::Conversation::new());
    }
    app.active_conversation = session
        .active_conversation
        .min(app.conversations.len().saturating_sub(1));
    app.selected_model = session.selected_model.clone();
    app.selected_provider = session.selected_provider.clone();
    app.agent_mode = session.agent_mode;
    app.code_mode = session.code_mode;
    app.scroll_offset = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests that read/write `last_session.json` must hold this lock to avoid
    /// racing each other (Rust runs tests in parallel by default).
    static FS_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn session_serialization_roundtrip() {
        let session = Session {
            id: "test-id".into(),
            conversations: vec![SessionConversation {
                id: "conv1".into(),
                title: "Test".into(),
                messages: vec![("user".into(), "hello".into())],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let restored: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, "test-id");
        assert_eq!(restored.conversations.len(), 1);
        assert_eq!(restored.selected_model, "sonnet");
    }

    #[test]
    fn save_and_load_session() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![SessionConversation {
                id: "test-conv".into(),
                title: "Save Load Test".into(),
                messages: vec![("user".into(), "save_load_test".into())],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        save_session(&session).unwrap();
        // Verify the named copy exists
        let prefix = format!("session_{}", &session.id[..8]);
        let dir = sessions_dir();
        let found = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&prefix));
        assert!(found, "Named session file should exist");
    }

    #[test]
    fn list_sessions_doesnt_panic() {
        let _ = list_sessions(); // Just verify no panic
    }

    #[test]
    fn session_from_app_captures_all_state() {
        let mut app = crate::app::App::new();
        app.add_user_message("hello".into());
        app.add_assistant_message("hi".into());
        app.selected_model = "opus".into();
        app.selected_provider = "openai".into();
        app.agent_mode = true;
        app.code_mode = true;

        let session = session_from_app(&app);
        assert_eq!(session.selected_model, "opus");
        assert_eq!(session.selected_provider, "openai");
        assert!(session.agent_mode);
        assert!(session.code_mode);
        assert_eq!(session.conversations.len(), 1);
        assert_eq!(session.conversations[0].messages.len(), 2);
    }

    #[test]
    fn restore_session_to_app_works() {
        let session = Session {
            id: "test".into(),
            conversations: vec![
                SessionConversation {
                    id: "c1".into(),
                    title: "Test Conv".into(),
                    messages: vec![("user".into(), "hello".into())],
                    created_at: chrono::Utc::now(),
                },
                SessionConversation {
                    id: "c2".into(),
                    title: "Second Conv".into(),
                    messages: vec![],
                    created_at: chrono::Utc::now(),
                },
            ],
            active_conversation: 1,
            selected_model: "haiku".into(),
            selected_provider: "ollama".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        let mut app = crate::app::App::new();
        restore_session_to_app(&session, &mut app);

        assert_eq!(app.conversations.len(), 2);
        assert_eq!(app.active_conversation, 1);
        assert_eq!(app.selected_model, "haiku");
        assert_eq!(app.selected_provider, "ollama");
        assert_eq!(app.conversations[0].title, "Test Conv");
    }

    #[test]
    fn restore_empty_session_creates_default_conversation() {
        let session = Session {
            id: "test".into(),
            conversations: vec![],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        let mut app = crate::app::App::new();
        restore_session_to_app(&session, &mut app);

        assert_eq!(app.conversations.len(), 1); // Should create a default
        assert_eq!(app.conversations[0].title, "New Conversation");
    }

    #[test]
    fn restore_clamps_active_conversation() {
        let session = Session {
            id: "test".into(),
            conversations: vec![SessionConversation {
                id: "c1".into(),
                title: "Only one".into(),
                messages: vec![],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 99, // Out of bounds
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        let mut app = crate::app::App::new();
        restore_session_to_app(&session, &mut app);
        assert_eq!(app.active_conversation, 0); // Clamped
    }

    #[test]
    fn session_save_load_preserves_messages() {
        let _lock = FS_LOCK.lock().unwrap();
        let mut app = crate::app::App::new();
        app.add_user_message("test message 123".into());
        app.add_assistant_message("response 456".into());

        let session = session_from_app(&app);
        save_session(&session).unwrap();

        let loaded = load_last_session().unwrap();
        assert_eq!(loaded.conversations[0].messages.len(), 2);
        assert_eq!(loaded.conversations[0].messages[0].1, "test message 123");
        assert_eq!(loaded.conversations[0].messages[1].1, "response 456");
    }

    #[test]
    fn session_with_multiple_conversations() {
        let mut app = crate::app::App::new();
        app.add_user_message("conv1".into());
        app.new_conversation();
        app.add_user_message("conv2".into());
        app.new_conversation();
        app.add_user_message("conv3".into());

        let session = session_from_app(&app);
        assert_eq!(session.conversations.len(), 3);
        assert_eq!(session.active_conversation, 2); // Last created

        let mut restored = crate::app::App::new();
        restore_session_to_app(&session, &mut restored);
        assert_eq!(restored.conversations.len(), 3);
        assert_eq!(restored.active_conversation, 2);
    }

    // === File-based tests using tempdir =========================================

    #[test]
    fn save_session_creates_both_files() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![],
            active_conversation: 0,
            selected_model: "test-model".into(),
            selected_provider: "test-provider".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        save_session(&session).unwrap();

        // last_session.json must exist
        assert!(last_session_path().exists());

        // named copy must exist
        let prefix = format!("session_{}", &session.id[..8]);
        let dir = sessions_dir();
        let found = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&prefix));
        assert!(found, "Named session file should exist");

        // Cleanup named copy
        delete_session(&session.id).unwrap();
    }

    #[test]
    fn save_load_roundtrip_all_fields() {
        let _lock = FS_LOCK.lock().unwrap();
        let now = chrono::Utc::now();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![
                SessionConversation {
                    id: "roundtrip-c1".into(),
                    title: "First Conversation".into(),
                    messages: vec![
                        ("user".into(), "hello".into()),
                        ("assistant".into(), "hi there".into()),
                    ],
                    created_at: now,
                },
                SessionConversation {
                    id: "roundtrip-c2".into(),
                    title: "Second Conversation".into(),
                    messages: vec![("user".into(), "question".into())],
                    created_at: now,
                },
            ],
            active_conversation: 1,
            selected_model: "opus-4".into(),
            selected_provider: "openai".into(),
            agent_mode: true,
            code_mode: true,
            saved_at: now,
        };

        save_session(&session).unwrap();
        let loaded = load_last_session().unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.conversations.len(), 2);
        assert_eq!(loaded.active_conversation, 1);
        assert_eq!(loaded.selected_model, "opus-4");
        assert_eq!(loaded.selected_provider, "openai");
        assert!(loaded.agent_mode);
        assert!(loaded.code_mode);

        // Verify conversation content
        assert_eq!(loaded.conversations[0].id, "roundtrip-c1");
        assert_eq!(loaded.conversations[0].title, "First Conversation");
        assert_eq!(loaded.conversations[0].messages.len(), 2);
        assert_eq!(loaded.conversations[1].messages[0].0, "user");
        assert_eq!(loaded.conversations[1].messages[0].1, "question");

        // Cleanup
        delete_session(&session.id).unwrap();
    }

    #[test]
    fn load_corrupted_json_returns_err() {
        let _lock = FS_LOCK.lock().unwrap();
        let dir = sessions_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = last_session_path();

        // Save the original content if it exists
        let original = std::fs::read_to_string(&path).ok();

        std::fs::write(&path, "{{{{not valid json!!!!}}}}").unwrap();
        let result = load_last_session();
        assert!(result.is_err(), "Corrupted JSON should return Err");

        // Restore original (or remove the corrupted file)
        if let Some(content) = original {
            std::fs::write(&path, content).unwrap();
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn load_missing_file_returns_err() {
        let _lock = FS_LOCK.lock().unwrap();
        let path = last_session_path();
        let backup = path.with_extension("json.bak_test");
        let had_file = path.exists();
        if had_file {
            std::fs::rename(&path, &backup).unwrap();
        }

        let result = load_last_session();
        assert!(result.is_err(), "Missing file should return Err");

        // Restore
        if had_file {
            std::fs::rename(&backup, &path).unwrap();
        }
    }

    #[test]
    fn load_partial_json_returns_err() {
        let _lock = FS_LOCK.lock().unwrap();
        let dir = sessions_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = last_session_path();

        let original = std::fs::read_to_string(&path).ok();

        // Write valid JSON but missing required fields
        std::fs::write(&path, r#"{"id": "test"}"#).unwrap();
        let result = load_last_session();
        assert!(
            result.is_err(),
            "Partial JSON missing required fields should return Err"
        );

        if let Some(content) = original {
            std::fs::write(&path, content).unwrap();
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn session_with_unicode_content() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![SessionConversation {
                id: "unicode-conv".into(),
                title: "\u{1F980} Ferris says hi \u{2764}".into(),
                messages: vec![
                    (
                        "user".into(),
                        "\u{00E4}\u{00F6}\u{00FC}\u{00DF} German".into(),
                    ),
                    ("assistant".into(), "\u{4F60}\u{597D} Chinese".into()),
                    ("user".into(), "\u{0410}\u{0411}\u{0412} Russian".into()),
                ],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        save_session(&session).unwrap();
        let loaded = load_last_session().unwrap();

        assert!(loaded.conversations[0].title.contains("\u{1F980}"));
        assert!(
            loaded.conversations[0].messages[0]
                .1
                .contains("\u{00E4}\u{00F6}\u{00FC}")
        );
        assert!(
            loaded.conversations[0].messages[1]
                .1
                .contains("\u{4F60}\u{597D}")
        );

        delete_session(&session.id).unwrap();
    }

    #[test]
    fn restore_session_resets_scroll_offset() {
        let session = Session {
            id: "scroll-test".into(),
            conversations: vec![SessionConversation {
                id: "c1".into(),
                title: "Test".into(),
                messages: vec![],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        let mut app = crate::app::App::new();
        app.scroll_offset = 42;
        restore_session_to_app(&session, &mut app);
        assert_eq!(app.scroll_offset, 0, "Scroll offset should be reset to 0");
    }

    #[test]
    fn restore_session_preserves_agent_and_code_mode() {
        let session = Session {
            id: "mode-test".into(),
            conversations: vec![SessionConversation {
                id: "c1".into(),
                title: "Test".into(),
                messages: vec![],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: true,
            code_mode: true,
            saved_at: chrono::Utc::now(),
        };

        let mut app = crate::app::App::new();
        assert!(!app.agent_mode);
        assert!(!app.code_mode);

        restore_session_to_app(&session, &mut app);
        assert!(app.agent_mode, "Agent mode should be restored");
        assert!(app.code_mode, "Code mode should be restored");
    }

    #[test]
    fn restore_overwrites_existing_conversations() {
        let mut app = crate::app::App::new();
        app.add_user_message("old message".into());
        app.new_conversation();
        app.add_user_message("another old message".into());
        assert_eq!(app.conversations.len(), 2);

        let session = Session {
            id: "overwrite-test".into(),
            conversations: vec![SessionConversation {
                id: "new-c1".into(),
                title: "Brand New".into(),
                messages: vec![("user".into(), "fresh start".into())],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        restore_session_to_app(&session, &mut app);
        assert_eq!(app.conversations.len(), 1);
        assert_eq!(app.conversations[0].id, "new-c1");
        assert_eq!(app.conversations[0].messages[0].1, "fresh start");
    }

    #[test]
    fn session_from_app_generates_unique_ids() {
        let app = crate::app::App::new();
        let s1 = session_from_app(&app);
        let s2 = session_from_app(&app);
        assert_ne!(s1.id, s2.id, "Each session should get a unique ID");
    }

    #[test]
    fn session_from_app_captures_conversation_ids() {
        let mut app = crate::app::App::new();
        let original_id = app.conversations[0].id.clone();
        app.add_user_message("test".into());

        let session = session_from_app(&app);
        assert_eq!(
            session.conversations[0].id, original_id,
            "Session should preserve conversation IDs"
        );
    }

    #[test]
    fn delete_session_removes_named_copy() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        save_session(&session).unwrap();

        // Verify named copy exists
        let dir = sessions_dir();
        let prefix = format!("session_{}", &session.id[..8]);
        let before = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&prefix));
        assert!(before, "Named copy should exist before delete");

        delete_session(&session.id).unwrap();

        let after = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&prefix));
        assert!(!after, "Named copy should be gone after delete");
    }

    #[test]
    fn session_conversation_preserves_created_at() {
        let fixed_time = chrono::DateTime::parse_from_rfc3339("2025-06-15T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        let session = Session {
            id: "time-test".into(),
            conversations: vec![SessionConversation {
                id: "c1".into(),
                title: "Time Test".into(),
                messages: vec![],
                created_at: fixed_time,
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: fixed_time,
        };

        let json = serde_json::to_string(&session).unwrap();
        let loaded: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.conversations[0].created_at, fixed_time);
        assert_eq!(loaded.saved_at, fixed_time);
    }

    #[test]
    fn list_sessions_skips_last_session_file() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };
        save_session(&session).unwrap();

        let sessions = list_sessions().unwrap();
        // list_sessions explicitly skips last_session.json, so there should
        // be no entry whose id came only from the last_session.json file.
        // We can't directly test the skip, but we can verify the named copy
        // shows up exactly once.
        let count = sessions
            .iter()
            .filter(|(id, _, _)| *id == session.id)
            .count();
        assert_eq!(
            count, 1,
            "Session should appear exactly once (from named copy)"
        );

        delete_session(&session.id).unwrap();
    }

    #[test]
    fn session_with_empty_messages() {
        let _lock = FS_LOCK.lock().unwrap();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            conversations: vec![SessionConversation {
                id: "empty-msgs".into(),
                title: "Empty Messages".into(),
                messages: vec![],
                created_at: chrono::Utc::now(),
            }],
            active_conversation: 0,
            selected_model: "sonnet".into(),
            selected_provider: "claude_code".into(),
            agent_mode: false,
            code_mode: false,
            saved_at: chrono::Utc::now(),
        };

        save_session(&session).unwrap();
        let loaded = load_last_session().unwrap();
        assert_eq!(loaded.conversations[0].messages.len(), 0);

        delete_session(&session.id).unwrap();
    }
}
