use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

/// Events emitted during a streaming chat completion.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A new token fragment from the model.
    Token(String),
    /// The stream has finished successfully.
    Done,
    /// An error occurred during streaming.
    Error(String),
}

/// Metadata about an available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_length: Option<usize>,
}

/// Trait implemented by all AI provider backends.
///
/// Returns `Pin<Box<dyn Future>>` so the trait is dyn-compatible and can be
/// used behind `Arc<dyn AiProvider>`.
pub trait AiProvider: Send + Sync {
    /// Send a chat completion request and stream tokens back via the channel.
    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;

    /// Non-streaming chat completion. Returns the full assistant response.
    fn chat(
        &self,
        messages: &[ChatMessage],
        model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>>;

    /// List available models from the provider.
    fn list_models(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>>;

    /// The human-readable provider name (e.g. "OpenAI", "Ollama").
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatMessage::system() ───────────────────────────────────────────

    #[test]
    fn system_message_from_str() {
        let msg = ChatMessage::system("You are helpful.");
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "You are helpful.");
    }

    #[test]
    fn system_message_from_string() {
        let content = String::from("System instructions");
        let msg = ChatMessage::system(content);
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "System instructions");
    }

    #[test]
    fn system_message_empty() {
        let msg = ChatMessage::system("");
        assert_eq!(msg.role, "system");
        assert!(msg.content.is_empty());
    }

    // ── ChatMessage::user() ─────────────────────────────────────────────

    #[test]
    fn user_message_from_str() {
        let msg = ChatMessage::user("Hello!");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello!");
    }

    #[test]
    fn user_message_from_string() {
        let content = String::from("User question");
        let msg = ChatMessage::user(content);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "User question");
    }

    #[test]
    fn user_message_empty() {
        let msg = ChatMessage::user("");
        assert_eq!(msg.role, "user");
        assert!(msg.content.is_empty());
    }

    // ── ChatMessage::assistant() ────────────────────────────────────────

    #[test]
    fn assistant_message_from_str() {
        let msg = ChatMessage::assistant("Sure, here's the answer.");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Sure, here's the answer.");
    }

    #[test]
    fn assistant_message_from_string() {
        let content = String::from("Response text");
        let msg = ChatMessage::assistant(content);
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Response text");
    }

    #[test]
    fn assistant_message_empty() {
        let msg = ChatMessage::assistant("");
        assert_eq!(msg.role, "assistant");
        assert!(msg.content.is_empty());
    }

    // ── Into<String> / generic ──────────────────────────────────────────

    #[test]
    fn message_from_cow_str() {
        // std::borrow::Cow<str> implements Into<String>
        let cow: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed("cow content");
        let msg = ChatMessage::user(cow);
        assert_eq!(msg.content, "cow content");
    }

    #[test]
    fn message_with_unicode_content() {
        let msg = ChatMessage::user("Bonjour le monde! \u{1F30D}");
        assert_eq!(msg.content, "Bonjour le monde! \u{1F30D}");
        assert_eq!(msg.role, "user");
    }

    #[test]
    fn message_with_multiline_content() {
        let content = "line1\nline2\nline3";
        let msg = ChatMessage::system(content);
        assert_eq!(msg.content, "line1\nline2\nline3");
    }

    // ── Clone / Debug / Serialize ───────────────────────────────────────

    #[test]
    fn chat_message_is_cloneable() {
        let msg = ChatMessage::user("test");
        let cloned = msg.clone();
        assert_eq!(cloned.role, msg.role);
        assert_eq!(cloned.content, msg.content);
    }

    #[test]
    fn chat_message_serializes_to_json() {
        let msg = ChatMessage::user("hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"hello\""));
    }

    #[test]
    fn chat_message_deserializes_from_json() {
        let json = r#"{"role":"assistant","content":"hi"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "hi");
    }

    // ── StreamEvent ─────────────────────────────────────────────────────

    #[test]
    fn stream_event_token_is_cloneable() {
        let event = StreamEvent::Token("chunk".into());
        let cloned = event.clone();
        if let StreamEvent::Token(t) = cloned {
            assert_eq!(t, "chunk");
        } else {
            panic!("expected Token variant");
        }
    }

    #[test]
    fn stream_event_done_variant() {
        let event = StreamEvent::Done;
        assert!(matches!(event, StreamEvent::Done));
    }

    #[test]
    fn stream_event_error_variant() {
        let event = StreamEvent::Error("oops".into());
        if let StreamEvent::Error(e) = event {
            assert_eq!(e, "oops");
        } else {
            panic!("expected Error variant");
        }
    }

    // ── ModelInfo ───────────────────────────────────────────────────────

    #[test]
    fn model_info_serialization_roundtrip() {
        let info = ModelInfo {
            id: "test-model".into(),
            name: "Test Model".into(),
            provider: "test-provider".into(),
            context_length: Some(8192),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-model");
        assert_eq!(deserialized.name, "Test Model");
        assert_eq!(deserialized.provider, "test-provider");
        assert_eq!(deserialized.context_length, Some(8192));
    }

    #[test]
    fn model_info_context_length_none() {
        let info = ModelInfo {
            id: "x".into(),
            name: "X".into(),
            provider: "p".into(),
            context_length: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.context_length, None);
    }
}
