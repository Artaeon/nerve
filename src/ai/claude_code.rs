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
    /// Whether to allow Claude Code to use tools (file access, bash, etc.).
    enable_tools: bool,
    /// Optional session ID for session continuity.
    session_id: Option<String>,
    /// Working directory for file access when tools are enabled.
    working_dir: Option<String>,
}

impl ClaudeCodeProvider {
    /// Create a new provider that looks for `claude` in `$PATH`.
    /// Tools are disabled (chat-only mode).
    pub fn new() -> Self {
        Self {
            claude_binary: "claude".into(),
            default_model: "sonnet".into(),
            enable_tools: false,
            session_id: None,
            working_dir: None,
        }
    }

    /// Create a new provider with tools enabled (file access, bash, etc.).
    /// Uses `--dangerously-skip-permissions` for non-interactive tool use.
    pub fn with_tools() -> Self {
        Self {
            claude_binary: "claude".into(),
            default_model: "sonnet".into(),
            enable_tools: true,
            session_id: None,
            working_dir: None,
        }
    }

    /// Set the working directory for file access (builder pattern).
    pub fn with_working_dir(mut self, dir: String) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Create a new provider with a specific default model.
    pub fn with_model(model: String) -> Self {
        Self {
            claude_binary: "claude".into(),
            default_model: model,
            enable_tools: false,
            session_id: None,
            working_dir: None,
        }
    }

    /// Returns whether tools (file access, bash) are enabled.
    pub fn tools_enabled(&self) -> bool {
        self.enable_tools
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

            let mut args: Vec<String> = vec![
                "-p".into(),
                prompt.clone(),
                "--output-format".into(),
                "text".into(),
                "--model".into(),
                model.into(),
            ];

            if self.enable_tools {
                args.push("--dangerously-skip-permissions".into());
            } else {
                args.push("--allowedTools".into());
                args.push("".into());
            }

            if let Some(ref session) = self.session_id {
                args.push("--resume".into());
                args.push(session.clone());
            } else {
                args.push("--no-session-persistence".into());
            }

            if let Some(ref dir) = self.working_dir {
                args.push("--add-dir".into());
                args.push(dir.clone());
            }

            if let Some(ref sys) = system_prompt {
                args.push("--system-prompt".into());
                args.push(sys.clone());
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

            let mut args: Vec<String> = vec![
                "-p".into(),
                prompt.clone(),
                "--output-format".into(),
                "json".into(),
                "--model".into(),
                model.into(),
            ];

            if self.enable_tools {
                args.push("--dangerously-skip-permissions".into());
            } else {
                args.push("--allowedTools".into());
                args.push("".into());
            }

            if let Some(ref session) = self.session_id {
                args.push("--resume".into());
                args.push(session.clone());
            } else {
                args.push("--no-session-persistence".into());
            }

            if let Some(ref dir) = self.working_dir {
                args.push("--add-dir".into());
                args.push(dir.clone());
            }

            if let Some(ref sys) = system_prompt {
                args.push("--system-prompt".into());
                args.push(sys.clone());
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

            // Capture session ID for continuity (logged for now; the trait
            // returns only a String so we cannot propagate it directly).
            if let Some(sid) = json["session_id"].as_str() {
                tracing::info!("Claude Code session: {sid}");
            }

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
                    id: "opus".into(),
                    name: "Claude Opus 4.6".into(),
                    provider: "Claude Code".into(),
                    context_length: Some(1_000_000),
                },
                ModelInfo {
                    id: "sonnet".into(),
                    name: "Claude Sonnet 4.6".into(),
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
