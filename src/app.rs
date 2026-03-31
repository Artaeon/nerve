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
}

/// Whether the user is navigating (Normal / vim-style) or typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
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

    // -- clipboard manager --
    pub clipboard_manager: crate::clipboard_manager::ClipboardManager,
    pub clipboard_search: String,
    pub clipboard_select_index: usize,

    // -- history browser --
    pub history_entries: Vec<crate::history::ConversationRecord>,
    pub history_select_index: usize,
    pub history_search: String,

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

    // -- misc --
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub show_help: bool,
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

            clipboard_manager: crate::clipboard_manager::ClipboardManager::load()
                .unwrap_or_else(|_| crate::clipboard_manager::ClipboardManager::new(100)),
            clipboard_search: String::new(),
            clipboard_select_index: 0,

            history_entries: Vec::new(),
            history_select_index: 0,
            history_search: String::new(),

            available_providers: vec![
                "claude_code".into(),
                "ollama".into(),
                "openai".into(),
                "openrouter".into(),
            ],
            selected_provider: "claude_code".into(),
            provider_select_index: 0,
            provider_changed: false,

            code_mode: false,
            working_dir: None,

            status_message: None,
            should_quit: false,
            show_help: false,
        }
    }

    // ── Conversation accessors ──────────────────────────────────────────

    /// Reference to the currently active conversation.
    pub fn current_conversation(&self) -> &Conversation {
        &self.conversations[self.active_conversation]
    }

    /// Mutable reference to the currently active conversation.
    pub fn current_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversations[self.active_conversation]
    }

    /// Create a new empty conversation and switch to it.
    pub fn new_conversation(&mut self) {
        self.conversations.push(Conversation::new());
        self.active_conversation = self.conversations.len() - 1;
        self.scroll_offset = 0;
        self.streaming_response.clear();
        self.is_streaming = false;
        self.status_message = Some("New conversation started".into());
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
    }

    // ── Cursor / editing ────────────────────────────────────────────────

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            // Respect grapheme boundaries by finding the previous char boundary.
            let new_pos = self.input[..self.cursor_position]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.cursor_position = new_pos;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.len() {
            let new_pos = self.input[self.cursor_position..]
                .char_indices()
                .nth(1)
                .map(|(idx, _)| self.cursor_position + idx)
                .unwrap_or(self.input.len());
            self.cursor_position = new_pos;
        }
    }

    /// Insert a character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.input[..self.cursor_position]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.input.drain(prev..self.cursor_position);
            self.cursor_position = prev;
        }
    }

    /// Take the current input text, clear the input buffer, and return it.
    /// Returns `None` if the input was empty (whitespace-only).
    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.input.clear();
        self.cursor_position = 0;
        Some(text)
    }

    // ── Scrolling ───────────────────────────────────────────────────────

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
    }
}
