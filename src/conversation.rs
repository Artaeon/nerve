use std::sync::Arc;

use tokio::sync::mpsc;

use crate::ai::provider::{AiProvider, ChatMessage, StreamEvent};
use crate::app::{self, App, InputMode};
use crate::clipboard_manager::ClipboardSource;
use crate::config::Config;
use crate::{clipboard, commands, files, knowledge};

// ─── Smart title generation ────────────────────────────────────────────────

/// Generate a concise, meaningful title from the user's first message.
pub(crate) fn generate_title(first_user_message: &str) -> String {
    let msg = first_user_message.trim();

    if msg.is_empty() {
        return "New Conversation".into();
    }

    // If it starts with a slash command, use the command as context
    if msg.starts_with('/') {
        let parts: Vec<&str> = msg.splitn(3, ' ').collect();
        return match parts.first().copied() {
            Some("/file") => format!("File: {}", parts.get(1).unwrap_or(&"unknown")),
            Some("/test") => "Test Run".into(),
            Some("/build") => "Build".into(),
            Some("/diff") => "Code Review (diff)".into(),
            Some("/url") => format!(
                "Web: {}",
                parts
                    .get(1)
                    .map(|u| {
                        u.split("//")
                            .nth(1)
                            .unwrap_or(u)
                            .split('/')
                            .next()
                            .unwrap_or(u)
                    })
                    .unwrap_or("unknown")
            ),
            Some("/scaffold") => format!(
                "Scaffold: {}",
                parts.get(1..).map(|p| p.join(" ")).unwrap_or_default()
            ),
            Some("/template") => format!("Template: {}", parts.get(1).unwrap_or(&"")),
            Some(cmd) if cmd.len() > 1 => cmd[1..].to_string(), // Strip / and use command name
            _ => "New Conversation".into(),
        };
    }

    // For regular messages, try to extract a meaningful title
    // Remove common prefixes (leading punctuation)
    let cleaned = msg
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .to_string();

    if cleaned.is_empty() {
        return "New Conversation".into();
    }

    // If it's a question or ends a sentence, use the first sentence
    if let Some(end) = cleaned.find(['?', '.', '\n']) {
        let title: String = cleaned[..=end].chars().take(60).collect();
        return title;
    }

    // Otherwise use first 50 chars at a word boundary
    if cleaned.len() <= 50 {
        return cleaned;
    }

    // Char-safe truncation: byte slicing would panic on multi-byte
    // characters (emoji, CJK) that cross byte index 50.
    let truncated: String = cleaned.chars().take(50).collect();
    if let Some(space) = truncated.rfind(' ') {
        truncated[..space].to_string()
    } else {
        truncated
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Submit the user's message and spawn a streaming response task.
///
/// Handles slash commands (`/help`, `/clear`, `/new`, `/model`, `/models`,
/// `/url`, `/run`, `/pipe`, `/diff`, `/test`, `/build`, `/git`) before
/// falling through to the normal AI chat path.
pub(crate) async fn submit_message(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    if app.is_streaming {
        app.set_status("Already streaming \u{2014} press Esc to cancel first");
        return;
    }

    // ── Slash-command dispatch ──────────────────────────────────────────
    if text.starts_with('/') && commands::handle(app, text, provider).await {
        return;
        // Not a recognised command — treat as a normal message.
    }

    // ── Auto-agent: detect intent and temporarily enable tools ────────
    if app.auto_agent && !app.agent_mode && crate::agent::intent::needs_tools(text) {
        app.agent_mode = true;
        app.auto_agent_active = true;

        // Inject the tools system prompt (same as `/agent on`).
        let tools_prompt = crate::agent::tools::tools_system_prompt();
        app.current_conversation_mut().messages.retain(|(r, c)| {
            !(r == "system"
                && (c.contains("You have access to the following tools")
                    || c.contains("You are Nerve, an AI coding assistant")))
        });
        app.current_conversation_mut()
            .messages
            .insert(0, ("system".into(), tools_prompt));

        // Inject workspace context if available.
        let ws_for_agent = app
            .cached_workspace
            .clone()
            .or_else(crate::workspace::detect_workspace);
        if let Some(ws) = ws_for_agent {
            let project_map = crate::workspace::generate_project_map(&ws.root, 3);
            // `len()` is byte length (O(1)); if it's over the threshold
            // we definitely need to truncate. Take the first N *chars*
            // (not bytes) so we never slice through a multi-byte UTF-8
            // boundary — project paths can contain CJK or emoji.
            let map_context = if project_map.len() > 2000 {
                let head: String = project_map.chars().take(2000).collect();
                format!("{head}...\n[Project map truncated]")
            } else {
                project_map
            };
            app.current_conversation_mut().messages.insert(
                1,
                (
                    "system".into(),
                    format!("Current project context:\n\n{map_context}"),
                ),
            );
        }

        app.set_status("Auto-agent: tool access enabled for this request");
    }

    send_to_ai(app, text, provider).await;
}

/// Build the messages array from conversation history and start streaming.
async fn send_to_ai(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    app.add_user_message(text.to_string());
    app.scroll_offset = 0;
    send_to_ai_from_history(app, provider).await;
}

/// Prepend the active pipeline role's system prompt (and, for tool-capable
/// roles, the tools-format prompt) to a freshly built message list.
///
/// Pipeline role prompts are ephemeral — they are never persisted into
/// `conversation.messages` — so every LLM call for that role (the initial
/// turn AND any follow-up turn after a tool round) must re-inject them, or
/// the model loses its role and the tool-call format. No-op when no pipeline
/// role is active.
pub(crate) fn inject_pipeline_role_prompts(app: &App, messages: &mut Vec<ChatMessage>) {
    use crate::agent::pipeline::ToolPolicy;
    let Some(role) = app.pipeline.as_ref().and_then(|p| p.step.role()) else {
        return;
    };
    // Strip any stale agent-tools system message from prior turns so the
    // planner (no tools) doesn't see tool docs and we don't duplicate them.
    // The tools prompt opens with "You are Nerve, an AI coding assistant".
    messages.retain(|m| {
        !(m.role == "system"
            && m.content
                .starts_with("You are Nerve, an AI coding assistant"))
    });
    // Prepend tool docs if this role has tool access; the reviewer's own
    // system prompt restricts its USAGE to read-only.
    if matches!(role.tool_policy, ToolPolicy::Full | ToolPolicy::ReadOnly) {
        messages.insert(
            0,
            ChatMessage::system(crate::agent::tools::tools_system_prompt()),
        );
    }
    // Role prompt sits at position 0 so the LLM reads it first.
    messages.insert(0, ChatMessage::system(role.system_prompt.clone()));
}

/// Build the outgoing `ChatMessage` list for a turn from the current
/// conversation: context compaction, `@file` expansion on the newest user turn,
/// knowledge-base + auto-context injection, and the mode / pipeline-role system
/// prompt.
///
/// Shared by the initial send (`send_to_ai_from_history`) and the post-tool
/// rebuild (`AgentToolsComplete`) so both turns carry IDENTICAL context. Before
/// this was factored out, the rebuild reconstructed messages by hand and
/// silently dropped the mode prompt, KB context, and file expansion after the
/// first tool round.
///
/// `search_query` is the text used for KB search / auto-context gathering (the
/// user's actual request — not tool-result payloads).
pub(crate) fn build_context_messages(app: &mut App, search_query: &str) -> Vec<ChatMessage> {
    // Apply context management based on provider (halved in Efficient mode).
    let base_limit = crate::agent::context::ContextManager::effective_limit(
        &app.selected_provider,
        app.context_limit_override,
    );
    let limit = if app.active_mode == app::NerveMode::Efficient {
        base_limit / 2
    } else {
        base_limit
    };
    let cm = crate::agent::context::ContextManager::new(limit);

    // Expand @file references on ONLY the most recent user turn.
    //
    // Expanding every historical user message re-read each @file from disk (up
    // to 1 MB each) and re-injected its full contents on *every* request —
    // quadratic token growth as the conversation grew. The file content was
    // already delivered to the model on the turn it was referenced; re-sending
    // it every turn thereafter is pure waste. We expand BEFORE compaction so
    // the token-limit decision reflects the true payload actually sent, not the
    // pre-expansion `@path` placeholder text.
    let raw = &app.current_conversation().messages;
    let last_user_idx = raw.iter().rposition(|(role, _)| role == "user");
    let expanded: Vec<(String, String)> = raw
        .iter()
        .enumerate()
        .map(|(i, (role, content))| {
            if role == "user" && Some(i) == last_user_idx {
                (role.clone(), files::expand_file_references(content))
            } else {
                (role.clone(), content.clone())
            }
        })
        .collect();

    // First compact tool results if in agent mode, then compact overall.
    let conversation_messages = if app.agent_mode {
        cm.compact_tool_results(&expanded)
    } else {
        expanded
    };
    let final_messages = cm.compact_messages(&conversation_messages);

    let mut messages: Vec<ChatMessage> = final_messages
        .iter()
        .filter_map(|(role, content)| match role.as_str() {
            "user" => Some(ChatMessage::user(content)),
            "assistant" => Some(ChatMessage::assistant(content)),
            "system" => Some(ChatMessage::system(content)),
            _ => None,
        })
        .collect();

    // If a knowledge base exists, search for relevant context and inject it.
    if !search_query.is_empty()
        && let Ok(kb) = knowledge::KnowledgeBase::load("default")
        && !kb.chunks.is_empty()
    {
        let results = knowledge::search_knowledge(&kb, search_query, 3);
        if !results.is_empty() {
            let context = results
                .iter()
                .map(|r| format!("[From: {}]\n{}", r.document_title, r.chunk.content))
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");

            messages.insert(
                0,
                ChatMessage::system(format!(
                    "The following knowledge base context may be relevant \
                     to the user's query:\n\n{context}\n\n\
                     Use this context to inform your response if relevant."
                )),
            );
        }
    }

    // Auto-context: gather relevant files when NOT in agent mode
    // (agent mode reads files on demand via tools).
    if !app.agent_mode && app.auto_agent {
        let ws_root = crate::workspace::detect_workspace().map(|w| w.root);
        let ctx = crate::agent::auto_context::gather_context(search_query, ws_root.as_deref());
        if let Some(context_msg) = crate::agent::auto_context::format_context(&ctx) {
            messages.insert(0, ChatMessage::system(context_msg));
        }
    }

    // Inject mode-specific system prompt at position 0 so it shapes the
    // entire conversation. If a multi-agent pipeline is active, its
    // current role's prompt REPLACES the mode prompt — the role fully
    // owns the system context for that turn.
    if let Some(role) = app.pipeline.as_ref().and_then(|p| p.step.role()) {
        // Re-inject the role + tools prompt (shared with the post-tool
        // AgentToolsComplete path so both turns carry the same context).
        inject_pipeline_role_prompts(app, &mut messages);

        // Align agent_mode with the role so the Done handler's tool-call
        // parsing kicks in for Coder and Reviewer.
        app.agent_mode = !matches!(role.tool_policy, crate::agent::pipeline::ToolPolicy::None);
    } else if let Some(mode_prompt) = app.active_mode.system_prompt() {
        messages.insert(0, ChatMessage::system(mode_prompt.to_string()));
    }

    // Update the context-usage tracker (raw conversation) for the status bar,
    // and record the ACTUAL sent-payload size (post-expansion/compaction plus
    // injected system prompts) for usage accounting.
    app.total_tokens_used = crate::agent::context::ContextManager::conversation_tokens(
        &app.current_conversation().messages,
    );
    app.last_sent_tokens = messages
        .iter()
        .map(|m| crate::agent::context::ContextManager::estimate_tokens(&m.content))
        .sum();

    messages
}

/// Start a streaming AI request using the current conversation history.
/// Assumes the caller has already added the user message to the conversation.
pub(crate) async fn send_to_ai_from_history(app: &mut App, provider: &Arc<dyn AiProvider>) {
    if app.is_streaming {
        app.set_status("Already streaming \u{2014} press Esc to cancel first");
        return;
    }

    // Check spending limit before sending.
    {
        let estimated_tokens: usize = app
            .current_conversation()
            .messages
            .iter()
            .map(|(_, c)| c.len() / 4 + 1)
            .sum::<usize>()
            + 4000; // +4000 for expected response
        if let Some(warning) = app.spending_limit.would_exceed(
            &app.usage_stats,
            estimated_tokens,
            &app.selected_provider,
            &app.selected_model,
        ) {
            app.add_assistant_message(format!("Warning: {warning}"));
            return;
        }
    }

    // Find the most recent user message for KB context lookup.
    let user_message = app
        .current_conversation()
        .messages
        .iter()
        .rev()
        .find(|(role, _)| role == "user")
        .map(|(_, content)| content.clone())
        .unwrap_or_default();

    let messages = build_context_messages(app, &user_message);

    let model = app.selected_model.clone();
    let (tx, rx) = mpsc::unbounded_channel();

    // Cancel any prior in-flight stream before we replace the receiver —
    // otherwise the old task keeps running against a dropped receiver.
    app.cancel_active_stream();

    app.stream_rx = Some(rx);
    app.is_streaming = true;
    app.streaming_response.clear();
    app.streaming_start = Some(std::time::Instant::now());

    let provider = Arc::clone(provider);
    let handle = tokio::spawn(async move {
        if let Err(e) = provider.chat_stream(&messages, &model, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error(e.to_string()));
        }
    });
    app.stream_abort = Some(handle.abort_handle());
}

/// Advance the active multi-agent workflow one step. Called from the
/// event-loop `Done` branch when the current role's turn ends with no
/// outstanding tool calls.
///
/// Returns the step the pipeline landed on, or `None` if there was no
/// active pipeline (idempotent). When the pipeline transitions to a new
/// role, this function appends the role's handoff message to the
/// conversation and kicks off the next streaming call via
/// `send_to_ai_from_history`. When it transitions to `Done`, it clears
/// `app.pipeline` and sets a completion status.
///
/// Extracted from the inline Done handler so the state machine can be
/// driven from tests with a mock provider (see
/// `pipeline_advances_through_all_three_roles` in the tests module).
pub(crate) async fn advance_pipeline_if_active(
    app: &mut App,
    provider: &Arc<dyn AiProvider>,
) -> Option<crate::agent::pipeline::PipelineStep> {
    use crate::agent::pipeline::{PipelineStep, should_iterate_on_feedback};

    // No active workflow — nothing to do.
    let current_step = app.pipeline.as_ref()?.step;

    // Special case: when the Reviewer's turn just finished, look at its
    // verdict. If it asked for fixes and we still have iteration
    // budget, loop back to the Coder with the reviewer's feedback as
    // context instead of finishing the workflow.
    let iteration_handoff = if current_step == PipelineStep::Reviewing {
        let reviewer_msg = app
            .current_conversation()
            .messages
            .iter()
            .rfind(|(r, _)| r == "assistant")
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        let iterations = app.pipeline.as_ref()?.iterations_used;
        if should_iterate_on_feedback(&reviewer_msg, iterations) {
            // Loop back: drive Coding again with the feedback in
            // conversation. The state's iteration counter is bumped so
            // we don't loop forever.
            app.pipeline.as_mut()?.iterate_back_to_coding();
            Some(app.pipeline.as_ref()?.iterations_used)
        } else {
            None
        }
    } else {
        None
    };

    let (next_step, task) = if iteration_handoff.is_some() {
        // We already set step = Coding via iterate_back_to_coding;
        // don't call advance() again.
        let state = app.pipeline.as_ref()?;
        (state.step, state.task.clone())
    } else {
        let state = app.pipeline.as_mut()?;
        state.advance();
        (state.step, state.task.clone())
    };
    // Each role gets its own fresh agent-iteration budget.
    app.agent_iterations = 0;

    if next_step == PipelineStep::Done {
        let task_preview: String = task.chars().take(60).collect();
        app.set_status(format!("Workflow complete: {task_preview}"));
        app.pipeline = None;
        // The Coder/Reviewer roles enabled agent mode; reset it so the
        // user's next ordinary message isn't silently executed as agent.
        app.agent_mode = false;
        return Some(PipelineStep::Done);
    }

    if let Some(role) = next_step.role() {
        let step_num = match next_step {
            PipelineStep::Planning => 1,
            PipelineStep::Coding => 2,
            PipelineStep::Reviewing => 3,
            PipelineStep::Done => 0,
        };
        // Distinct status when the coder is on a feedback iteration so
        // the user sees we're not stuck repeating the first attempt.
        if let Some(iter_n) = iteration_handoff {
            app.set_status(format!(
                "Workflow: Coder iteration {iter_n}/{} (addressing reviewer feedback)",
                crate::agent::pipeline::MAX_ITERATIONS
            ));
            // Specific handoff message for iterations: tells the Coder
            // to read the reviewer's feedback above and address it.
            app.add_user_message(
                "The reviewer asked for fixes (verdict above). Read their \
                 findings carefully, address each one, and run any \
                 verification commands again. Do not start over — only \
                 fix what the reviewer flagged."
                    .into(),
            );
        } else {
            app.set_status(format!(
                "Workflow: {} (step {step_num}/3)",
                next_step.label()
            ));
            if !role.handoff_prompt.is_empty() {
                app.add_user_message(role.handoff_prompt.clone());
            }
        }
        // send_to_ai_from_history sets up a new stream_rx and abort
        // handle; the drain loop picks it up on its next iteration.
        send_to_ai_from_history(app, provider).await;
    }
    Some(next_step)
}

/// Async tool runner for agent mode.
///
/// Runs each tool call on the tokio blocking pool so the UI event loop keeps
/// rendering while slow commands (`npm install`, test runs, etc.) execute.
/// Emits `ToolStart` / `ToolDone` events per call so the status bar updates
/// in real time, and a final `AgentToolsComplete` carrying the aggregated
/// results string to inject into the conversation as a user message before
/// the next LLM call.
///
/// `policy` gates which tools may actually run. In the multi-agent
/// pipeline, the Reviewer role passes `ToolPolicy::ReadOnly`, which causes
/// write-capable tool calls (`write_file`, `edit_file`, `run_command`,
/// `create_directory`) to be refused before execution. The refusal is
/// reported back to the LLM as a failed tool result so it can adjust.
///
/// This function is engineered to always reach its final
/// `AgentToolsComplete` send — even when the receiver is dropped
/// mid-way — so the drain loop can never get stuck with `is_streaming=true`
/// and an orphaned receiver.
pub(crate) async fn run_agent_tools_task(
    tool_calls: Vec<crate::agent::tools::ToolCall>,
    timeout_secs: u64,
    policy: crate::agent::pipeline::ToolPolicy,
    tx: mpsc::UnboundedSender<StreamEvent>,
) {
    use crate::agent::pipeline::ToolPolicy;

    // Tools that mutate the filesystem or execute arbitrary commands.
    // ReadOnly roles may call everything else.
    const WRITE_TOOLS: &[&str] = &["write_file", "edit_file", "run_command", "create_directory"];

    let mut results = String::from("I executed your tool calls. Here are the results:\n\n");
    let mut all_success = true;
    let total = tool_calls.len();

    for (idx, call) in tool_calls.into_iter().enumerate() {
        let args_summary = call
            .args
            .iter()
            .map(|(k, v)| {
                let short: String = v.chars().take(40).collect();
                format!("{k}={short}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let start_summary = format!("{}/{} {} ({args_summary})", idx + 1, total, call.tool);
        // `break` (not `return`) on send failure so the final
        // AgentToolsComplete send still runs — that's the signal the
        // drain loop needs to finalise `is_streaming`.
        if tx
            .send(StreamEvent::ToolStart {
                tool: call.tool.clone(),
                summary: start_summary,
            })
            .is_err()
        {
            break;
        }

        let blocked = policy == ToolPolicy::ReadOnly && WRITE_TOOLS.contains(&call.tool.as_str());

        let result = if blocked {
            all_success = false;
            crate::agent::tools::ToolResult {
                tool: call.tool.clone(),
                success: false,
                output: format!(
                    "Blocked: tool `{}` is not permitted in read-only mode. \
                     The Reviewer role may only call read_file, read_lines, \
                     search_code, list_files, and find_files — report your \
                     findings without writing or running code.",
                    call.tool
                ),
            }
        } else {
            // Run the blocking tool on the spawn_blocking pool. If the
            // task itself panics we surface a synthetic error result
            // rather than letting the panic tear down the runtime.
            tokio::task::spawn_blocking(move || {
                crate::agent::tools::execute_tool(&call, timeout_secs)
            })
            .await
            .unwrap_or_else(|e| crate::agent::tools::ToolResult {
                tool: "<panicked>".into(),
                success: false,
                output: format!("tool task panicked: {e}"),
            })
        };

        if !result.success {
            all_success = false;
        }

        let preview: String = result.output.chars().take(80).collect();
        if tx
            .send(StreamEvent::ToolDone {
                tool: result.tool.clone(),
                success: result.success,
                output_preview: preview,
            })
            .is_err()
        {
            break;
        }

        let status_icon = if result.success { "OK" } else { "ERROR" };
        // Char-safe truncation — tool output can be arbitrary bytes
        // (file contents in any language, subprocess stdout, etc.).
        let output_str = if result.output.len() > 5000 {
            let head: String = result.output.chars().take(5000).collect();
            format!(
                "{head}...\n[Output truncated: {} bytes total]",
                result.output.len()
            )
        } else {
            result.output.clone()
        };
        results.push_str(&format!(
            "### Tool {}: {} [{}]\n```\n{}\n```\n\n",
            idx + 1,
            result.tool,
            status_icon,
            output_str,
        ));
    }

    if !all_success {
        results.push_str(
            "Some tools failed. Please review the errors above and adjust your approach.\n",
        );
    }

    // Always attempt the terminator, even if we broke out of the loop
    // on a send error. If the receiver really is gone, this send is a
    // harmless no-op; if the receiver is still alive it lets the drain
    // loop wind down cleanly instead of sitting in an "is_streaming"
    // state with no more events coming.
    let _ = tx.send(StreamEvent::AgentToolsComplete {
        user_message: results,
    });
}

/// Build a short human-readable summary of pending tool calls for the status
/// bar ("Reading src/main.rs", "Running: cargo test", etc.).
pub(crate) fn format_agent_action_summary(tool_calls: &[crate::agent::tools::ToolCall]) -> String {
    let mut out = String::new();
    for call in tool_calls {
        let brief = match call.tool.as_str() {
            "read_file" => format!("Reading {}", call.args.get("path").unwrap_or(&"?".into())),
            "write_file" => format!("Writing {}", call.args.get("path").unwrap_or(&"?".into())),
            "edit_file" => format!("Editing {}", call.args.get("path").unwrap_or(&"?".into())),
            "run_command" => format!(
                "Running: {}",
                call.args.get("command").unwrap_or(&"?".into())
            ),
            "list_files" => format!("Listing {}", call.args.get("path").unwrap_or(&".".into())),
            "search_code" => format!(
                "Searching for '{}'",
                call.args.get("pattern").unwrap_or(&"?".into())
            ),
            "create_directory" => {
                format!("Creating {}", call.args.get("path").unwrap_or(&"?".into()))
            }
            "find_files" => format!(
                "Finding {}",
                call.args.get("pattern").unwrap_or(&"*".into())
            ),
            "read_lines" => format!(
                "Reading lines from {}",
                call.args.get("path").unwrap_or(&"?".into())
            ),
            _ => call.tool.to_string(),
        };
        out.push_str(&format!("  > {brief}\n"));
    }
    out
}

/// Regenerate the last assistant response by removing it and re-sending.
pub(crate) async fn regenerate_response(
    app: &mut App,
    provider: &Arc<dyn AiProvider>,
    _config: &Config,
) {
    if app.is_streaming {
        return;
    }

    let conv = app.current_conversation_mut();
    // Remove the last assistant message
    if let Some(pos) = conv
        .messages
        .iter()
        .rposition(|(role, _)| role == "assistant")
    {
        conv.messages.remove(pos);
    } else {
        app.set_status("No response to regenerate");
        return;
    }

    // Apply context management based on provider (halved in Efficient mode)
    let base_limit = crate::agent::context::ContextManager::effective_limit(
        &app.selected_provider,
        app.context_limit_override,
    );
    let limit = if app.active_mode == app::NerveMode::Efficient {
        base_limit / 2
    } else {
        base_limit
    };
    let cm = crate::agent::context::ContextManager::new(limit);

    let conversation_messages = if app.agent_mode {
        cm.compact_tool_results(&app.current_conversation().messages)
    } else {
        app.current_conversation().messages.clone()
    };
    let final_messages = cm.compact_messages(&conversation_messages);

    // Rebuild messages and re-send (expand @file references in user messages)
    let messages: Vec<ChatMessage> = final_messages
        .iter()
        .filter_map(|(role, content)| match role.as_str() {
            "user" => {
                let expanded = files::expand_file_references(content);
                Some(ChatMessage::user(expanded))
            }
            "assistant" => Some(ChatMessage::assistant(content)),
            "system" => Some(ChatMessage::system(content)),
            _ => None,
        })
        .collect();

    let model = app.selected_model.clone();
    let (tx, rx) = mpsc::unbounded_channel();
    app.cancel_active_stream();
    app.stream_rx = Some(rx);
    app.is_streaming = true;
    app.streaming_response.clear();
    app.streaming_start = Some(std::time::Instant::now());
    app.scroll_offset = 0;
    app.set_status("Regenerating...");

    let provider = Arc::clone(provider);
    let handle = tokio::spawn(async move {
        if let Err(e) = provider.chat_stream(&messages, &model, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error(e.to_string()));
        }
    });
    app.stream_abort = Some(handle.abort_handle());
}

/// Edit the last user message: load it back into the input buffer and remove
/// it (plus any assistant response after it) from the conversation.
pub(crate) fn edit_last_message(app: &mut App) {
    if app.is_streaming {
        return;
    }

    let conv = app.current_conversation_mut();

    // Find the last user message
    if let Some(pos) = conv.messages.iter().rposition(|(role, _)| role == "user") {
        let (_, content) = conv.messages[pos].clone();

        // Remove the user message and everything after it (the response)
        conv.messages.truncate(pos);

        // Load into input
        app.input = content;
        app.cursor_position = app.input.len();
        app.input_mode = InputMode::Insert;
        app.set_status("Editing last message \u{2014} press Enter to resend");
    } else {
        app.set_status("No message to edit");
    }
}

/// Delete the last message exchange (assistant + preceding user message).
pub(crate) fn delete_last_exchange(app: &mut App) {
    if app.is_streaming {
        return;
    }
    let conv = app.current_conversation_mut();
    if conv.messages.is_empty() {
        return;
    }

    // Remove last message
    let last_role = conv.messages.last().map(|(r, _)| r.clone());
    conv.messages.pop();

    // If we removed an assistant message, also remove the preceding user message
    if last_role.as_deref() == Some("assistant")
        && conv.messages.last().map(|(r, _)| r.as_str()) == Some("user")
    {
        conv.messages.pop();
    }

    app.set_status("Deleted last exchange");
}

/// Copy the last assistant message to the system clipboard.
pub(crate) fn copy_last_assistant_message(app: &mut App) {
    let last = app
        .current_conversation()
        .messages
        .iter()
        .rev()
        .find(|(role, _)| role == "assistant")
        .map(|(_, content)| content.clone());

    match last {
        Some(text) => match clipboard::copy_to_clipboard(&text) {
            Ok(()) => {
                app.clipboard_manager.add(text, ClipboardSource::ManualCopy);
                let _ = app.clipboard_manager.save();
                app.set_status("Copied to clipboard");
            }
            Err(e) => {
                app.set_status(format!("Clipboard error: {e}"));
            }
        },
        None => {
            app.set_status("No assistant message to copy");
        }
    }
}

/// Clear the active conversation's messages and reset streaming state.
pub(crate) fn clear_conversation(app: &mut App) {
    // Abort any in-flight provider/agent task first — otherwise the spawned
    // task (and any subprocess it launched) keeps running detached and its
    // abort handle dangles, exactly like new_conversation/finish_streaming.
    app.cancel_active_stream();
    app.current_conversation_mut().messages.clear();
    app.streaming_response.clear();
    app.is_streaming = false;
    app.stream_rx = None;
    app.streaming_start = None;
    app.scroll_offset = 0;
    app.set_status("Conversation cleared");
}

/// Cycle to the next conversation (wraps around).
pub(crate) fn cycle_conversation(app: &mut App) {
    if app.conversations.len() > 1 {
        app.active_conversation = (app.active_conversation + 1) % app.conversations.len();
        app.scroll_offset = 0;
        app.set_status(format!(
            "Switched to conversation {}",
            app.active_conversation + 1
        ));
    }
}

pub(crate) fn cycle_conversation_back(app: &mut App) {
    if app.conversations.len() > 1 {
        app.active_conversation = if app.active_conversation == 0 {
            app.conversations.len() - 1
        } else {
            app.active_conversation - 1
        };
        app.scroll_offset = 0;
        app.set_status(format!(
            "Switched to conversation {}",
            app.active_conversation + 1
        ));
    }
}
