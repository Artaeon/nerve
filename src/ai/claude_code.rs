use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use anyhow::{Context, anyhow};
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn tools_enabled(&self) -> bool {
        self.enable_tools
    }

    /// Build a single prompt string and an optional system prompt from a slice
    /// of [`ChatMessage`]s.
    ///
    /// System messages are concatenated into a dedicated `--system-prompt`
    /// argument.  User/assistant messages are flattened into a single text
    /// block that is passed via `-p`.
    pub(crate) fn build_prompt(messages: &[ChatMessage]) -> (Option<String>, String) {
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

    /// Check if the `claude` CLI is available in `$PATH`.
    ///
    /// Runs `claude --version` and returns `true` only if it exits
    /// successfully.  This is a blocking check intended for startup
    /// validation.
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        std::process::Command::new(&self.claude_binary)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Resolve the effective model identifier — use the caller's choice if
    /// non-empty, otherwise fall back to our default.
    pub(crate) fn resolve_model<'a>(&'a self, model: &'a str) -> &'a str {
        if model.is_empty() {
            &self.default_model
        } else {
            model
        }
    }

    /// Build the common CLI argument list shared by streaming and
    /// non-streaming calls. Only `output_format` differs ("text" vs "json").
    fn build_args(
        &self,
        prompt: &str,
        model: &str,
        output_format: &str,
        system_prompt: &Option<String>,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-p".into(),
            prompt.into(),
            "--output-format".into(),
            output_format.into(),
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

        if let Some(sys) = system_prompt {
            args.push("--system-prompt".into());
            args.push(sys.clone());
        }

        args
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

            let args = self.build_args(&prompt, model, "text", &system_prompt);

            let mut child = Command::new(&self.claude_binary)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .context("failed to spawn claude CLI — is it installed and in PATH?")?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("failed to capture claude stdout"))?;

            let stderr_handle = child.stderr.take();

            // Use a small-capacity BufReader so we get chunks as soon as
            // the CLI flushes them, giving a streaming feel.
            let mut reader = tokio::io::BufReader::with_capacity(8192, stdout);
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
            let status = child
                .wait()
                .await
                .context("failed to wait on claude process")?;
            if !status.success() {
                let stderr_msg = if let Some(mut stderr) = stderr_handle {
                    let mut buf = String::new();
                    stderr.read_to_string(&mut buf).await.ok();
                    buf
                } else {
                    String::new()
                };
                if stderr_msg.is_empty() {
                    tracing::warn!("claude exited with status {status}");
                } else {
                    tracing::warn!("claude exited with status {status}: {stderr_msg}");
                }
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

            let args = self.build_args(&prompt, model, "json", &system_prompt);

            let output = Command::new(&self.claude_binary)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
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

            let result = json["result"].as_str().unwrap_or("").to_string();

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_prompt: user-only messages ────────────────────────────────

    #[test]
    fn build_prompt_single_user_message() {
        let msgs = vec![ChatMessage::user("Hello")];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(system.is_none());
        assert_eq!(conversation, "Hello");
    }

    #[test]
    fn build_prompt_multiple_user_messages() {
        let msgs = vec![ChatMessage::user("First"), ChatMessage::user("Second")];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(system.is_none());
        assert_eq!(conversation, "First\n\nSecond");
    }

    // ── build_prompt: system + user ─────────────────────────────────────

    #[test]
    fn build_prompt_system_and_user() {
        let msgs = vec![
            ChatMessage::system("Be helpful."),
            ChatMessage::user("What is Rust?"),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(system, Some("Be helpful.".into()));
        assert_eq!(conversation, "What is Rust?");
    }

    // ── build_prompt: system + user + assistant ─────────────────────────

    #[test]
    fn build_prompt_system_user_assistant() {
        let msgs = vec![
            ChatMessage::system("You are a tutor."),
            ChatMessage::user("Explain closures."),
            ChatMessage::assistant("A closure captures variables from its environment."),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(system, Some("You are a tutor.".into()));
        assert!(conversation.contains("Explain closures."));
        assert!(conversation.contains(
            "[Previous assistant response: A closure captures variables from its environment.]"
        ));
    }

    #[test]
    fn build_prompt_user_assistant_user() {
        let msgs = vec![
            ChatMessage::user("Q1"),
            ChatMessage::assistant("A1"),
            ChatMessage::user("Q2"),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(system.is_none());
        assert_eq!(
            conversation,
            "Q1\n\n[Previous assistant response: A1]\n\nQ2"
        );
    }

    // ── build_prompt: multiple system messages ──────────────────────────

    #[test]
    fn build_prompt_multiple_system_messages_concatenated() {
        let msgs = vec![
            ChatMessage::system("Rule 1."),
            ChatMessage::system("Rule 2."),
            ChatMessage::user("Go."),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(system, Some("Rule 1.\n\nRule 2.".into()));
        assert_eq!(conversation, "Go.");
    }

    #[test]
    fn build_prompt_system_messages_scattered() {
        // System messages scattered between other messages should all be collected.
        let msgs = vec![
            ChatMessage::system("Sys1"),
            ChatMessage::user("U1"),
            ChatMessage::system("Sys2"),
            ChatMessage::user("U2"),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(system, Some("Sys1\n\nSys2".into()));
        assert_eq!(conversation, "U1\n\nU2");
    }

    // ── build_prompt: empty messages ────────────────────────────────────

    #[test]
    fn build_prompt_empty_messages() {
        let msgs: Vec<ChatMessage> = vec![];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(system.is_none());
        assert!(conversation.is_empty());
    }

    #[test]
    fn build_prompt_only_system_messages() {
        let msgs = vec![ChatMessage::system("Be helpful.")];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(system, Some("Be helpful.".into()));
        assert!(conversation.is_empty());
    }

    // ── build_prompt: unknown role ──────────────────────────────────────

    #[test]
    fn build_prompt_unknown_role_ignored() {
        let msgs = vec![
            ChatMessage {
                role: "function".into(),
                content: "ignored".into(),
            },
            ChatMessage::user("Hello"),
        ];
        let (system, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(system.is_none());
        assert_eq!(conversation, "Hello");
    }

    // ── build_prompt: assistant wrapping format ─────────────────────────

    #[test]
    fn build_prompt_assistant_message_format() {
        let msgs = vec![ChatMessage::assistant("The answer is 42.")];
        let (_, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert_eq!(
            conversation,
            "[Previous assistant response: The answer is 42.]"
        );
    }

    // ── build_prompt: no double newlines at start ───────────────────────

    #[test]
    fn build_prompt_no_leading_newlines() {
        let msgs = vec![ChatMessage::user("Only one")];
        let (_, conversation) = ClaudeCodeProvider::build_prompt(&msgs);
        assert!(!conversation.starts_with('\n'));
    }

    // ── resolve_model ───────────────────────────────────────────────────

    #[test]
    fn resolve_model_uses_provided_model() {
        let provider = ClaudeCodeProvider::new();
        assert_eq!(provider.resolve_model("opus"), "opus");
    }

    #[test]
    fn resolve_model_empty_uses_default() {
        let provider = ClaudeCodeProvider::new();
        assert_eq!(provider.resolve_model(""), "sonnet");
    }

    #[test]
    fn resolve_model_custom_default() {
        let provider = ClaudeCodeProvider::with_model("haiku".into());
        assert_eq!(provider.resolve_model(""), "haiku");
    }

    #[test]
    fn resolve_model_non_empty_overrides_custom_default() {
        let provider = ClaudeCodeProvider::with_model("haiku".into());
        assert_eq!(provider.resolve_model("opus"), "opus");
    }

    // ── Constructor variants ────────────────────────────────────────────

    #[test]
    fn new_provider_defaults() {
        let p = ClaudeCodeProvider::new();
        assert_eq!(p.claude_binary, "claude");
        assert_eq!(p.default_model, "sonnet");
        assert!(!p.enable_tools);
        assert!(p.session_id.is_none());
        assert!(p.working_dir.is_none());
    }

    #[test]
    fn with_tools_enables_tools() {
        let p = ClaudeCodeProvider::with_tools();
        assert!(p.enable_tools);
        assert!(p.tools_enabled());
    }

    #[test]
    fn with_working_dir_sets_dir() {
        let p = ClaudeCodeProvider::new().with_working_dir("/tmp/test".into());
        assert_eq!(p.working_dir, Some("/tmp/test".into()));
    }

    #[test]
    fn with_model_sets_model() {
        let p = ClaudeCodeProvider::with_model("opus".into());
        assert_eq!(p.default_model, "opus");
        assert!(!p.enable_tools);
    }

    #[test]
    fn name_returns_claude_code() {
        let p = ClaudeCodeProvider::new();
        assert_eq!(p.name(), "Claude Code");
    }

    // ── is_available ───────────────────────────────────────────────────

    #[test]
    fn is_available_returns_false_for_nonexistent_binary() {
        let p = ClaudeCodeProvider {
            claude_binary: "definitely_not_a_real_binary_xyz_123".into(),
            ..ClaudeCodeProvider::new()
        };
        assert!(!p.is_available());
    }

    // ── Security: special characters in prompts ────────────────────────

    #[test]
    fn build_prompt_with_shell_metacharacters() {
        let messages = vec![ChatMessage::user(
            "What does `echo 'hello' | grep -c 'h'` do?",
        )];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("echo"));
        assert!(prompt.contains("grep"));
        // Prompt should preserve the content as-is -- no escaping needed
        // since we pass via Command::args() not sh -c
        assert!(prompt.contains("'hello'"));
        assert!(prompt.contains("|"));
    }

    #[test]
    fn build_prompt_with_dollar_expansion() {
        let messages = vec![ChatMessage::user("What is $(whoami)?")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("$(whoami)"));
    }

    #[test]
    fn build_prompt_with_backticks() {
        let messages = vec![ChatMessage::user("Run `rm -rf /` please")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("`rm -rf /`"));
    }

    #[test]
    fn build_prompt_with_semicolons_and_ampersands() {
        let messages = vec![ChatMessage::user("cmd1; cmd2 && cmd3 || cmd4 & cmd5")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("; cmd2 && cmd3 || cmd4 & cmd5"));
    }

    // ── Security: very long prompts ────────────────────────────────────

    #[test]
    fn build_prompt_with_very_long_message() {
        let long_msg = "x".repeat(100_000);
        let messages = vec![ChatMessage::user(&long_msg)];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert_eq!(prompt.len(), 100_000);
    }

    // ── Prompt content preservation ────────────────────────────────────

    #[test]
    fn build_prompt_preserves_newlines() {
        let messages = vec![ChatMessage::user("line1\nline2\nline3")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("line1\nline2\nline3"));
    }

    #[test]
    fn build_prompt_preserves_tabs_and_whitespace() {
        let messages = vec![ChatMessage::user("fn main() {\n\tprintln!(\"hi\");\n}")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("\t"));
    }

    #[test]
    fn build_prompt_preserves_unicode() {
        let messages = vec![ChatMessage::user("\u{1F600} \u{4e16}\u{754c}")];
        let (_, prompt) = ClaudeCodeProvider::build_prompt(&messages);
        assert!(prompt.contains("\u{1F600}"));
        assert!(prompt.contains("\u{4e16}"));
    }

    // ── Constructor builder pattern ────────────────────────────────────

    #[test]
    fn with_tools_enables_flag() {
        let p = ClaudeCodeProvider::with_tools();
        assert!(p.tools_enabled());
    }

    #[test]
    fn default_has_no_tools() {
        let p = ClaudeCodeProvider::new();
        assert!(!p.tools_enabled());
    }

    #[test]
    fn with_working_dir_chainable() {
        let p = ClaudeCodeProvider::with_tools().with_working_dir("/tmp/test".into());
        assert!(p.tools_enabled());
        assert_eq!(p.working_dir, Some("/tmp/test".into()));
    }
}
