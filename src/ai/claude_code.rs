use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use anyhow::{anyhow, Context};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::provider::{AiProvider, ChatMessage, ModelInfo, StreamEvent};

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

/// An AI provider that delegates to the `claude` CLI binary (Claude Code).
///
/// This provider requires no API key — it uses the user's existing Claude Code
/// subscription.  The `claude` binary is invoked in non-interactive (`-p`) mode
/// for each request.
#[derive(Debug, Clone)]
pub struct ClaudeCodeProvider {
    claude_binary: String,
    default_model: String,
}

impl ClaudeCodeProvider {
    /// Create a new provider that looks for `claude` in `$PATH`.
    pub fn new() -> Self {
        Self {
            claude_binary: "claude".into(),
            default_model: "sonnet".into(),
        }
    }

    /// Create a new provider with a specific default model.
    pub fn with_model(model: String) -> Self {
        Self {
            claude_binary: "claude".into(),
            default_model: model,
        }
    }

    /// Build a single prompt string and an optional system prompt from a slice
    /// of [`ChatMessage`]s.
    ///
    /// System messages are concatenated into a dedicated `--system-prompt`
    /// argument.  User/assistant messages are flattened into a single text
    /// block that is passed via `-p`.
    fn build_prompt(messages: &[ChatMessage]) -> (Option<String>, String) {
        let mut system_parts: Vec<&str> = Vec::new();
        let mut conversation = String::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_parts.push(&msg.content);
                }
                "user" => {
                    if !conversation.is_empty() {
                        conversation.push_str("\n\n");
                    }
                    conversation.push_str(&msg.content);
                }
                "assistant" => {
                    if !conversation.is_empty() {
                        conversation.push_str("\n\n");
                    }
                    conversation
                        .push_str(&format!("[Previous assistant response: {}]", msg.content));
                }
                _ => {}
            }
        }

        let system_prompt = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n\n"))
        };

        (system_prompt, conversation)
    }

    /// Resolve the effective model identifier — use the caller's choice if
    /// non-empty, otherwise fall back to our default.
    fn resolve_model<'a>(&'a self, model: &'a str) -> &'a str {
        if model.is_empty() {
            &self.default_model
        } else {
            model
        }
    }
}

impl AiProvider for ClaudeCodeProvider {
    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let messages = messages.to_vec();
        let model = model.to_string();
        Box::pin(async move {
            let (system_prompt, prompt) = Self::build_prompt(&messages);
            let model = self.resolve_model(&model);

            if prompt.is_empty() {
                let _ = tx.send(StreamEvent::Done);
                return Ok(());
            }

            let mut args: Vec<&str> = vec![
                "-p",
                &prompt,
                "--output-format",
                "text",
                "--model",
                model,
                "--allowedTools",
                "",
                "--no-session-persistence",
            ];

            // Borrow system_prompt content for the args slice.
            if let Some(ref sys) = system_prompt {
                args.push("--system-prompt");
                args.push(sys.as_str());
            }

            let mut child = Command::new(&self.claude_binary)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .context("failed to spawn claude CLI — is it installed and in PATH?")?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("failed to capture claude stdout"))?;

            // Use a small-capacity BufReader so we get chunks as soon as
            // the CLI flushes them, giving a streaming feel.
            let mut reader = tokio::io::BufReader::with_capacity(32, stdout);
            let mut buf = [0u8; 256];

            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                        if tx.send(StreamEvent::Token(chunk)).is_err() {
                            // Receiver dropped — kill the child and bail.
                            child.kill().await.ok();
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("read error: {e}")));
                        child.kill().await.ok();
                        break;
                    }
                }
            }

            let _ = tx.send(StreamEvent::Done);

            // Reap the child so we don't leave zombies.
            let status = child.wait().await.context("failed to wait on claude process")?;
            if !status.success() {
                tracing::warn!("claude exited with status {status}");
            }

            Ok(())
        })
    }

    fn chat(
        &self,
        messages: &[ChatMessage],
        model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        let messages = messages.to_vec();
        let model = model.to_string();
        Box::pin(async move {
            let (system_prompt, prompt) = Self::build_prompt(&messages);
            let model = self.resolve_model(&model);

            if prompt.is_empty() {
                return Ok(String::new());
            }

            let mut args: Vec<&str> = vec![
                "-p",
                &prompt,
                "--output-format",
                "json",
                "--model",
                model,
                "--allowedTools",
                "",
                "--no-session-persistence",
            ];

            if let Some(ref sys) = system_prompt {
                args.push("--system-prompt");
                args.push(sys.as_str());
            }

            let output = Command::new(&self.claude_binary)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .await
                .context("failed to run claude CLI — is it installed and in PATH?")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(anyhow!(
                    "claude exited with {}: {}{}",
                    output.status,
                    stderr,
                    stdout
                ));
            }

            // The JSON output has a "result" field containing the response text.
            let json: serde_json::Value = serde_json::from_slice(&output.stdout)
                .context("failed to parse claude JSON output")?;

            let result = json["result"]
                .as_str()
                .unwrap_or("")
                .to_string();

            Ok(result)
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
        Box::pin(async move {
            let models = vec![
                ModelInfo {
                    id: "sonnet".into(),
                    name: "Claude Sonnet 4".into(),
                    provider: "Claude Code".into(),
                    context_length: Some(200_000),
                },
                ModelInfo {
                    id: "opus".into(),
                    name: "Claude Opus 4".into(),
                    provider: "Claude Code".into(),
                    context_length: Some(200_000),
                },
                ModelInfo {
                    id: "haiku".into(),
                    name: "Claude Haiku 4.5".into(),
                    provider: "Claude Code".into(),
                    context_length: Some(200_000),
                },
            ];
            Ok(models)
        })
    }

    fn name(&self) -> &str {
        "Claude Code"
    }
}
