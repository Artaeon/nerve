use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use anyhow::{Context, anyhow};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::provider::{AiProvider, ChatMessage, ModelInfo, StreamEvent};

/// GitHub Copilot CLI provider — uses `gh copilot` for AI responses.
#[derive(Debug, Clone)]
pub struct CopilotProvider {
    gh_binary: String,
}

impl CopilotProvider {
    pub fn new() -> Self {
        Self {
            gh_binary: "gh".into(),
        }
    }

    pub fn is_available() -> bool {
        std::process::Command::new("gh")
            .args(["copilot", "--help"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn build_prompt(messages: &[ChatMessage]) -> String {
        // Combine messages into a single prompt
        let mut prompt = String::new();
        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    if !prompt.is_empty() {
                        prompt.push_str("\n\n");
                    }
                    prompt.push_str(&format!("Context: {}", msg.content));
                }
                "user" => {
                    if !prompt.is_empty() {
                        prompt.push_str("\n\n");
                    }
                    prompt.push_str(&msg.content);
                }
                "assistant" => {
                    if !prompt.is_empty() {
                        prompt.push_str("\n\n");
                    }
                    prompt.push_str(&format!("[Previous response: {}]", msg.content));
                }
                _ => {}
            }
        }
        prompt
    }
}

impl AiProvider for CopilotProvider {
    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        _model: &str,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let messages = messages.to_vec();
        Box::pin(async move {
            let prompt = Self::build_prompt(&messages);
            if prompt.is_empty() {
                let _ = tx.send(StreamEvent::Done);
                return Ok(());
            }

            // Use gh copilot suggest for general queries
            let mut child = Command::new(&self.gh_binary)
                .args(["copilot", "suggest", "-t", "shell", &prompt])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context(
                    "Failed to spawn gh copilot — is GitHub CLI installed with Copilot extension?",
                )?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("Failed to capture gh copilot stdout"))?;

            let mut reader = tokio::io::BufReader::with_capacity(32, stdout);
            let mut buf = [0u8; 256];

            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                        if tx.send(StreamEvent::Token(chunk)).is_err() {
                            child.kill().await.ok();
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("Read error: {e}")));
                        child.kill().await.ok();
                        break;
                    }
                }
            }

            let _ = tx.send(StreamEvent::Done);
            child.wait().await.ok();
            Ok(())
        })
    }

    fn chat(
        &self,
        messages: &[ChatMessage],
        _model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        let messages = messages.to_vec();
        Box::pin(async move {
            let prompt = Self::build_prompt(&messages);
            let output = Command::new(&self.gh_binary)
                .args(["copilot", "suggest", "-t", "shell", &prompt])
                .output()
                .await
                .context("Failed to run gh copilot")?;

            let result = String::from_utf8_lossy(&output.stdout).to_string();
            if result.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Copilot returned empty response: {stderr}"));
            }
            Ok(result)
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
        Box::pin(async move {
            Ok(vec![ModelInfo {
                id: "copilot".into(),
                name: "GitHub Copilot".into(),
                provider: "Copilot".into(),
                context_length: Some(8_000),
            }])
        })
    }

    fn name(&self) -> &str {
        "Copilot"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_provider_creates_with_default_binary() {
        let p = CopilotProvider::new();
        assert_eq!(p.gh_binary, "gh");
    }

    #[test]
    fn copilot_provider_name() {
        let p = CopilotProvider::new();
        assert_eq!(p.name(), "Copilot");
    }

    #[test]
    fn build_prompt_empty_messages() {
        let prompt = CopilotProvider::build_prompt(&[]);
        assert!(prompt.is_empty());
    }

    #[test]
    fn build_prompt_single_user_message() {
        let messages = vec![ChatMessage::user("hello")];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert_eq!(prompt, "hello");
    }

    #[test]
    fn build_prompt_system_and_user() {
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("hi"),
        ];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.contains("Context: You are helpful."));
        assert!(prompt.contains("hi"));
    }

    #[test]
    fn build_prompt_includes_assistant_messages() {
        let messages = vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("response"),
            ChatMessage::user("second"),
        ];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.contains("first"));
        assert!(prompt.contains("[Previous response: response]"));
        assert!(prompt.contains("second"));
    }

    #[test]
    fn build_prompt_ignores_unknown_roles() {
        let messages = vec![
            ChatMessage {
                role: "tool".into(),
                content: "data".into(),
            },
            ChatMessage::user("question"),
        ];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(!prompt.contains("data"));
        assert!(prompt.contains("question"));
    }

    #[tokio::test]
    async fn list_models_returns_copilot() {
        let p = CopilotProvider::new();
        let models = p.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "copilot");
        assert_eq!(models[0].provider, "Copilot");
        assert_eq!(models[0].context_length, Some(8_000));
    }

    // ── build_prompt: edge cases ───────────────────────────────────────

    #[test]
    fn build_prompt_user_only() {
        let messages = vec![ChatMessage::user("hello")];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert_eq!(prompt, "hello");
    }

    #[test]
    fn build_prompt_with_system_context() {
        let messages = vec![
            ChatMessage::system("You are helpful"),
            ChatMessage::user("hello"),
        ];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.contains("Context:"));
        assert!(prompt.contains("hello"));
    }

    #[test]
    fn build_prompt_empty() {
        let messages: Vec<ChatMessage> = vec![];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.is_empty());
    }

    #[test]
    fn build_prompt_multiple_system_messages() {
        let messages = vec![
            ChatMessage::system("Rule 1"),
            ChatMessage::system("Rule 2"),
            ChatMessage::user("go"),
        ];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.contains("Context: Rule 1"));
        assert!(prompt.contains("Context: Rule 2"));
        assert!(prompt.contains("go"));
    }

    #[test]
    fn build_prompt_with_shell_metacharacters() {
        let messages = vec![ChatMessage::user("$(rm -rf /) && `whoami` | bash")];
        let prompt = CopilotProvider::build_prompt(&messages);
        // Content should be preserved as-is (passed via Command::args, not shell)
        assert!(prompt.contains("$(rm -rf /)"));
        assert!(prompt.contains("`whoami`"));
    }

    #[test]
    fn build_prompt_very_long() {
        let long_msg = "a".repeat(100_000);
        let messages = vec![ChatMessage::user(&long_msg)];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert_eq!(prompt.len(), 100_000);
    }

    #[test]
    fn build_prompt_preserves_newlines() {
        let messages = vec![ChatMessage::user("line1\nline2\nline3")];
        let prompt = CopilotProvider::build_prompt(&messages);
        assert!(prompt.contains("line1\nline2\nline3"));
    }
}
