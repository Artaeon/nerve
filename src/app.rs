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

    // -- viewport --
    pub scroll_offset: u16,

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
    /// Whether a git stash checkpoint was created when agent mode was enabled.
    pub agent_has_stash: bool,
    /// When `true`, automatically enable agent mode for messages that appear
    /// to need tool access.  Mirrors `config.auto_agent`.
    pub auto_agent: bool,
    /// Set to `true` when agent mode was activated by auto-detection (not by
    /// the user).  Used to disable agent mode again after the response if no
    /// tools were actually invoked.
    pub auto_agent_active: bool,

    // -- search overlay --
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub search_current: usize,

    // -- branches --
    pub branches: Vec<Branch>,

    // -- context tracking --
    /// Running count of estimated tokens in the current conversation.
    pub total_tokens_used: usize,

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

            scroll_offset: 0,

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

            active_mode: NerveMode::Standard,
            mode_name: "standard".into(),

            agent_mode: false,
            agent_iterations: 0,
            agent_has_stash: false,
            auto_agent: true,
            auto_agent_active: false,

            search_query: String::new(),
            search_results: Vec::new(),
            search_current: 0,

            branches: Vec::new(),

            total_tokens_used: 0,

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
    pub fn current_conversation(&self) -> &Conversation {
        let idx = self
            .active_conversation
            .min(self.conversations.len().saturating_sub(1));
        &self.conversations[idx]
    }

    /// Mutable reference to the currently active conversation.
    ///
    /// Clamps `active_conversation` to a valid index so we never panic even if
    /// conversations were deleted without updating the index.
    pub fn current_conversation_mut(&mut self) -> &mut Conversation {
        let idx = self
            .active_conversation
            .min(self.conversations.len().saturating_sub(1));
        &mut self.conversations[idx]
    }

    /// Create a new empty conversation and switch to it.
    pub fn new_conversation(&mut self) {
        self.conversations.push(Conversation::new());
        self.active_conversation = self.conversations.len() - 1;
        self.scroll_offset = 0;
        self.streaming_response.clear();
        self.is_streaming = false;
        self.agent_iterations = 0; // Reset agent iterations
        self.status_message = Some("New conversation started".into());
    }

    // ── Status ──────────────────────────────────────────────────────────

    /// Set a status message with an auto-clear timestamp.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
        self.status_time = Some(std::time::Instant::now());
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
        if !self.streaming_response.is_empty() {
            let content = std::mem::take(&mut self.streaming_response);
            self.add_assistant_message(content);
        }
        self.is_streaming = false;
        self.stream_rx = None;
        self.streaming_start = None;
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
        if self.input_history.last().map(|s| s.as_str()) != Some(&text) {
            self.input_history.push(text.clone());
        }
        self.input.clear();
        self.cursor_position = 0;
        self.input_history_index = None;
        Some(text)
    }

    // ── Scrolling ───────────────────────────────────────────────────────

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── App::new() defaults ─────────────────────────────────────────────

    #[test]
    fn new_app_defaults() {
        let app = App::new();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.input_mode, InputMode::Insert);
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
        assert_eq!(app.conversations.len(), 1);
        assert_eq!(app.active_conversation, 0);
        assert!(!app.is_streaming);
        assert!(!app.should_quit);
        assert!(app.streaming_response.is_empty());
        assert!(app.stream_rx.is_none());
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.selected_model, "sonnet");
        assert!(!app.available_models.is_empty());
        assert_eq!(app.model_select_index, 0);
        assert!(app.status_message.is_none());
        assert!(!app.code_mode);
        assert!(app.working_dir.is_none());
        assert!(!app.agent_mode);
        assert_eq!(app.agent_iterations, 0);
        assert_eq!(app.selected_provider, "claude_code");
        assert!(!app.provider_changed);
    }

    #[test]
    fn new_app_has_expected_models() {
        let app = App::new();
        assert!(app.available_models.contains(&"opus".to_string()));
        assert!(app.available_models.contains(&"sonnet".to_string()));
        assert!(app.available_models.contains(&"haiku".to_string()));
        assert!(app.available_models.contains(&"gpt-4o".to_string()));
    }

    #[test]
    fn new_app_has_expected_providers() {
        let app = App::new();
        assert!(app.available_providers.contains(&"claude_code".to_string()));
        assert!(app.available_providers.contains(&"ollama".to_string()));
        assert!(app.available_providers.contains(&"openai".to_string()));
        assert!(app.available_providers.contains(&"openrouter".to_string()));
        assert!(app.available_providers.contains(&"copilot".to_string()));
    }

    // ── Conversation::new() ─────────────────────────────────────────────

    #[test]
    fn conversation_new_has_uuid() {
        let conv = Conversation::new();
        // UUID v4 has 36 characters with hyphens.
        assert_eq!(conv.id.len(), 36);
        assert!(uuid::Uuid::parse_str(&conv.id).is_ok());
    }

    #[test]
    fn conversation_new_has_default_title() {
        let conv = Conversation::new();
        assert_eq!(conv.title, "New Conversation");
    }

    #[test]
    fn conversation_new_has_empty_messages() {
        let conv = Conversation::new();
        assert!(conv.messages.is_empty());
    }

    #[test]
    fn conversation_new_has_recent_timestamp() {
        let before = chrono::Utc::now();
        let conv = Conversation::new();
        let after = chrono::Utc::now();
        assert!(conv.created_at >= before);
        assert!(conv.created_at <= after);
    }

    #[test]
    fn two_conversations_have_different_ids() {
        let a = Conversation::new();
        let b = Conversation::new();
        assert_ne!(a.id, b.id);
    }

    // ── current_conversation() ──────────────────────────────────────────

    #[test]
    fn current_conversation_returns_first_by_default() {
        let app = App::new();
        let conv = app.current_conversation();
        assert_eq!(conv.title, "New Conversation");
        assert!(conv.messages.is_empty());
    }

    #[test]
    fn current_conversation_tracks_active_index() {
        let mut app = App::new();
        app.new_conversation();
        // active_conversation should now point to the second one.
        assert_eq!(app.active_conversation, 1);
        let id = app.current_conversation().id.clone();
        assert_eq!(app.conversations[1].id, id);
    }

    // ── new_conversation() ──────────────────────────────────────────────

    #[test]
    fn new_conversation_increases_count() {
        let mut app = App::new();
        assert_eq!(app.conversations.len(), 1);
        app.new_conversation();
        assert_eq!(app.conversations.len(), 2);
        app.new_conversation();
        assert_eq!(app.conversations.len(), 3);
    }

    #[test]
    fn new_conversation_switches_active() {
        let mut app = App::new();
        let first_id = app.current_conversation().id.clone();
        app.new_conversation();
        let second_id = app.current_conversation().id.clone();
        assert_ne!(first_id, second_id);
        assert_eq!(app.active_conversation, 1);
    }

    #[test]
    fn new_conversation_resets_state() {
        let mut app = App::new();
        app.scroll_offset = 42;
        app.streaming_response = "partial".into();
        app.is_streaming = true;
        app.new_conversation();
        assert_eq!(app.scroll_offset, 0);
        assert!(app.streaming_response.is_empty());
        assert!(!app.is_streaming);
        assert_eq!(app.status_message, Some("New conversation started".into()));
    }

    // ── add_user_message() ──────────────────────────────────────────────

    #[test]
    fn add_user_message_appends() {
        let mut app = App::new();
        app.add_user_message("Hello".into());
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "user");
        assert_eq!(msgs[0].1, "Hello");
    }

    #[test]
    fn add_user_message_multiple() {
        let mut app = App::new();
        app.add_user_message("First".into());
        app.add_user_message("Second".into());
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].1, "First");
        assert_eq!(msgs[1].1, "Second");
    }

    // ── add_assistant_message() ─────────────────────────────────────────

    #[test]
    fn add_assistant_message_appends() {
        let mut app = App::new();
        app.add_assistant_message("Response".into());
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "assistant");
        assert_eq!(msgs[0].1, "Response");
    }

    #[test]
    fn mixed_messages_preserve_order() {
        let mut app = App::new();
        app.add_user_message("Q1".into());
        app.add_assistant_message("A1".into());
        app.add_user_message("Q2".into());
        app.add_assistant_message("A2".into());
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0], ("user".into(), "Q1".into()));
        assert_eq!(msgs[1], ("assistant".into(), "A1".into()));
        assert_eq!(msgs[2], ("user".into(), "Q2".into()));
        assert_eq!(msgs[3], ("assistant".into(), "A2".into()));
    }

    // ── append_to_streaming() ───────────────────────────────────────────

    #[test]
    fn append_to_streaming_accumulates() {
        let mut app = App::new();
        app.append_to_streaming("Hello");
        app.append_to_streaming(", ");
        app.append_to_streaming("world!");
        assert_eq!(app.streaming_response, "Hello, world!");
    }

    #[test]
    fn append_to_streaming_empty_string() {
        let mut app = App::new();
        app.append_to_streaming("");
        assert!(app.streaming_response.is_empty());
    }

    // ── finish_streaming() ──────────────────────────────────────────────

    #[test]
    fn finish_streaming_moves_content_to_conversation() {
        let mut app = App::new();
        app.is_streaming = true;
        app.streaming_response = "The answer is 42".into();
        app.finish_streaming();

        assert!(!app.is_streaming);
        assert!(app.streaming_response.is_empty());
        assert!(app.stream_rx.is_none());
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "assistant");
        assert_eq!(msgs[0].1, "The answer is 42");
    }

    #[test]
    fn finish_streaming_empty_does_not_add_message() {
        let mut app = App::new();
        app.is_streaming = true;
        app.streaming_response = String::new();
        app.finish_streaming();

        assert!(!app.is_streaming);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn finish_streaming_clears_stream_rx() {
        let mut app = App::new();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        app.stream_rx = Some(rx);
        app.is_streaming = true;
        app.streaming_response = "data".into();
        app.finish_streaming();

        assert!(app.stream_rx.is_none());
    }

    // ── move_cursor_left() ──────────────────────────────────────────────

    #[test]
    fn move_cursor_left_from_end() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 3;
        app.move_cursor_left();
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn move_cursor_left_at_zero_stays() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 0;
        app.move_cursor_left();
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn move_cursor_left_empty_string() {
        let mut app = App::new();
        app.input = String::new();
        app.cursor_position = 0;
        app.move_cursor_left();
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn move_cursor_left_multibyte() {
        let mut app = App::new();
        // 'e' with accent: 2 bytes in UTF-8
        app.input = "cafe\u{0301}".into(); // "café" as base + combining
        app.cursor_position = app.input.len();
        app.move_cursor_left();
        // Should move back by one char boundary (the combining accent).
        assert!(app.cursor_position < app.input.len());
    }

    #[test]
    fn move_cursor_left_with_emoji() {
        let mut app = App::new();
        app.input = "hi\u{1F600}".into(); // "hi😀"
        app.cursor_position = app.input.len(); // past the emoji
        app.move_cursor_left();
        // emoji is 4 bytes, so cursor should be at 2
        assert_eq!(app.cursor_position, 2);
    }

    // ── move_cursor_right() ─────────────────────────────────────────────

    #[test]
    fn move_cursor_right_from_start() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 0;
        app.move_cursor_right();
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn move_cursor_right_at_end_stays() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 3;
        app.move_cursor_right();
        assert_eq!(app.cursor_position, 3);
    }

    #[test]
    fn move_cursor_right_empty_string() {
        let mut app = App::new();
        app.input = String::new();
        app.cursor_position = 0;
        app.move_cursor_right();
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn move_cursor_right_with_emoji() {
        let mut app = App::new();
        app.input = "\u{1F600}hi".into(); // "😀hi"
        app.cursor_position = 0;
        app.move_cursor_right();
        // emoji is 4 bytes, should jump to position 4
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    fn move_cursor_left_right_roundtrip() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 3;
        app.move_cursor_left();
        app.move_cursor_right();
        assert_eq!(app.cursor_position, 3);
    }

    // ── insert_char() ───────────────────────────────────────────────────

    #[test]
    fn insert_char_at_start() {
        let mut app = App::new();
        app.input = "bc".into();
        app.cursor_position = 0;
        app.insert_char('a');
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn insert_char_at_middle() {
        let mut app = App::new();
        app.input = "ac".into();
        app.cursor_position = 1;
        app.insert_char('b');
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn insert_char_at_end() {
        let mut app = App::new();
        app.input = "ab".into();
        app.cursor_position = 2;
        app.insert_char('c');
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor_position, 3);
    }

    #[test]
    fn insert_char_into_empty() {
        let mut app = App::new();
        app.insert_char('x');
        assert_eq!(app.input, "x");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn insert_multibyte_char() {
        let mut app = App::new();
        app.input = "ab".into();
        app.cursor_position = 1;
        app.insert_char('\u{1F600}'); // 😀, 4 bytes
        assert_eq!(app.input, "a\u{1F600}b");
        assert_eq!(app.cursor_position, 5); // 1 + 4
    }

    #[test]
    fn insert_char_sequence() {
        let mut app = App::new();
        for c in "hello".chars() {
            app.insert_char(c);
        }
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 5);
    }

    // ── delete_char() ───────────────────────────────────────────────────

    #[test]
    fn delete_char_at_start_noop() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 0;
        app.delete_char();
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn delete_char_at_end() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 3;
        app.delete_char();
        assert_eq!(app.input, "ab");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn delete_char_in_middle() {
        let mut app = App::new();
        app.input = "abc".into();
        app.cursor_position = 2;
        app.delete_char();
        assert_eq!(app.input, "ac");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn delete_char_empty_string() {
        let mut app = App::new();
        app.cursor_position = 0;
        app.delete_char();
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn delete_multibyte_char() {
        let mut app = App::new();
        app.input = "a\u{1F600}b".into(); // "a😀b"
        app.cursor_position = 5; // after the emoji
        app.delete_char();
        assert_eq!(app.input, "ab");
        assert_eq!(app.cursor_position, 1);
    }

    #[test]
    fn delete_all_chars_one_by_one() {
        let mut app = App::new();
        app.input = "hi".into();
        app.cursor_position = 2;
        app.delete_char();
        app.delete_char();
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    // ── submit_input() ──────────────────────────────────────────────────

    #[test]
    fn submit_input_returns_trimmed_text() {
        let mut app = App::new();
        app.input = "  hello world  ".into();
        app.cursor_position = 15;
        let result = app.submit_input();
        assert_eq!(result, Some("hello world".into()));
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn submit_empty_returns_none() {
        let mut app = App::new();
        app.input = "   ".into();
        assert_eq!(app.submit_input(), None);
    }

    #[test]
    fn submit_completely_empty_returns_none() {
        let mut app = App::new();
        assert_eq!(app.submit_input(), None);
    }

    #[test]
    fn submit_input_clears_state() {
        let mut app = App::new();
        app.input = "test".into();
        app.cursor_position = 4;
        let _ = app.submit_input();
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn submit_preserves_inner_whitespace() {
        let mut app = App::new();
        app.input = "  hello   world  ".into();
        let result = app.submit_input();
        assert_eq!(result, Some("hello   world".into()));
    }

    // ── scroll_up / scroll_down ─────────────────────────────────────────

    #[test]
    fn scroll_up_increments_by_three() {
        let mut app = App::new();
        assert_eq!(app.scroll_offset, 0);
        app.scroll_up();
        assert_eq!(app.scroll_offset, 3);
        app.scroll_up();
        assert_eq!(app.scroll_offset, 6);
    }

    #[test]
    fn scroll_down_decrements_by_three() {
        let mut app = App::new();
        app.scroll_offset = 9;
        app.scroll_down();
        assert_eq!(app.scroll_offset, 6);
        app.scroll_down();
        assert_eq!(app.scroll_offset, 3);
    }

    #[test]
    fn scroll_down_saturates_at_zero() {
        let mut app = App::new();
        app.scroll_offset = 2;
        app.scroll_down();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn scroll_down_at_zero_stays() {
        let mut app = App::new();
        app.scroll_offset = 0;
        app.scroll_down();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn scroll_up_near_max_u16() {
        let mut app = App::new();
        app.scroll_offset = u16::MAX - 1;
        app.scroll_up();
        // saturating_add should clamp at u16::MAX
        assert_eq!(app.scroll_offset, u16::MAX);
    }

    // ── Messages go to the correct conversation ─────────────────────────

    #[test]
    fn messages_go_to_active_conversation_not_others() {
        let mut app = App::new();
        app.add_user_message("first conv msg".into());
        app.new_conversation();
        app.add_user_message("second conv msg".into());

        assert_eq!(app.conversations[0].messages.len(), 1);
        assert_eq!(app.conversations[0].messages[0].1, "first conv msg");
        assert_eq!(app.conversations[1].messages.len(), 1);
        assert_eq!(app.conversations[1].messages[0].1, "second conv msg");
    }

    // ── Multiple conversations: isolation ──────────────────────────────

    #[test]
    fn multiple_conversations_isolated() {
        let mut app = App::new();
        app.add_user_message("msg1".into());
        app.new_conversation();
        app.add_user_message("msg2".into());
        assert_eq!(app.conversations.len(), 2);
        assert_eq!(app.current_conversation().messages.len(), 1);
        assert_eq!(app.current_conversation().messages[0].1, "msg2");
        // Switch back
        app.active_conversation = 0;
        assert_eq!(app.current_conversation().messages.len(), 1);
        assert_eq!(app.current_conversation().messages[0].1, "msg1");
    }

    #[test]
    fn switch_between_three_conversations() {
        let mut app = App::new();
        app.add_user_message("conv0".into());
        app.new_conversation();
        app.add_user_message("conv1".into());
        app.new_conversation();
        app.add_user_message("conv2".into());
        assert_eq!(app.conversations.len(), 3);

        // Verify each conversation is independent
        app.active_conversation = 0;
        assert_eq!(app.current_conversation().messages[0].1, "conv0");
        app.active_conversation = 1;
        assert_eq!(app.current_conversation().messages[0].1, "conv1");
        app.active_conversation = 2;
        assert_eq!(app.current_conversation().messages[0].1, "conv2");
    }

    #[test]
    fn adding_message_to_switched_conversation() {
        let mut app = App::new();
        app.add_user_message("first".into());
        app.new_conversation();
        app.add_user_message("second".into());

        // Switch back and add to first conversation
        app.active_conversation = 0;
        app.add_assistant_message("reply to first".into());
        assert_eq!(app.conversations[0].messages.len(), 2);
        assert_eq!(app.conversations[0].messages[1].1, "reply to first");
        // Second conversation unaffected
        assert_eq!(app.conversations[1].messages.len(), 1);
    }

    // ── Cursor with emoji / multibyte ──────────────────────────────────

    #[test]
    fn cursor_handles_emoji_insert_delete() {
        let mut app = App::new();
        app.insert_char('\u{1F980}'); // 🦀 (crab emoji, 4 bytes)
        assert_eq!(app.input, "\u{1F980}");
        assert_eq!(app.cursor_position, 4);
        app.delete_char();
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn insert_accented_characters() {
        let mut app = App::new();
        app.insert_char('\u{00E9}'); // e with accent (2 bytes)
        assert_eq!(app.input, "\u{00E9}");
        assert_eq!(app.cursor_position, 2);
        app.insert_char('\u{00E8}'); // another accented e
        assert_eq!(app.input, "\u{00E9}\u{00E8}");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    fn delete_accented_char() {
        let mut app = App::new();
        app.input = "caf\u{00E9}".into(); // "cafe" with accent on e (2 bytes)
        app.cursor_position = app.input.len();
        app.delete_char();
        assert_eq!(app.input, "caf");
        assert_eq!(app.cursor_position, 3);
    }

    #[test]
    fn cursor_movement_across_emoji_boundary() {
        let mut app = App::new();
        app.input = "a\u{1F600}b".into(); // "a😀b"
        app.cursor_position = 0;

        app.move_cursor_right(); // past 'a'
        assert_eq!(app.cursor_position, 1);
        app.move_cursor_right(); // past emoji (4 bytes)
        assert_eq!(app.cursor_position, 5);
        app.move_cursor_right(); // past 'b'
        assert_eq!(app.cursor_position, 6);

        app.move_cursor_left(); // back before 'b'
        assert_eq!(app.cursor_position, 5);
        app.move_cursor_left(); // back before emoji
        assert_eq!(app.cursor_position, 1);
        app.move_cursor_left(); // back before 'a'
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn insert_emoji_in_middle_of_text() {
        let mut app = App::new();
        app.input = "ab".into();
        app.cursor_position = 1;
        app.insert_char('\u{2764}'); // ❤ (3 bytes)
        assert_eq!(app.input, "a\u{2764}b");
        assert_eq!(app.cursor_position, 4); // 1 + 3
    }

    // ── scroll_down at 0 ───────────────────────────────────────────────

    #[test]
    fn scroll_down_from_zero_repeated() {
        let mut app = App::new();
        app.scroll_down();
        app.scroll_down();
        app.scroll_down();
        assert_eq!(app.scroll_offset, 0);
    }

    // ── finish_streaming edge cases ────────────────────────────────────

    #[test]
    fn finish_streaming_when_not_streaming() {
        let mut app = App::new();
        app.is_streaming = false;
        app.streaming_response = String::new();
        app.finish_streaming();
        assert!(!app.is_streaming);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn finish_streaming_preserves_existing_messages() {
        let mut app = App::new();
        app.add_user_message("question".into());
        app.is_streaming = true;
        app.streaming_response = "answer".into();
        app.finish_streaming();
        let msgs = &app.current_conversation().messages;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0], ("user".into(), "question".into()));
        assert_eq!(msgs[1], ("assistant".into(), "answer".into()));
    }

    // ── submit_input with unicode ──────────────────────────────────────

    #[test]
    fn submit_input_with_unicode() {
        let mut app = App::new();
        app.input = "Hello \u{1F600} world".into();
        app.cursor_position = app.input.len();
        let result = app.submit_input();
        assert_eq!(result, Some("Hello \u{1F600} world".into()));
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
    }

    // ── active_conversation bounds after delete ────────────────────────

    #[test]
    fn active_conversation_bounds_after_delete() {
        let mut app = App::new();
        app.new_conversation();
        app.new_conversation();
        assert_eq!(app.conversations.len(), 3);
        assert_eq!(app.active_conversation, 2);

        // Delete the last conversation
        app.conversations.remove(2);
        // active_conversation is now out of bounds (pointing to index 2, but
        // only indices 0..1 exist). current_conversation() clamps internally,
        // so it must NOT panic.
        let _ = app.current_conversation();
        let _ = app.current_conversation_mut();
    }

    // ── new_conversation resets streaming ──────────────────────────────

    #[test]
    fn new_conversation_resets_streaming() {
        let mut app = App::new();
        app.is_streaming = true;
        app.streaming_response = "partial".into();
        app.new_conversation();
        // New conversation should not inherit streaming state
        assert!(app.streaming_response.is_empty());
        assert!(!app.is_streaming);
    }

    // ── insert_char at various positions ──────────────────────────────

    #[test]
    fn insert_char_at_various_positions() {
        let mut app = App::new();
        app.input = "hello".into();

        // Insert at beginning
        app.cursor_position = 0;
        app.insert_char('X');
        assert_eq!(app.input, "Xhello");

        // Insert at end
        app.cursor_position = 6;
        app.insert_char('Y');
        assert_eq!(app.input, "XhelloY");

        // Insert in middle
        app.cursor_position = 3;
        app.insert_char('Z');
        assert_eq!(app.input, "XheZlloY");
    }

    // ── scroll_offset large values ────────────────────────────────────

    #[test]
    fn scroll_offset_large_values() {
        let mut app = App::new();
        // Scroll up many times
        for _ in 0..1000 {
            app.scroll_up();
        }
        assert_eq!(app.scroll_offset, 3000); // 1000 * 3

        // Scroll down many times
        for _ in 0..1000 {
            app.scroll_down();
        }
        assert_eq!(app.scroll_offset, 0);
    }

    // ── current_conversation clamps when all but one deleted ──────────

    #[test]
    fn current_conversation_clamps_to_last_valid() {
        let mut app = App::new();
        app.new_conversation();
        app.new_conversation();
        // Point to the last (index 2)
        app.active_conversation = 2;
        // Remove conversations 1 and 2
        app.conversations.truncate(1);
        // active_conversation = 2 is way out of bounds, but the accessor
        // must clamp to 0.
        let conv = app.current_conversation();
        assert_eq!(conv.title, "New Conversation");
    }

    // ── delete_char at cursor_position=0 with content is no-op ───────

    #[test]
    fn delete_char_at_cursor_zero_with_content() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 0;
        app.delete_char();
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 0);
    }

    // ── Branching ──────────────────────────────────────────────────────

    #[test]
    fn create_and_restore_branch() {
        let mut app = App::new();
        app.add_user_message("hello".into());
        app.add_assistant_message("hi".into());

        app.create_branch("test branch".into());
        assert_eq!(app.branches.len(), 1);
        assert_eq!(app.branches[0].messages.len(), 2);

        // Add more messages
        app.add_user_message("more".into());
        assert_eq!(app.current_conversation().messages.len(), 3);

        // Restore branch
        app.restore_branch(0);
        assert_eq!(app.current_conversation().messages.len(), 2);
    }

    #[test]
    fn delete_branch() {
        let mut app = App::new();
        app.create_branch("branch1".into());
        app.create_branch("branch2".into());
        assert_eq!(app.branches.len(), 2);

        app.delete_branch(0);
        assert_eq!(app.branches.len(), 1);
        assert_eq!(app.branches[0].name, "branch2");
    }

    #[test]
    fn restore_invalid_branch() {
        let mut app = App::new();
        app.add_user_message("hello".into());
        app.restore_branch(99); // Should not panic
        assert_eq!(app.current_conversation().messages.len(), 1);
    }

    // === Branch edge cases ===

    #[test]
    fn create_branch_captures_correct_snapshot() {
        let mut app = App::new();
        app.add_user_message("msg1".into());
        app.add_assistant_message("resp1".into());
        app.create_branch("snapshot1".into());

        // Add more messages after branching
        app.add_user_message("msg2".into());

        // Branch should have only the original 2 messages
        assert_eq!(app.branches[0].messages.len(), 2);
        // Current conversation should have 3
        assert_eq!(app.current_conversation().messages.len(), 3);
    }

    #[test]
    fn create_multiple_branches_independent() {
        let mut app = App::new();
        app.add_user_message("msg1".into());
        app.create_branch("branch1".into());

        app.add_user_message("msg2".into());
        app.create_branch("branch2".into());

        assert_eq!(app.branches[0].messages.len(), 1);
        assert_eq!(app.branches[1].messages.len(), 2);
    }

    #[test]
    fn delete_branch_out_of_bounds_safe() {
        let mut app = App::new();
        app.create_branch("test".into());
        app.delete_branch(99); // Out of bounds
        assert_eq!(app.branches.len(), 1); // Not deleted
    }

    // === Conversation management edge cases ===

    #[test]
    fn switching_conversations_preserves_messages() {
        let mut app = App::new();
        app.add_user_message("conv1_msg".into());

        app.new_conversation();
        app.add_user_message("conv2_msg".into());

        // Switch back
        app.active_conversation = 0;
        assert_eq!(app.current_conversation().messages[0].1, "conv1_msg");

        // Switch forward
        app.active_conversation = 1;
        assert_eq!(app.current_conversation().messages[0].1, "conv2_msg");
    }

    // === Input handling edge cases ===

    #[test]
    fn delete_char_at_position_zero() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 0;
        app.delete_char();
        assert_eq!(app.input, "hello"); // Nothing deleted
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn cursor_movement_empty_input() {
        let mut app = App::new();
        app.move_cursor_left();
        assert_eq!(app.cursor_position, 0);
        app.move_cursor_right();
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn submit_input_with_only_newlines() {
        let mut app = App::new();
        app.input = "\n\n\n".into();
        let result = app.submit_input();
        assert!(result.is_none()); // Whitespace-only
    }

    #[test]
    fn submit_input_preserves_internal_newlines() {
        let mut app = App::new();
        app.input = "line1\nline2\nline3".into();
        let result = app.submit_input();
        assert!(result.is_some());
        assert!(result.unwrap().contains('\n'));
    }

    // === Streaming edge cases ===

    #[test]
    fn finish_streaming_multiple_times() {
        let mut app = App::new();
        app.is_streaming = true;
        app.streaming_response = "test".into();
        app.finish_streaming();
        assert!(!app.is_streaming);

        // Second call should be safe
        app.finish_streaming();
        assert!(!app.is_streaming);
    }

    #[test]
    fn append_to_streaming_concatenates() {
        let mut app = App::new();
        app.append_to_streaming("hello ");
        app.append_to_streaming("world");
        assert_eq!(app.streaming_response, "hello world");
    }

    // === set_status edge cases ===

    #[test]
    fn set_status_updates_both_fields() {
        let mut app = App::new();
        app.set_status("test message");
        assert_eq!(app.status_message, Some("test message".into()));
        assert!(app.status_time.is_some());
    }

    #[test]
    fn set_status_replaces_previous() {
        let mut app = App::new();
        app.set_status("first");
        app.set_status("second");
        assert_eq!(app.status_message, Some("second".into()));
    }

    // === Stress tests ===

    #[test]
    fn rapid_conversation_switching() {
        let mut app = App::new();
        for _ in 0..100 {
            app.new_conversation();
        }
        assert_eq!(app.conversations.len(), 101);

        // Rapidly switch between conversations
        for i in 0..101 {
            app.active_conversation = i;
            let _ = app.current_conversation();
        }
    }

    #[test]
    fn massive_message_history() {
        let mut app = App::new();
        for i in 0..1000 {
            app.add_user_message(format!("Message {i}"));
            app.add_assistant_message(format!("Response {i}"));
        }
        assert_eq!(app.current_conversation().messages.len(), 2000);
    }

    #[test]
    fn very_long_input_handling() {
        let mut app = App::new();
        let long_input = "x".repeat(100_000);
        for ch in long_input.chars() {
            app.insert_char(ch);
        }
        assert_eq!(app.input.len(), 100_000);
        assert_eq!(app.cursor_position, 100_000);

        // Submit should work
        let result = app.submit_input();
        assert!(result.is_some());
        assert!(app.input.is_empty());
    }

    #[test]
    fn cursor_at_every_position() {
        let mut app = App::new();
        app.input = "Hello, World! \u{1F980}".into();
        let len = app.input.len();

        // Move cursor to every valid position
        app.cursor_position = 0;
        for _ in 0..20 {
            app.move_cursor_right();
        }
        // Should be at or near the end, not past it
        assert!(app.cursor_position <= len);

        // Move back to start
        for _ in 0..20 {
            app.move_cursor_left();
        }
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    fn many_branches() {
        let mut app = App::new();
        app.add_user_message("base".into());

        for i in 0..50 {
            app.create_branch(format!("branch_{i}"));
        }
        assert_eq!(app.branches.len(), 50);

        // Restore the last one
        app.restore_branch(49);
        assert_eq!(app.current_conversation().messages.len(), 1);

        // Delete all
        while !app.branches.is_empty() {
            app.delete_branch(0);
        }
        assert!(app.branches.is_empty());
    }

    // ── Cursor out-of-bounds clamping ──────────────────────────────────

    #[test]
    fn cursor_out_of_bounds_clamped() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 999;

        app.insert_char('!');
        assert_eq!(app.input, "hello!");

        app.cursor_position = 999;
        app.delete_char();
        assert_eq!(app.input, "hello");

        app.cursor_position = 999;
        app.move_cursor_left();
        assert!(app.cursor_position <= app.input.len());

        app.cursor_position = 999;
        app.move_cursor_right();
        assert!(app.cursor_position <= app.input.len());
    }

    #[test]
    fn submit_input_clears_cursor() {
        let mut app = App::new();
        app.input = "test".into();
        app.cursor_position = 4;
        app.submit_input();
        assert_eq!(app.cursor_position, 0);
        assert!(app.input.is_empty());
    }

    #[test]
    fn cursor_position_never_exceeds_input_len() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 100; // Way out of bounds
        // These should not panic
        app.move_cursor_left();
        app.move_cursor_right();
        app.delete_char();
        assert!(app.cursor_position <= app.input.len());
    }

    #[test]
    fn insert_char_with_invalid_cursor() {
        let mut app = App::new();
        app.input = "hello".into();
        app.cursor_position = 100;
        // Should clamp to end and insert there
        app.insert_char('!');
        assert_eq!(app.input, "hello!");
        assert_eq!(app.cursor_position, 6);
    }

    // ── Extended default field checks ──────────────────────────────────

    #[test]
    fn app_new_has_correct_defaults_extended() {
        let app = App::new();
        assert!(!app.agent_has_stash);
        assert_eq!(app.history_sort, 0);
        assert!(!app.history_delete_pending);
        assert_eq!(app.thinking_frame, 0);
        assert_eq!(app.theme_index, 0);
        assert_eq!(app.settings_tab, 0);
        assert_eq!(app.settings_select, 0);
        assert!(app.detected_workspace.is_none());
    }

    #[test]
    fn app_set_status_timer() {
        let mut app = App::new();
        app.set_status("test");
        assert!(app.status_time.is_some());
        // The timer should be very recent
        assert!(app.status_time.unwrap().elapsed().as_secs() < 1);
    }

    // ── Input history ──────────────────────────────────────────────────

    #[test]
    fn input_history_records_submissions() {
        let mut app = App::new();
        app.input = "first command".into();
        app.submit_input();
        app.input = "second command".into();
        app.submit_input();
        assert_eq!(app.input_history.len(), 2);
        assert_eq!(app.input_history[0], "first command");
        assert_eq!(app.input_history[1], "second command");
    }

    #[test]
    fn input_history_no_consecutive_duplicates() {
        let mut app = App::new();
        app.input = "same command".into();
        app.submit_input();
        app.input = "same command".into();
        app.submit_input();
        assert_eq!(app.input_history.len(), 1); // No duplicate
    }

    #[test]
    fn input_history_empty_not_recorded() {
        let mut app = App::new();
        app.input = "   ".into();
        app.submit_input();
        assert!(app.input_history.is_empty());
    }

    #[test]
    fn input_history_resets_index_on_submit() {
        let mut app = App::new();
        app.input = "hello".into();
        app.submit_input();
        app.input_history_index = Some(0); // Simulate browsing history
        app.input = "world".into();
        app.submit_input();
        assert!(app.input_history_index.is_none());
    }

    #[test]
    fn input_history_allows_non_consecutive_duplicates() {
        let mut app = App::new();
        app.input = "aaa".into();
        app.submit_input();
        app.input = "bbb".into();
        app.submit_input();
        app.input = "aaa".into();
        app.submit_input();
        assert_eq!(app.input_history.len(), 3);
    }

    #[test]
    fn new_app_has_empty_history_and_aliases() {
        let app = App::new();
        assert!(app.input_history.is_empty());
        assert!(app.input_history_index.is_none());
        assert!(app.input_saved.is_empty());
        assert!(app.aliases.is_empty());
    }
}
