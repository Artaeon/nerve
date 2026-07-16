use std::sync::Arc;

use tokio::sync::mpsc;

use crate::ai::provider::{AiProvider, ChatMessage, StreamEvent};
use crate::app::{self, App, InputMode};
use crate::clipboard_manager::ClipboardSource;
use crate::config::Config;
use crate::{clipboard, commands, files, knowledge};

/// Prefix of the synthetic "user" message that carries aggregated tool-run
/// output back to the model. Single source of truth so the post-tool rebuild
/// can reliably distinguish this dump from a real user request (and so
/// compaction can recognise it). Used by `run_agent_tools_task` and the
/// `AgentToolsComplete` handler in main.rs.
pub(crate) const TOOL_RESULTS_PREFIX: &str = "I executed your tool calls. Here are the results:";

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

/// True if `text` looks like a slash-command invocation — the first token is
/// `/word` with no interior path separator — as opposed to a normal message
/// that merely begins with a path such as `/usr/bin/x` or `/home/me/f.rs`.
pub(crate) fn looks_like_slash_command(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    first.len() > 1 && first.starts_with('/') && !first[1..].contains('/')
}

/// Handles slash commands (`/help`, `/clear`, `/new`, `/model`, `/models`,
/// `/url`, `/run`, `/pipe`, `/diff`, `/test`, `/build`, `/git`) before
/// falling through to the normal AI chat path.
pub(crate) async fn submit_message(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    if app.is_streaming {
        app.set_status("Already streaming \u{2014} press Esc to cancel first");
        return;
    }

    // ── Slash-command dispatch ──────────────────────────────────────────
    if text.starts_with('/') {
        if commands::handle(app, text, provider).await {
            return;
        }
        // Nothing matched. If the message clearly *looks* like a slash-command
        // attempt, don't silently ship the typo off to the model — tell the
        // user and stop. A message that merely starts with a path like
        // `/usr/bin/x` still falls through and is treated as normal chat.
        if looks_like_slash_command(text) {
            let first = text.split_whitespace().next().unwrap_or("");
            app.set_status(format!(
                "Unknown command {first} — press Ctrl+K or type /help to see commands"
            ));
            return;
        }
    }

    // ── Plan-approval gate: block ordinary messages while parked ──────
    // When a workflow is paused waiting for the user to review its plan,
    // a normal chat message must not slip through (it would otherwise get
    // a plain reply and muddy the paused state). Only /approve and /reject
    // — handled above as slash commands — may proceed.
    if app
        .pipeline
        .as_ref()
        .is_some_and(|p| p.step == crate::agent::pipeline::PipelineStep::AwaitingApproval)
    {
        app.set_status("Plan awaiting approval — /approve to run it, /reject to cancel");
        return;
    }

    // ── Auto-agent: detect intent and temporarily enable tools ────────
    // Inside a detected workspace, coding work is the default: any message
    // that isn't clearly conversational activates the agent. Outside a
    // workspace only strong signals do.
    let in_workspace = app.cached_workspace.is_some();
    if app.auto_agent
        && !app.agent_mode
        && crate::agent::intent::should_activate_agent(text, in_workspace)
    {
        app.agent_mode = true;
        app.auto_agent_active = true;

        // Inject the tools system prompt (same as `/agent on`).
        let tools_prompt = crate::agent::tools::tools_system_prompt();
        app.current_conversation_mut().messages.retain(|(r, c)| {
            !(r == "system"
                && (c.contains("You have access to the following tools")
                    || c.contains("You are Nerve, an AI coding assistant")
                    || c.starts_with("Current project context:")))
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
    // Fresh user turn: reset the per-turn verify gate + edited-files state.
    app.agent_made_edits = false;
    app.verify_rounds = 0;
    app.turn_edited_files.clear();
    // Durability: persist the user's turn to history BEFORE streaming starts.
    // Previously the conversation was only written when the response completed
    // (StreamEvent::Done), so a crash/kill mid-response silently lost the user's
    // just-typed message. Now the turn survives regardless of what happens next.
    persist_current_conversation(app);
    send_to_ai_from_history(app, provider).await;
}

/// Write the active conversation to history (best-effort). This is the single
/// source of truth for turning the in-memory conversation into a
/// `ConversationRecord`, used after the user's message is added, when a
/// response completes, and on quit — so all three paths save identical records
/// and no turn is ever lost to a crash mid-stream. A save failure is
/// intentionally non-fatal to the UI.
pub(crate) fn persist_current_conversation(app: &App) {
    if let Some(record) = conversation_record(app) {
        let _ = crate::history::save_conversation(&record);
    }
}

/// Build a `ConversationRecord` from the active conversation, or `None` when
/// there is no real turn to save yet (only seeded system prompts). Split out
/// from the disk write so the "don't persist an empty conversation" guard is
/// unit-testable without touching the filesystem.
fn conversation_record(app: &App) -> Option<crate::history::ConversationRecord> {
    let conv = app.current_conversation();
    if !conv
        .messages
        .iter()
        .any(|(r, _)| r == "user" || r == "assistant")
    {
        return None;
    }
    Some(crate::history::ConversationRecord {
        id: conv.id.clone(),
        title: conv.title.clone(),
        messages: conv
            .messages
            .iter()
            .map(|(role, content)| crate::history::MessageRecord {
                role: role.clone(),
                content: content.clone(),
                timestamp: chrono::Utc::now(),
            })
            .collect(),
        model: app.selected_model.clone(),
        provider: app.selected_provider.clone(),
        created_at: conv.created_at,
        updated_at: chrono::Utc::now(),
    })
}

/// Choose the model for the turn that is about to stream and remember it on the
/// app, applying prompt/role-based routing (see [`crate::model_router`]). The
/// remembered value lets agent tool-round continuations — which no longer carry
/// the original request text to classify — reuse the same model. When the
/// routed model differs from the user's selection, a status line makes it
/// visible so routing is never silent.
fn route_turn_model(app: &mut App, message: &str) -> String {
    let step = app.pipeline.as_ref().map(|p| p.step);
    let model = crate::model_router::route(
        app.auto_model_routing,
        &app.selected_provider,
        &app.selected_model,
        step,
        message,
    );
    app.active_turn_model = Some(model.clone());
    if model != app.selected_model {
        let why = match step.and_then(crate::model_router::tier_for_step) {
            Some(_) => "for this step",
            None => "for this turn",
        };
        app.set_status(format!("Routed to {model} {why}"));
    }
    model
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

    // Transparency: the first time compaction actually summarizes older turns,
    // tell the user (older detail is now a lossy summary — they may want /new or
    // a higher context_limit). Only fire on the transition, not every send.
    let did_compact = final_messages.len() < conversation_messages.len();
    if did_compact && !app.context_compacting {
        let summarized = conversation_messages.len() - final_messages.len();
        app.set_status(format!(
            "Context compacted: summarized {summarized} older message(s) to fit the window"
        ));
    }
    app.context_compacting = did_compact;

    let mut messages: Vec<ChatMessage> = final_messages
        .iter()
        .filter_map(|(role, content)| match role.as_str() {
            "user" => Some(ChatMessage::user(content)),
            "assistant" => Some(ChatMessage::assistant(content)),
            "system" => Some(ChatMessage::system(content)),
            _ => None,
        })
        .collect();

    // Per-project memory (.nerve/): the TUI now builds this through the SAME
    // shared `project_context::build` the headless worker uses — see that
    // module's docs for the measured evidence (`grep -c recall
    // src/agent/headless.rs` == 0 across 2,362 real tool calls) that drove
    // this unification. GENUINE BEHAVIOUR CHANGE, called out explicitly: the
    // old code here only ever PULLED memory — a tiny always-on header plus
    // query-gated recall — so a fact stored via `store.remember()` stayed out
    // of an unrelated turn's context entirely. `project_context::build`
    // intentionally mirrors the worker's unconditional PUSH model (brief +
    // memory + design + decisions, every turn, regardless of relevance)
    // because the whole point of the anti-drift refactor is that both
    // callers go through ONE implementation instead of each maintaining its
    // own policy — so a remembered fact now appears on every turn, not just
    // turns whose query happens to match it. `always_on_context` is kept
    // exactly as before (not duplicated inside `project_context::build`) and
    // still goes in ahead of everything else.
    if let Some(ws) = &app.cached_workspace {
        let store = crate::project::ProjectStore::for_workspace(&ws.root);

        let opts = crate::project_context::ContextOptions {
            recall_query: Some(search_query),
            include_design: crate::memory_recall::is_design_request(search_query),
        };
        let sections = crate::project_context::build(&store, &opts);
        // Insert in REVERSE so the first section in `sections` (brief) ends
        // up closest to the front, giving the final relative order: brief →
        // memory → design → decisions → recall-last, matching
        // `project_context::build`'s documented order.
        for section in sections.into_iter().rev() {
            messages.insert(0, ChatMessage::system(section));
        }

        if let Some(header) = crate::memory_recall::always_on_context(&store, 1200) {
            messages.insert(0, ChatMessage::system(header));
        }
    }

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
        let estimated_tokens = crate::agent::context::ContextManager::conversation_tokens(
            &app.current_conversation().messages,
        ) + 4000; // +4000 for expected response
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

    let model = route_turn_model(app, &user_message);
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
    advance_pipeline_inner(app, provider, false).await
}

/// Advance a pipeline that is parked at the plan-approval gate. This is the
/// ONLY path allowed to move past `AwaitingApproval` — the event-loop Done
/// handler uses `advance_pipeline_if_active` (approved = false), which leaves
/// a parked pipeline untouched so a stray interim message can't trigger the
/// coder without the user's `/approve`.
pub(crate) async fn approve_and_advance_pipeline(
    app: &mut App,
    provider: &Arc<dyn AiProvider>,
) -> Option<crate::agent::pipeline::PipelineStep> {
    advance_pipeline_inner(app, provider, true).await
}

async fn advance_pipeline_inner(
    app: &mut App,
    provider: &Arc<dyn AiProvider>,
    approved: bool,
) -> Option<crate::agent::pipeline::PipelineStep> {
    use crate::agent::pipeline::{PipelineStep, should_iterate_on_feedback};

    // No active workflow — nothing to do.
    let current_step = app.pipeline.as_ref()?.step;

    // A pipeline parked at the approval gate must never be advanced by the
    // event-loop Done path — only an explicit `/approve` (approved = true)
    // may move it forward. Without this, sending any ordinary message while
    // the plan is on screen would stream a turn whose Done handler advances
    // AwaitingApproval → Coding and executes the plan without consent.
    if current_step == PipelineStep::AwaitingApproval && !approved {
        return Some(PipelineStep::AwaitingApproval);
    }

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

    let (mut next_step, task) = if iteration_handoff.is_some() {
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

    // ── Plan-approval gate ──────────────────────────────────────────────
    // The planner just finished: pause here. Nothing executes until the
    // user reviews the plan and consents. `/approve` re-enters this
    // function (current step AwaitingApproval → advances to Coding);
    // `/reject` clears the pipeline. workflow_auto_approve restores the
    // old un-gated behaviour for users who want it.
    if next_step == PipelineStep::AwaitingApproval {
        if app.workflow_auto_approve {
            let state = app.pipeline.as_mut()?;
            state.advance();
            next_step = state.step; // → Coding
        } else {
            // No LLM turn runs while waiting; make sure an ordinary chat
            // message typed now isn't executed as an agent turn.
            app.agent_mode = false;
            app.add_assistant_message(
                "**Plan ready for review.** Nothing has been executed yet.\n\n\
                 - `/approve` — run the plan (coder → reviewer)\n\
                 - `/reject` — cancel the workflow"
                    .into(),
            );
            app.set_status("Plan awaiting approval — /approve to execute, /reject to cancel");
            return Some(PipelineStep::AwaitingApproval);
        }
    }

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
            // No LLM turn runs in these steps (role() is None), so they
            // never reach this branch — 0 keeps the match exhaustive.
            PipelineStep::AwaitingApproval | PipelineStep::Done => 0,
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

    // Tools that mutate state (filesystem, shell, or persistent project
    // memory). ReadOnly roles may call everything else. `remember` and
    // `update_tasks` write into .nerve/ — which is injected into every
    // future prompt — so a ReadOnly role (the pre-approval Planner, the
    // Reviewer) must not be able to use them to plant persistent content.
    const WRITE_TOOLS: &[&str] = &[
        "write_file",
        "edit_file",
        "run_command",
        "create_directory",
        "remember",
        "update_tasks",
    ];

    let mut results = format!("{TOOL_RESULTS_PREFIX}\n\n");
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
        // (file contents in any language, subprocess stdout, etc.). The cap
        // matches `read_file`'s own cap so a file the model just read is fed
        // back whole, not re-truncated to a smaller window.
        let output_str = if result.output.len() > crate::agent::tools::fs::MAX_TOOL_OUTPUT_CHARS {
            let head: String = result
                .output
                .chars()
                .take(crate::agent::tools::fs::MAX_TOOL_OUTPUT_CHARS)
                .collect();
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

    // Rebuild the outgoing messages via the shared helper so a regenerated
    // response carries the SAME context as the original send — mode/pipeline
    // system prompt, KB results, auto-context, and @file expansion (on the
    // newest turn only) — and records last_sent_tokens for accurate usage.
    // Previously this duplicated only compaction + @file (and re-expanded every
    // historical @file), silently dropping the persona and knowledge base.
    let search_query = app
        .current_conversation()
        .messages
        .iter()
        .rev()
        .find(|(role, _)| role == "user")
        .map(|(_, c)| c.clone())
        .unwrap_or_default();
    let messages = build_context_messages(app, &search_query);

    let model = route_turn_model(app, &search_query);
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

#[cfg(test)]
mod tests {
    use super::{build_context_messages, conversation_record, looks_like_slash_command};
    use crate::app::App;
    use crate::workspace::{ProjectType, WorkspaceInfo};

    #[test]
    fn conversation_record_skips_system_only() {
        // A fresh app whose conversation holds only a seeded system prompt must
        // NOT be persisted (nothing to lose yet).
        let mut app = App::new();
        app.current_conversation_mut()
            .messages
            .push(("system".into(), "you are nerve".into()));
        assert!(conversation_record(&app).is_none());
    }

    #[test]
    fn conversation_record_saves_after_user_turn() {
        // As soon as the user has spoken, the turn is durable — even before any
        // assistant response exists (the mid-stream-crash case).
        let mut app = App::new();
        app.add_user_message("fix the bug".into());
        let record = conversation_record(&app).expect("user turn should persist");
        assert!(
            record
                .messages
                .iter()
                .any(|m| m.role == "user" && m.content == "fix the bug")
        );
    }

    fn app_with_seeded_memory(root: &std::path::Path) -> App {
        let store = crate::project::ProjectStore::for_workspace(root);
        store
            .save_brief("# Payments\n\nA billing microservice in Rust.")
            .unwrap();
        store
            .remember("the database connection pool size is capped at 17 for this service")
            .unwrap();
        store
            .remember("the frontend is built with svelte and vite")
            .unwrap();
        store
            .record_decision("chose postgres over mysql for jsonb support")
            .unwrap();

        let mut app = App::new();
        app.cached_workspace = Some(WorkspaceInfo {
            root: root.to_path_buf(),
            project_type: ProjectType::Rust,
            name: "payments".into(),
            description: String::new(),
            key_files: vec![],
            tech_stack: vec![],
        });
        app
    }

    #[test]
    fn memory_is_retrieved_not_dumped_into_context() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_seeded_memory(dir.path());

        // A turn about the connection pool surfaces the matching fact via
        // recall (LAST section per project_context::build's documented
        // order), plus the always-on header.
        let messages =
            build_context_messages(&mut app, "what is the database connection pool size");
        let systems: String = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        // Always-on header: project headline + pointer to recall.
        assert!(systems.contains("A billing microservice in Rust"));
        assert!(systems.contains("recall"));
        // Auto-recall surfaced the relevant fact...
        assert!(systems.contains("capped at 17"));
        // `project_context::build` now pushes the FULL memory.md unconditionally
        // (matching the worker's model — the point of this unification), so the
        // unrelated fact is ALSO present, not just the recalled one. This is the
        // intentional, spec-called-out behaviour change from pull-only to push.
        assert!(systems.contains("svelte and vite"));
    }

    #[test]
    fn unrelated_turn_injects_no_facts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_seeded_memory(dir.path());

        let messages = build_context_messages(&mut app, "quantum blockchain cryptography theory");
        let systems: String = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        // Header still present.
        assert!(systems.contains("recall"));
        // `project_context::build` pushes memory.md unconditionally (see
        // module docs — this is the deliberate convergence with the worker's
        // behaviour), so BOTH stored facts are present even for a completely
        // unrelated query. Recall no longer gates whether a fact is injected
        // at all, only whether it additionally surfaces in the "Relevant
        // project memory" section.
        assert!(systems.contains("capped at 17"));
        assert!(systems.contains("svelte and vite"));
        assert!(systems.contains("postgres over mysql"));
    }

    #[test]
    fn design_principles_injected_only_on_design_turns() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::project::ProjectStore::for_workspace(dir.path());
        store
            .append_design("use a strict 8px spacing scale, never arbitrary margins")
            .unwrap();
        let mut app = app_with_seeded_memory(dir.path());

        // A design-related request injects the principles.
        let messages = build_context_messages(&mut app, "redesign the landing page header");
        let systems: String = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        // Heading text comes from `project_context::build`, not the old
        // hand-written "Project design principles" string.
        assert!(systems.contains("Design principles"));
        assert!(systems.contains("strict 8px spacing scale"));

        // A backend request does NOT — no wasted tokens.
        let messages = build_context_messages(&mut app, "fix the booking engine bug");
        let systems: String = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!systems.contains("Design principles"));
        assert!(!systems.contains("8px spacing scale"));
    }

    /// THE ANTI-DRIFT TEST (TUI side) -- the counterpart to
    /// `project_context::tests::both_callers_use_the_shared_builder`. Proves
    /// the TUI path also routes through `project_context::build` rather than
    /// assembling its own copy of these sections (which is exactly how the
    /// worker and the TUI silently diverged before -- see
    /// `project_context`'s module docs). There is no independent code path
    /// left in `build_context_messages` that assembles brief/memory/design/
    /// decisions/recall itself; it only formats whatever
    /// `project_context::build` returns (see the block right above the KB
    /// search in this function) -- so asserting its output contains every
    /// heading/fragment `build` itself produces for the same store+opts is
    /// the practical stand-in for a structural "same function" check.
    #[test]
    fn tui_path_uses_the_shared_builder() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_seeded_memory(dir.path());

        let store = crate::project::ProjectStore::for_workspace(dir.path());
        let opts = crate::project_context::ContextOptions {
            recall_query: Some("task text"),
            include_design: crate::memory_recall::is_design_request("task text"),
        };
        let expected_sections = crate::project_context::build(&store, &opts);
        assert!(
            !expected_sections.is_empty(),
            "seeded store should produce sections"
        );

        let messages = build_context_messages(&mut app, "task text");
        let systems: String = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        for section in &expected_sections {
            let heading = section.lines().next().unwrap_or(section);
            assert!(
                systems.contains(heading),
                "TUI system messages missing heading {heading:?} from shared builder output"
            );
        }
        assert!(systems.contains("billing microservice"));
        assert!(systems.contains("capped at 17"));
        assert!(systems.contains("postgres over mysql"));
    }

    #[test]
    fn detects_slash_command_attempts() {
        // Real command shapes (should be caught as command attempts).
        assert!(looks_like_slash_command("/help"));
        assert!(looks_like_slash_command("/hepl typo here"));
        assert!(looks_like_slash_command("/model gpt-4o"));
    }

    #[test]
    fn ignores_paths_and_plain_text() {
        // Messages that merely start with a path must NOT be treated as commands.
        assert!(!looks_like_slash_command("/usr/bin/env is the path"));
        assert!(!looks_like_slash_command("/home/me/file.rs please review"));
        // Not slash-prefixed, or too short.
        assert!(!looks_like_slash_command("just a normal message"));
        assert!(!looks_like_slash_command("/"));
        assert!(!looks_like_slash_command(""));
    }
}
