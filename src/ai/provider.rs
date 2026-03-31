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
