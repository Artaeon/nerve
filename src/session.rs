use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Utc};

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

/// Save the current session
pub fn save_session(session: &Session) -> anyhow::Result<()> {
    let dir = sessions_dir();
    fs::create_dir_all(&dir)?;

    let path = last_session_path();
    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, json)?;

    // Also save a named copy
    let named_path = dir.join(format!("session_{}.json", session.id.chars().take(8).collect::<String>()));
    fs::write(named_path, serde_json::to_string(session)?)?;

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
        if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
        if path.file_name().and_then(|n| n.to_str()) == Some("last_session.json") { continue; }

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
        conversations: app.conversations.iter().map(|conv| {
            SessionConversation {
                id: conv.id.clone(),
                title: conv.title.clone(),
                messages: conv.messages.clone(),
                created_at: conv.created_at,
            }
        }).collect(),
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
    app.active_conversation = session.active_conversation.min(app.conversations.len().saturating_sub(1));
    app.selected_model = session.selected_model.clone();
    app.selected_provider = session.selected_provider.clone();
    app.agent_mode = session.agent_mode;
    app.code_mode = session.code_mode;
    app.scroll_offset = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // This test writes to last_session.json — use a unique session
        // and verify by the named copy to avoid races with other tests.
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
}
