use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::ai::provider::StreamEvent;

// ─── App mode ────────────────────────────────────────────────────────────────

/// Top-level mode the application can be in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    /// Main chat view.
    Normal,
    /// Quick command / prompt input — the "Nerve Bar" (Ctrl+K).
    CommandBar,
    /// Browsing SmartPrompts full-screen.
    PromptPicker,
    /// Help overlay.
    Help,
    /// Picking an AI model.
    ModelSelect,
    /// Settings view.
    Settings,
    /// Clipboard manager overlay.
    ClipboardManager,
    /// Conversation history browser.
    HistoryBrowser,
    /// Picking an AI provider.
    ProviderSelect,
    /// In-conversation search overlay (Ctrl+F).
    SearchOverlay,
}

/// Whether the user is navigating (Normal / vim-style) or typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

/// Behavioural mode that adjusts system prompts, context limits, and AI style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NerveMode {
    /// Default behaviour — no special adjustments.
    Standard,
    /// Token-saving: concise prompts, auto-compact, smaller context window.
    Efficient,
    /// Full context, detailed responses, no compaction shortcuts.
    Thorough,
    /// Agent mode with tool access for autonomous coding.
    Agent,
    /// Explanations optimised for learning — analogies, exercises, checks.
    Learning,
}

impl NerveMode {
    /// Return a mode-specific system prompt that shapes the AI's behaviour.
    /// Returns `None` for Standard mode (no extra instructions).
    pub fn system_prompt(&self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::Efficient => Some(
                "You are in Efficient mode. Be extremely concise. \
                 Use short sentences. Omit pleasantries. Skip explanations \
                 unless asked. Prefer code over prose. Use bullet points. \
                 Never repeat the question back.",
            ),
            Self::Thorough => Some(
                "You are in Thorough mode. Provide detailed, comprehensive responses. \
                 Explain your reasoning step by step. Consider edge cases. \
                 Show alternative approaches when relevant. Include examples. \
                 Cite file paths and line numbers when discussing code.",
            ),
            Self::Agent => Some(
                "You are in Agent mode with tool access. Follow this workflow: \
                 1) UNDERSTAND the request fully before acting. \
                 2) PLAN your approach — list the steps. \
                 3) IMPLEMENT using the available tools (read, write, edit, run). \
                 4) VERIFY your changes compile and work. \
                 5) REPORT what you did and any issues found. \
                 Always read files before modifying them. Run tests after changes.",
            ),
            Self::Learning => Some(
                "You are in Learning mode. The user wants to understand, not just get answers. \
                 Explain concepts using analogies to things they already know. \
                 Ask Socratic questions to check understanding. \
                 Break complex topics into digestible pieces. \
                 Provide small exercises or challenges when appropriate. \
                 Use progressive disclosure — start simple, add detail on follow-up.",
            ),
        }
    }
}

// ─── Conversation ────────────────────────────────────────────────────────────

/// A single chat conversation.
pub struct Conversation {
    pub id: String,
    pub title: String,
    /// Each entry is `(role, content)` where role is `"user"` or `"assistant"`.
    pub messages: Vec<(String, String)>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Conversation {
    /// Create a brand-new, empty conversation.
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "New Conversation".into(),
            messages: Vec::new(),
            created_at: chrono::Utc::now(),
        }
    }
}

// ─── Branch ─────────────────────────────────────────────────────────────────

/// A saved conversation branch point
#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub messages: Vec<(String, String)>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ─── App ─────────────────────────────────────────────────────────────────────

/// Root application state.
pub struct App {
    // -- mode --
    pub mode: AppMode,
    pub input_mode: InputMode,

    // -- text input --
    pub input: String,
    pub cursor_position: usize,

    // -- conversations --
    pub conversations: Vec<Conversation>,
    pub active_conversation: usize,

    // -- streaming --
    pub streaming_response: String,
    pub is_streaming: bool,
    pub stream_rx: Option<mpsc::UnboundedReceiver<StreamEvent>>,
    pub streaming_start: Option<std::time::Instant>,
    /// Abort handle for the currently spawned provider task (or the
    /// currently running tool-execution task in agent mode). Calling
    /// `.abort()` drops the task's future, which — combined with
    /// `kill_on_drop(true)` on child processes — tears down the whole
    /// chain within milliseconds.
    pub stream_abort: Option<tokio::task::AbortHandle>,

    // -- viewport --
    pub scroll_offset: u16,
    /// Max scroll offset from the last chat render, used to clamp `scroll_up`
    /// so the offset can't run away past the top of the content. Interior
    /// mutability because the renderer only has `&App`.
    pub last_max_scroll: std::cell::Cell<u16>,
    /// Vertical scroll offset for the Help overlay (Ctrl+H / `?`).
    pub help_scroll: u16,
    /// Max help scroll offset from the last Help render, for clamping.
    pub help_max_scroll: std::cell::Cell<u16>,

    // -- model selection --
    pub selected_model: String,
    pub available_models: Vec<String>,
    pub model_select_index: usize,

    // -- prompt picker --
    pub prompt_filter: String,
    pub prompt_select_index: usize,
    pub prompt_category_index: usize,
    /// `true`  → focus is on the prompt list (right panel)
    /// `false` → focus is on the category list (left panel)
    pub prompt_focus_right: bool,

    // -- command bar --
    pub command_bar_input: String,
    pub command_bar_select_index: usize,
    /// Active category filter tab (0 = All, 1+ = specific category).
    pub command_bar_category: usize,

    // -- clipboard manager --
    pub clipboard_manager: crate::clipboard_manager::ClipboardManager,
    pub clipboard_search: String,
    pub clipboard_select_index: usize,

    // -- history browser --
    pub history_entries: Vec<crate::history::ConversationRecord>,
    pub history_select_index: usize,
    pub history_search: String,
    /// When `true`, a second press of `d` is required to confirm deletion.
    pub history_delete_pending: bool,
    /// Sort mode for history list: 0=date, 1=title, 2=message count.
    pub history_sort: usize,

    // -- provider selection --
    pub available_providers: Vec<String>,
    pub selected_provider: String,
    pub provider_select_index: usize,
    pub provider_changed: bool,

    // -- code mode --
    /// When `true`, the Claude Code provider has file & terminal access.
    pub code_mode: bool,
    /// Working directory for Claude Code file access.
    pub working_dir: Option<String>,

    // -- git config --
    /// Git commit author name (from config). Used with `--author` flag.
    pub git_user_name: String,
    /// Git commit author email (from config). Used with `--author` flag.
    pub git_user_email: String,

    // -- smart mode --
    /// Active behavioural mode (standard, efficient, thorough, agent, learning).
    pub active_mode: NerveMode,
    /// Display name of the current mode (for badge rendering).
    pub mode_name: String,

    // -- agent mode --
    /// When `true`, the AI can use tools (read/write files, run commands, etc.)
    /// and will loop until no more tool calls remain.
    pub agent_mode: bool,
    /// Number of tool-call iterations in the current agent loop (capped at 10).
    pub agent_iterations: usize,
    /// Currently executing tool name (shown in UI during agent execution).
    pub active_tool: Option<String>,
    /// Whether a git stash checkpoint was created when agent mode was enabled.
    pub agent_has_stash: bool,
    /// When `true`, automatically enable agent mode for messages that appear
    /// to need tool access.  Mirrors `config.auto_agent`.
    pub auto_agent: bool,
    /// Set to `true` when agent mode was activated by auto-detection (not by
    /// the user).  Used to disable agent mode again after the response if no
    /// tools were actually invoked.
    pub auto_agent_active: bool,
    /// When `true`, `/workflow` skips the plan-approval gate and executes
    /// the plan immediately. Mirrors `config.workflow_auto_approve`.
    pub workflow_auto_approve: bool,

    /// When `true` (default), each turn is routed to a model tier relative to
    /// `selected_model` (strong for planning/review/hard work, small for
    /// trivial). Mirrors `config.auto_model_routing`.
    pub auto_model_routing: bool,

    /// When `true` (default), after an agent turn that edited files Nerve runs
    /// `verify_command` and feeds failures back for the agent to fix.
    pub auto_verify: bool,
    /// The resolved verify command (config override or auto-detected). `None`
    /// when no command could be inferred.
    pub verify_command: Option<String>,
    /// Whether the current agent turn has edited files (so a read-only turn is
    /// not verified). Reset when a new user turn starts.
    pub agent_made_edits: bool,
    /// Verify→fix rounds used this turn, capped at `verify::MAX_VERIFY_ROUNDS`.
    pub verify_rounds: u8,
    /// Paths written/edited this turn — used to auto design-check UI files
    /// against the project's design principles. Reset when a new turn starts.
    pub turn_edited_files: Vec<String>,
    /// The model chosen for the in-flight turn by routing, remembered so that
    /// agent tool-round continuations — which no longer carry the original
    /// request text to classify — reuse the same model instead of re-deciding.
    pub active_turn_model: Option<String>,

    /// User-configured context window override (from `config.context_limit`).
    pub context_limit_override: Option<usize>,

    /// Active multi-agent workflow, if any. When `Some`, the event loop's
    /// Done handler advances through planner → coder → reviewer instead of
    /// terminating the stream.
    pub pipeline: Option<crate::agent::pipeline::PipelineState>,

    // -- search overlay --
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub search_current: usize,

    // -- branches --
    pub branches: Vec<Branch>,

    // -- context tracking --
    /// Running count of estimated tokens in the current conversation.
    pub total_tokens_used: usize,
    /// Estimated tokens in the payload ACTUALLY sent on the last request
    /// (post-expansion, post-compaction, including injected system prompts).
    /// Set by `build_context_messages`; used for usage accounting so recorded
    /// tokens reflect what was sent, not the raw stored conversation.
    pub last_sent_tokens: usize,
    /// Whether the last built payload had older turns summarized by compaction.
    /// Tracked so the user is notified once when compaction begins, not on
    /// every subsequent send.
    pub context_compacting: bool,

    // -- plugins --
    pub plugins: Vec<crate::plugins::Plugin>,

    // -- usage tracking --
    pub usage_stats: crate::usage::UsageStats,
    pub spending_limit: crate::usage::SpendingLimit,

    // -- settings overlay --
    pub settings_tab: usize,
    pub settings_select: usize,
    pub theme_index: usize,

    // -- workspace --
    /// Human-readable summary of the detected project workspace.
    pub detected_workspace: Option<String>,
    /// Full cached workspace info (avoids re-scanning the filesystem).
    pub cached_workspace: Option<crate::workspace::WorkspaceInfo>,

    // -- animation --
    /// Frame counter for animated UI elements (spinners, pulsing indicators).
    /// Incremented every draw cycle; wraps on overflow.
    pub thinking_frame: usize,

    // -- input history --
    pub input_history: Vec<String>,
    pub input_history_index: Option<usize>, // None = current input, Some(n) = nth previous
    pub input_saved: String,                // Saves current input when browsing history

    // -- aliases --
    pub aliases: HashMap<String, String>,

    // -- autocomplete --
    /// Autocomplete suggestions currently visible.
    pub autocomplete_items: Vec<String>,
    /// Currently selected index in the autocomplete popup.
    pub autocomplete_index: usize,
    /// Whether the autocomplete popup is visible.
    pub autocomplete_visible: bool,

    // -- pending command --
    /// A slash command queued for execution (e.g. selected from Nerve Bar).
    /// Checked and drained at the top of the event loop.
    pub pending_command: Option<String>,

    // -- shell --
    /// Timeout in seconds for shell commands (0 = no timeout).
    pub command_timeout_secs: u64,

    // -- misc --
    pub status_message: Option<String>,
    /// Timestamp of when `status_message` was last set (for auto-clear).
    pub status_time: Option<std::time::Instant>,
    pub should_quit: bool,
}

impl App {
    // ── Constructor ──────────────────────────────────────────────────────

    /// Create a new `App` with sensible defaults and one empty conversation.
    pub fn new() -> Self {
        let first = Conversation::new();
        Self {
            mode: AppMode::Normal,
            input_mode: InputMode::Insert,

            input: String::new(),
            cursor_position: 0,

            conversations: vec![first],
            active_conversation: 0,

            streaming_response: String::new(),
            is_streaming: false,
            stream_rx: None,
            streaming_start: None,
            stream_abort: None,

            scroll_offset: 0,
            last_max_scroll: std::cell::Cell::new(0),
            help_scroll: 0,
            help_max_scroll: std::cell::Cell::new(0),

            selected_model: "sonnet".into(),
            available_models: vec![
                "opus".into(),
                "sonnet".into(),
                "haiku".into(),
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "llama3".into(),
            ],
            model_select_index: 0,

            prompt_filter: String::new(),
            prompt_select_index: 0,
            prompt_category_index: 0,
            prompt_focus_right: false,

            command_bar_input: String::new(),
            command_bar_select_index: 0,
            command_bar_category: 0,

            clipboard_manager: crate::clipboard_manager::ClipboardManager::load()
                .unwrap_or_else(|_| crate::clipboard_manager::ClipboardManager::new(100)),
            clipboard_search: String::new(),
            clipboard_select_index: 0,

            history_entries: Vec::new(),
            history_select_index: 0,
            history_search: String::new(),
            history_delete_pending: false,
            history_sort: 0,

            available_providers: vec![
                "claude_code".into(),
                "ollama".into(),
                "openai".into(),
                "openrouter".into(),
                "copilot".into(),
            ],
            selected_provider: "claude_code".into(),
            provider_select_index: 0,
            provider_changed: false,

            code_mode: false,
            working_dir: None,

            git_user_name: String::new(),
            git_user_email: String::new(),

            active_mode: NerveMode::Standard,
            mode_name: "standard".into(),

            agent_mode: false,
            agent_iterations: 0,
            active_tool: None,
            agent_has_stash: false,
            auto_agent: true,
            workflow_auto_approve: false,
            auto_model_routing: true,
            active_turn_model: None,
            auto_verify: true,
            verify_command: None,
            agent_made_edits: false,
            verify_rounds: 0,
            turn_edited_files: Vec::new(),
            auto_agent_active: false,
            context_limit_override: None,
            pipeline: None,

            search_query: String::new(),
            search_results: Vec::new(),
            search_current: 0,

            branches: Vec::new(),

            total_tokens_used: 0,
            last_sent_tokens: 0,
            context_compacting: false,

            plugins: Vec::new(),

            usage_stats: crate::usage::UsageStats::new(),
            spending_limit: crate::usage::SpendingLimit::default(),

            settings_tab: 0,
            settings_select: 0,
            theme_index: 0,

            detected_workspace: None,
            cached_workspace: None,

            thinking_frame: 0,

            input_history: Vec::new(),
            input_history_index: None,
            input_saved: String::new(),

            aliases: HashMap::new(),

            autocomplete_items: Vec::new(),
            autocomplete_index: 0,
            autocomplete_visible: false,

            pending_command: None,

            command_timeout_secs: crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS,

            status_message: None,
            status_time: None,
            should_quit: false,
        }
    }

    // ── Conversation accessors ──────────────────────────────────────────

    /// Reference to the currently active conversation.
    ///
    /// Clamps `active_conversation` to a valid index so we never panic even if
    /// conversations were deleted without updating the index.
    ///
    /// # Panics
    ///
    /// Panics (debug builds only) if `conversations` is empty — callers must
    /// maintain the invariant that at least one conversation always exists.
    pub fn current_conversation(&self) -> &Conversation {
        debug_assert!(
            !self.conversations.is_empty(),
            "App must always have at least one conversation"
        );
        let idx = self
            .active_conversation
            .min(self.conversations.len().saturating_sub(1));
        &self.conversations[idx]
    }

    /// Mutable reference to the currently active conversation.
    ///
    /// Clamps `active_conversation` to a valid index so we never panic even if
    /// conversations were deleted without updating the index.
    ///
    /// # Panics
    ///
    /// Panics (debug builds only) if `conversations` is empty.
    pub fn current_conversation_mut(&mut self) -> &mut Conversation {
        debug_assert!(
            !self.conversations.is_empty(),
            "App must always have at least one conversation"
        );
        let idx = self
            .active_conversation
            .min(self.conversations.len().saturating_sub(1));
        &mut self.conversations[idx]
    }

    /// Create a new empty conversation and switch to it.
    pub fn new_conversation(&mut self) {
        self.cancel_active_stream();
        self.conversations.push(Conversation::new());
        self.active_conversation = self.conversations.len() - 1;
        self.scroll_offset = 0;
        self.streaming_response.clear();
        self.is_streaming = false;
        self.stream_rx = None;
        self.streaming_start = None;
        self.agent_iterations = 0;
        self.auto_agent_active = false;
        // A new conversation abandons any in-progress multi-agent
        // workflow; the next message should start clean.
        self.pipeline = None;
        self.status_message = Some("New conversation started".into());
    }

    /// Abort the currently running provider/tool task (if any) and clear
    /// its abort handle. Safe to call when nothing is streaming.
    ///
    /// The spawned task's future is dropped, which drops any in-flight
    /// `tokio::process::Child` and HTTP stream. Combined with
    /// `kill_on_drop(true)` on the child, this SIGKILLs the subprocess
    /// immediately — no need to wait for the next send-error to bail out.
    pub fn cancel_active_stream(&mut self) {
        if let Some(handle) = self.stream_abort.take() {
            handle.abort();
        }
    }

    /// Revert an automatic (intent-detected) agent-mode activation once the
    /// whole request — including any tool loop — has finished, so the activation
    /// and its injected system prompts don't leak into the next message.
    ///
    /// Uses the persistent `auto_agent_active` flag (which now survives the tool
    /// loop) and only fires when no tool iterations are outstanding
    /// (`agent_iterations == 0`), i.e. the request is truly done. Called from
    /// every turn-terminating path — normal completion, Esc-cancel, and stream
    /// error. Strips ALL system context injected on activation: the tools/Nerve
    /// prompts AND the "Current project context:" map (which previously lingered
    /// and accumulated across activations).
    pub fn revert_auto_agent_activation(&mut self) {
        if self.auto_agent_active && self.agent_iterations == 0 {
            self.auto_agent_active = false;
            self.agent_mode = false;
            self.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("You have access to the following tools")
                        || c.contains("You are Nerve, an AI coding assistant")
                        || c.starts_with("Current project context:")))
            });
        }
    }

    // ── Status ──────────────────────────────────────────────────────────

    /// Set a status message with an auto-clear timestamp.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
        self.status_time = Some(std::time::Instant::now());
    }

    /// Set an error status message (`"Error: {e}"`).
    pub fn report_error(&mut self, e: impl std::fmt::Display) {
        self.set_status(format!("Error: {e}"));
    }

    // ── Messages ────────────────────────────────────────────────────────

    /// Append a user message to the active conversation.
    pub fn add_user_message(&mut self, content: String) {
        self.current_conversation_mut()
            .messages
            .push(("user".into(), content));
    }

    /// Append a complete assistant message to the active conversation.
    pub fn add_assistant_message(&mut self, content: String) {
        self.current_conversation_mut()
            .messages
            .push(("assistant".into(), content));
    }

    // ── Streaming ───────────────────────────────────────────────────────

    /// Append a token fragment to the in-progress streaming response.
    pub fn append_to_streaming(&mut self, token: &str) {
        self.streaming_response.push_str(token);
    }

    /// Finalise the streaming response: move it into the conversation history
    /// and reset the streaming state.
    pub fn finish_streaming(&mut self) {
        self.cancel_active_stream();
        if !self.streaming_response.is_empty() {
            let content = std::mem::take(&mut self.streaming_response);
            self.add_assistant_message(content);
        }
        self.is_streaming = false;
        self.stream_rx = None;
        self.streaming_start = None;
        // NOTE: auto_agent_active is deliberately NOT cleared here. It must
        // survive the agent tool loop (finish_streaming runs on every Done,
        // including mid-loop) so the activation can be reverted only once the
        // whole request completes. revert_auto_agent_activation() is the sole
        // consumer/clearer; new_conversation() also resets it.
    }

    // ── Cursor / editing ────────────────────────────────────────────────

    pub fn move_cursor_left(&mut self) {
        // Clamp to valid range before indexing to prevent panics.
        let pos = self.cursor_position.min(self.input.len());
        if pos > 0 {
            // Respect grapheme boundaries by finding the previous char boundary.
            let new_pos = self.input[..pos]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.cursor_position = new_pos;
        } else {
            // cursor_position was beyond input length or at 0; clamp it.
            self.cursor_position = pos;
        }
    }

    pub fn move_cursor_right(&mut self) {
        // Clamp to valid range before indexing to prevent panics.
        let pos = self.cursor_position.min(self.input.len());
        if pos < self.input.len() {
            let new_pos = self.input[pos..]
                .char_indices()
                .nth(1)
                .map(|(idx, _)| pos + idx)
                .unwrap_or(self.input.len());
            self.cursor_position = new_pos;
        } else {
            // cursor_position was beyond input length; clamp it.
            self.cursor_position = pos;
        }
    }

    /// Insert a character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        // Clamp to valid range before indexing to prevent panics.
        let pos = self.cursor_position.min(self.input.len());
        self.input.insert(pos, c);
        self.cursor_position = pos + c.len_utf8();
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char(&mut self) {
        // Clamp to valid range before indexing to prevent panics.
        let pos = self.cursor_position.min(self.input.len());
        if pos > 0 {
            let prev = self.input[..pos]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.input.drain(prev..pos);
            self.cursor_position = prev;
        }
    }

    /// Take the current input text, clear the input buffer, and return it.
    /// Returns `None` if the input was empty (whitespace-only).
    /// Non-empty submissions are saved to input history for arrow-key recall.
    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return None;
        }
        // Save to input history (avoid duplicates of the last entry)
        if self.input_history.last().map(std::string::String::as_str) != Some(&text) {
            self.input_history.push(text.clone());
        }
        self.input.clear();
        self.cursor_position = 0;
        self.input_history_index = None;
        Some(text)
    }

    // ── Scrolling ───────────────────────────────────────────────────────

    pub fn scroll_up(&mut self) {
        // Clamp to the last rendered maximum so the offset can't run away
        // past the top (which would otherwise make scroll_down feel "stuck"
        // for several presses and show a nonsense ↑N counter).
        let max = self.last_max_scroll.get();
        self.scroll_offset = self.scroll_offset.saturating_add(3).min(max);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
    }

    /// Jump to the latest message (bottom of conversation).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Jump to the oldest message (top of conversation).
    pub fn scroll_to_top(&mut self) {
        // Clamp to the last rendered maximum (set during chat render).
        self.scroll_offset = self.last_max_scroll.get();
    }

    // ── Branching ───────────────────────────────────────────────────────

    /// Create a branch from the current conversation state
    pub fn create_branch(&mut self, name: String) {
        let messages = self.current_conversation().messages.clone();
        self.branches.push(Branch {
            name,
            messages,
            created_at: chrono::Utc::now(),
        });
    }

    /// Restore a branch (replace current conversation messages)
    pub fn restore_branch(&mut self, index: usize) {
        if let Some(branch) = self.branches.get(index) {
            let messages = branch.messages.clone();
            self.current_conversation_mut().messages = messages;
            self.scroll_offset = 0;
        }
    }

    /// Delete a branch
    pub fn delete_branch(&mut self, index: usize) {
        if index < self.branches.len() {
            self.branches.remove(index);
        }
    }
}

/// Given the text to the left of the cursor, return the byte offset where a
/// word-delete (Ctrl+W) should leave the cursor: the start of the last
/// whitespace-delimited word, skipping trailing whitespace.
///
/// The returned offset is always a valid `char` boundary, so callers can
/// safely `drain(offset..cursor)` even when the input contains multi-byte
/// whitespace (e.g. U+00A0 no-break space, U+3000 ideographic space).
pub fn word_delete_start(before_cursor: &str) -> usize {
    let trimmed = before_cursor.trim_end();
    match trimmed.rfind(char::is_whitespace) {
        // Advance past the matched whitespace char, which may be multi-byte.
        Some(i) => i + trimmed[i..].chars().next().map_or(1, char::len_utf8),
        None => 0,
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
