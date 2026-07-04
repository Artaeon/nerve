use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use anyhow::{Context, anyhow};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
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

        // stream-json only emits intermediate events when --verbose is set,
        // and only emits per-token deltas when --include-partial-messages is
        // set. Without these, `claude -p` buffers the whole response and
        // dumps it at end-of-stream, making the TUI look frozen.
        if output_format == "stream-json" {
            args.push("--verbose".into());
            args.push("--include-partial-messages".into());
        }

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

            let args = self.build_args(&prompt, model, "stream-json", &system_prompt);

            // stdin is set to null: the TUI runs the terminal in raw mode with
            // the alternate screen buffer, and a child inheriting that tty as
            // stdin can consume the user's keystrokes or block.
            //
            // kill_on_drop(true) ensures the subprocess is SIGKILLed the
            // moment this future is dropped — e.g. when the user cancels
            // via Esc or Ctrl+N — rather than continuing to run and bill
            // against the subscription.
            let mut child = Command::new(&self.claude_binary)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context("failed to spawn claude CLI — is it installed and in PATH?")?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("failed to capture claude stdout"))?;
            let stderr = child.stderr.take();

            // Drain stderr concurrently. If we leave it undrained while
            // streaming stdout, the kernel pipe buffer (~64KB) can fill up
            // and the child will block writing to stderr, deadlocking the
            // whole call.
            let stderr_task = tokio::spawn(async move {
                let mut buf = String::new();
                if let Some(mut stderr) = stderr {
                    stderr.read_to_string(&mut buf).await.ok();
                }
                buf
            });

            let reader = tokio::io::BufReader::with_capacity(8192, stdout);
            let mut lines = reader.lines();
            let mut emitted_any_token = false;
            let mut fallback_result: Option<String> = None;
            let mut error_from_stream: Option<String> = None;

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
                            // Non-JSON line (e.g. stray log output). Ignore it
                            // rather than polluting the chat view.
                            continue;
                        };

                        let kind = event["type"].as_str().unwrap_or("");
                        match kind {
                            // Per-token deltas (emitted with --include-partial-messages).
                            "stream_event" => {
                                let ev = &event["event"];
                                if ev["type"].as_str() == Some("content_block_delta")
                                    && let Some(delta) = ev.get("delta")
                                    && delta["type"].as_str() == Some("text_delta")
                                    && let Some(text) = delta["text"].as_str()
                                    && !text.is_empty()
                                {
                                    if tx.send(StreamEvent::Token(text.to_string())).is_err() {
                                        child.kill().await.ok();
                                        break;
                                    }
                                    emitted_any_token = true;
                                }
                            }
                            // Terminal event. Capture session id for logging
                            // and the fallback result for the "no deltas"
                            // case (short responses sometimes skip deltas).
                            "result" => {
                                if let Some(sid) = event["session_id"].as_str() {
                                    tracing::info!("Claude Code session: {sid}");
                                }
                                if event["is_error"].as_bool().unwrap_or(false) {
                                    let msg = event["result"]
                                        .as_str()
                                        .or_else(|| event["api_error_status"].as_str())
                                        .unwrap_or("claude returned is_error=true")
                                        .to_string();
                                    error_from_stream = Some(msg);
                                } else if let Some(result) = event["result"].as_str() {
                                    fallback_result = Some(result.to_string());
                                }
                            }
                            // All other event kinds (system/init, rate_limit_event,
                            // assistant aggregated messages, etc.) are ignored — the
                            // deltas and result event carry what we need.
                            _ => {}
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        let _ = tx.send(StreamEvent::Error(format!("read error: {e}")));
                        child.kill().await.ok();
                        break;
                    }
                }
            }

            // Fallback: if no per-token deltas arrived but the terminal
            // `result` event carries a complete response, emit it in one
            // chunk so the user still sees something.
            if !emitted_any_token
                && error_from_stream.is_none()
                && let Some(text) = fallback_result
                && !text.is_empty()
            {
                let _ = tx.send(StreamEvent::Token(text));
            }

            if let Some(err) = error_from_stream {
                let _ = tx.send(StreamEvent::Error(err));
            }

            let status = child
                .wait()
                .await
                .context("failed to wait on claude process")?;
            let stderr_msg = stderr_task.await.unwrap_or_default();

            if !status.success() {
                // Surface the CLI error to the TUI so users see *something*
                // rather than silence.
                let detail = if stderr_msg.trim().is_empty() {
                    format!("claude exited with status {status}")
                } else {
                    format!("claude exited with status {status}: {}", stderr_msg.trim())
                };
                tracing::warn!("{detail}");
                let _ = tx.send(StreamEvent::Error(detail));
            }

            let _ = tx.send(StreamEvent::Done);
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
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .context("failed to run claude CLI — is it installed and in PATH?")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(anyhow!(friendly_cli_error(&stderr, &stdout)));
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

/// Turn a failed `claude` CLI invocation into a human-readable error.
///
/// The CLI often prints a full JSON result object on failure; showing that
/// blob verbatim is unreadable. Extract the `result` message when present and
/// add actionable guidance for the common "not logged in" case.
fn friendly_cli_error(stderr: &str, stdout: &str) -> String {
    // Prefer the JSON `result` field from stdout (the CLI's own message).
    let json_msg = serde_json::from_str::<serde_json::Value>(stdout.trim())
        .ok()
        .and_then(|v| v["result"].as_str().map(str::to_string))
        .filter(|s| !s.is_empty());

    let msg = json_msg.unwrap_or_else(|| {
        let s = stderr.trim();
        if s.is_empty() {
            "claude CLI failed with no error output".to_string()
        } else {
            s.to_string()
        }
    });

    if msg.to_lowercase().contains("not logged in") || msg.contains("/login") {
        format!(
            "Claude: {msg}\n\nRun `claude` in a terminal and use /login to sign in, then retry."
        )
    } else {
        format!("Claude: {msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── friendly_cli_error ───────────────────────────────────────────────

    #[test]
    fn friendly_error_extracts_json_result() {
        let stdout = r#"{"type":"result","is_error":true,"result":"Not logged in · Please run /login","session_id":"x"}"#;
        let msg = friendly_cli_error("", stdout);
        assert!(msg.contains("Not logged in"));
        assert!(
            msg.contains("/login to sign in"),
            "adds actionable guidance"
        );
        assert!(
            !msg.contains("session_id"),
            "raw JSON must not leak through"
        );
    }

    #[test]
    fn friendly_error_falls_back_to_stderr() {
        let msg = friendly_cli_error("command not found: something", "not json");
        assert!(msg.contains("command not found"));
    }

    #[test]
    fn friendly_error_handles_empty_output() {
        let msg = friendly_cli_error("", "");
        assert!(msg.contains("no error output"));
    }

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
        assert!(prompt.contains('|'));
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
        assert!(prompt.contains('\t'));
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

    // ── build_args tests ────────────────────────────────────────────

    #[test]
    fn build_args_basic_structure() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hello", "sonnet", "text", &None);
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "hello");
        assert_eq!(args[2], "--output-format");
        assert_eq!(args[3], "text");
        assert_eq!(args[4], "--model");
        assert_eq!(args[5], "sonnet");
    }

    #[test]
    fn build_args_json_format() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("prompt", "opus", "json", &None);
        assert_eq!(args[3], "json");
    }

    #[test]
    fn build_args_tools_disabled_adds_empty_allowed_tools() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(args.contains(&"--allowedTools".to_string()));
        assert!(args.contains(&String::new()));
        assert!(!args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn build_args_tools_enabled_adds_dangerous_flag() {
        let p = ClaudeCodeProvider::with_tools();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!args.contains(&"--allowedTools".to_string()));
    }

    #[test]
    fn build_args_no_session_adds_no_persistence() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--resume".to_string()));
    }

    #[test]
    fn build_args_with_session_adds_resume() {
        let mut p = ClaudeCodeProvider::new();
        p.session_id = Some("sess-123".into());
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess-123".to_string()));
        assert!(!args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn build_args_with_working_dir() {
        let p = ClaudeCodeProvider::new().with_working_dir("/project".into());
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(args.contains(&"--add-dir".to_string()));
        assert!(args.contains(&"/project".to_string()));
    }

    #[test]
    fn build_args_without_working_dir() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(!args.contains(&"--add-dir".to_string()));
    }

    #[test]
    fn build_args_with_system_prompt() {
        let p = ClaudeCodeProvider::new();
        let sys = Some("You are helpful.".to_string());
        let args = p.build_args("hi", "sonnet", "text", &sys);
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are helpful.".to_string()));
    }

    #[test]
    fn build_args_without_system_prompt() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(!args.contains(&"--system-prompt".to_string()));
    }

    #[test]
    fn build_args_stream_json_adds_verbose_and_partial() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "stream-json", &None);
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--include-partial-messages".to_string()));
        assert_eq!(args[3], "stream-json");
    }

    #[test]
    fn build_args_text_does_not_add_stream_flags() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "text", &None);
        assert!(!args.contains(&"--verbose".to_string()));
        assert!(!args.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn build_args_json_does_not_add_stream_flags() {
        let p = ClaudeCodeProvider::new();
        let args = p.build_args("hi", "sonnet", "json", &None);
        assert!(!args.contains(&"--verbose".to_string()));
        assert!(!args.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn build_args_special_characters_in_prompt() {
        let p = ClaudeCodeProvider::new();
        let prompt = "hello 'world' \"test\" $VAR `cmd`";
        let args = p.build_args(prompt, "sonnet", "text", &None);
        assert_eq!(args[1], prompt);
    }

    // ── Live integration: requires `claude` CLI + valid auth ────────────
    //
    // Run with: `cargo test --bin nerve claude_code_stream_live -- --ignored --nocapture`
    // Uses a real API call (short prompt, ~10 tokens) to verify that
    // chat_stream() actually delivers tokens via the channel when using
    // stream-json. This catches regressions the unit tests can't (e.g. if
    // claude CLI changes its output format).
    #[tokio::test]
    #[ignore = "live call to claude CLI — requires auth + network"]
    async fn chat_stream_live_emits_tokens() {
        if !ClaudeCodeProvider::new().is_available() {
            eprintln!("claude CLI not on PATH — skipping");
            return;
        }

        let provider = ClaudeCodeProvider::new();
        let messages = vec![ChatMessage::user(
            "Respond with exactly the five characters: hello",
        )];
        let (tx, mut rx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            provider.chat_stream(&messages, "sonnet", tx).await.unwrap();
        });

        let mut collected = String::new();
        let mut saw_done = false;
        let mut saw_error: Option<String> = None;
        while let Some(ev) = rx.recv().await {
            match ev {
                StreamEvent::Token(t) => collected.push_str(&t),
                StreamEvent::Done => {
                    saw_done = true;
                    break;
                }
                StreamEvent::Error(e) => saw_error = Some(e),
                _ => {}
            }
        }
        handle.await.unwrap();

        assert!(saw_done, "stream never sent Done");
        assert!(saw_error.is_none(), "stream errored: {saw_error:?}");
        assert!(
            collected.to_lowercase().contains("hello"),
            "expected 'hello' in response, got: {collected:?}"
        );
    }
}
