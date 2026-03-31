mod ai;
mod app;
mod automation;
mod clipboard;
mod clipboard_manager;
mod config;
mod daemon;
mod history;
mod keybinds;
mod knowledge;
mod prompts;
mod scraper;
mod ui;

use std::io::{self, Read as _};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use ai::provider::{AiProvider, ChatMessage, StreamEvent};
use ai::{ClaudeCodeProvider, OpenAiProvider};
use app::{App, AppMode, InputMode};
use clipboard_manager::ClipboardSource;
use config::Config;

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "nerve", about = "Raw AI power in your terminal")]
struct Cli {
    /// Direct prompt (non-interactive mode): nerve "translate this to Spanish"
    prompt: Option<String>,

    /// Model to use
    #[arg(short, long)]
    model: Option<String>,

    /// Provider to use (claude_code, openai, ollama, openrouter)
    #[arg(short, long)]
    provider: Option<String>,

    /// Read input from stdin (pipe mode)
    #[arg(long)]
    stdin: bool,

    /// List available models
    #[arg(long)]
    list_models: bool,

    /// Non-interactive mode (just print response)
    #[arg(short = 'n', long)]
    non_interactive: bool,

    /// Start as background daemon
    #[arg(long)]
    daemon: bool,

    /// Send query to running daemon
    #[arg(long)]
    query: Option<String>,

    /// Stop the running daemon
    #[arg(long)]
    stop_daemon: bool,
}

// ─── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise tracing (writes to stderr so it doesn't interfere with piped output).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    // ── Daemon commands (no provider needed) ────────────────────────
    if cli.daemon {
        return daemon::start_daemon().await;
    }
    if cli.stop_daemon {
        daemon::stop_daemon()?;
        println!("Nerve daemon stopped.");
        return Ok(());
    }
    if let Some(query) = &cli.query {
        let response = daemon::send_to_daemon(query).await?;
        println!("{}", response);
        return Ok(());
    }

    let config = Config::load().context("failed to load configuration")?;

    let provider = create_provider(&config, cli.provider.as_deref())
        .context("failed to create AI provider")?;
    let provider: Arc<dyn AiProvider> = Arc::from(provider);

    let model = cli
        .model
        .as_deref()
        .unwrap_or(&config.default_model)
        .to_string();

    // --list-models: print and exit.
    if cli.list_models {
        return list_models(&*provider).await;
    }

    // Pipe mode: read stdin, combine with prompt, run non-interactive.
    if cli.stdin {
        let mut stdin_buf = String::new();
        io::stdin()
            .read_to_string(&mut stdin_buf)
            .context("failed to read stdin")?;
        let prompt = match &cli.prompt {
            Some(p) => format!("{p}\n\n{stdin_buf}"),
            None => stdin_buf,
        };
        return run_non_interactive(&*provider, &model, &prompt).await;
    }

    // Direct prompt with -n flag: non-interactive.
    if cli.non_interactive {
        let prompt = cli
            .prompt
            .as_deref()
            .unwrap_or("Hello, how can you help me?");
        return run_non_interactive(&*provider, &model, prompt).await;
    }

    // Direct prompt without -n: also non-interactive (convenience).
    if let Some(ref prompt) = cli.prompt {
        return run_non_interactive(&*provider, &model, prompt).await;
    }

    // Interactive TUI.
    run_tui(provider, config).await
}

// ─── Provider creation ──────────────────────────────────────────────────────

fn create_provider(
    config: &Config,
    provider_override: Option<&str>,
) -> anyhow::Result<Box<dyn AiProvider>> {
    let provider_name = provider_override.unwrap_or(&config.default_provider);
    match provider_name {
        "claude_code" | "claude" => {
            Ok(Box::new(ClaudeCodeProvider::new()))
        }
        "openai" => {
            let pc = config.providers.openai.as_ref();
            let key = resolve_api_key(
                pc.and_then(|p| p.api_key.as_deref()),
                "OPENAI_API_KEY",
            )?;
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            Ok(Box::new(OpenAiProvider::new(key, base_url, "OpenAI".into())))
        }
        "ollama" => {
            let pc = config.providers.ollama.as_ref();
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".into());
            Ok(Box::new(OpenAiProvider::new(
                "ollama".into(),
                base_url,
                "Ollama".into(),
            )))
        }
        "openrouter" => {
            let pc = config.providers.openrouter.as_ref();
            let key = resolve_api_key(
                pc.and_then(|p| p.api_key.as_deref()),
                "OPENROUTER_API_KEY",
            )?;
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".into());
            Ok(Box::new(OpenAiProvider::new(
                key,
                base_url,
                "OpenRouter".into(),
            )))
        }
        other => {
            // Check custom providers.
            if let Some(custom) = config
                .providers
                .custom
                .iter()
                .find(|c| c.name == other)
            {
                return Ok(Box::new(OpenAiProvider::new(
                    custom.api_key.clone(),
                    custom.base_url.clone(),
                    custom.name.clone(),
                )));
            }
            anyhow::bail!("Unknown provider: {other}")
        }
    }
}

/// Create a provider using app state (respects code_mode for claude_code).
fn create_provider_from_app(config: &Config, app: &App) -> anyhow::Result<Box<dyn AiProvider>> {
    let provider_name = &app.selected_provider;
    match provider_name.as_str() {
        "claude_code" | "claude" => {
            if app.code_mode {
                let mut p = ClaudeCodeProvider::with_tools();
                if let Some(ref dir) = app.working_dir {
                    p = p.with_working_dir(dir.clone());
                }
                Ok(Box::new(p))
            } else {
                Ok(Box::new(ClaudeCodeProvider::new()))
            }
        }
        _ => create_provider(config, Some(provider_name)),
    }
}

/// Resolve an API key: prefer the config value, fall back to an environment
/// variable. Returns an error if neither is set.
fn resolve_api_key(config_value: Option<&str>, env_var: &str) -> anyhow::Result<String> {
    if let Some(val) = config_value {
        if !val.is_empty() {
            return Ok(val.to_string());
        }
    }
    std::env::var(env_var)
        .with_context(|| format!("API key not found: set it in config or via ${env_var}"))
}

// ─── Non-interactive mode ───────────────────────────────────────────────────

async fn run_non_interactive(
    provider: &dyn AiProvider,
    model: &str,
    prompt: &str,
) -> anyhow::Result<()> {
    let messages = vec![ChatMessage::user(prompt)];
    let response = provider.chat(&messages, model).await?;
    println!("{response}");
    Ok(())
}

// ─── List models ────────────────────────────────────────────────────────────

async fn list_models(provider: &dyn AiProvider) -> anyhow::Result<()> {
    let models = provider.list_models().await?;
    if models.is_empty() {
        println!("No models found for provider '{}'.", provider.name());
        return Ok(());
    }
    println!("Available models ({}):", provider.name());
    for m in &models {
        let ctx = m
            .context_length
            .map(|c| format!("  (ctx: {c})"))
            .unwrap_or_default();
        println!("  {}{ctx}", m.id);
    }
    Ok(())
}

// ─── Interactive TUI ────────────────────────────────────────────────────────

async fn run_tui(provider: Arc<dyn AiProvider>, config: Config) -> anyhow::Result<()> {
    // Enter the alternate screen and enable raw mode.
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new();
    app.selected_model = config.default_model.clone();
    app.selected_provider = config.default_provider.clone();

    // No initial system message — the rich welcome screen shows when messages are empty.

    let result = event_loop(&mut terminal, &mut app, &provider, &config).await;

    // Restore terminal state regardless of how we exited.
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// The core event loop: draw, handle input, process streaming tokens.
async fn event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
    initial_provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<()> {
    let mut provider: Arc<dyn AiProvider> = Arc::clone(initial_provider);

    loop {
        // Draw the UI.
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Check if provider needs to be recreated.
        if app.provider_changed {
            match create_provider_from_app(config, app) {
                Ok(new_provider) => {
                    provider = Arc::from(new_provider);
                }
                Err(e) => {
                    app.status_message = Some(format!("Provider error: {e}"));
                }
            }
            app.provider_changed = false;
        }

        // Poll for events with a short timeout so we can service the stream.
        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    handle_key_event(app, key, &provider, config).await?;
                }
                Event::Mouse(mouse) => match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollUp => app.scroll_up(),
                    crossterm::event::MouseEventKind::ScrollDown => app.scroll_down(),
                    _ => {}
                },
                _ => {}
            }
        }

        // Drain any pending stream tokens.
        //
        // We temporarily take the receiver out of `app` so we can mutate
        // both the receiver and the rest of app without overlapping borrows.
        if app.is_streaming {
            if let Some(mut rx) = app.stream_rx.take() {
                let mut finished = false;
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        StreamEvent::Token(token) => app.append_to_streaming(&token),
                        StreamEvent::Done => {
                            // Grab the content before finish_streaming moves it.
                            let response_content = app.streaming_response.clone();
                            app.finish_streaming();
                            if !response_content.is_empty() {
                                app.clipboard_manager.add(
                                    response_content,
                                    ClipboardSource::AiResponse,
                                );
                                let _ = app.clipboard_manager.save();
                            }

                            // Auto-set title from first user message.
                            {
                                let conv = app.current_conversation_mut();
                                if conv.title == "New Conversation" {
                                    if let Some((_role, content)) =
                                        conv.messages.iter().find(|(r, _)| r == "user")
                                    {
                                        let title: String =
                                            content.chars().take(50).collect();
                                        conv.title = title;
                                    }
                                }
                            }

                            // Auto-save conversation to history.
                            {
                                let conv = app.current_conversation();
                                let record = history::ConversationRecord {
                                    id: conv.id.clone(),
                                    title: conv.title.clone(),
                                    messages: conv
                                        .messages
                                        .iter()
                                        .map(|(role, content)| {
                                            history::MessageRecord {
                                                role: role.clone(),
                                                content: content.clone(),
                                                timestamp: chrono::Utc::now(),
                                            }
                                        })
                                        .collect(),
                                    model: app.selected_model.clone(),
                                    created_at: conv.created_at,
                                    updated_at: chrono::Utc::now(),
                                };
                                let _ = history::save_conversation(&record);
                            }

                            finished = true;
                            break;
                        }
                        StreamEvent::Error(e) => {
                            app.streaming_response
                                .push_str(&format!("\n[Error: {e}]"));
                            app.finish_streaming();
                            finished = true;
                            break;
                        }
                    }
                }
                // Put it back if not finished (finish_streaming sets it to None).
                if !finished {
                    app.stream_rx = Some(rx);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

// ─── Key event handling ─────────────────────────────────────────────────────

async fn handle_key_event(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    provider: &Arc<dyn AiProvider>,
    _config: &Config,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    // ── Global keys (always active) ─────────────────────────────────────
    if mods.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('c') | KeyCode::Char('d') => {
                app.should_quit = true;
                return Ok(());
            }
            KeyCode::Char('h') => {
                if app.mode == AppMode::Help {
                    app.mode = AppMode::Normal;
                } else {
                    app.mode = AppMode::Help;
                }
                return Ok(());
            }
            _ => {}
        }
    }

    // ── Dispatch by mode ────────────────────────────────────────────────
    match app.mode {
        AppMode::Normal => handle_normal_mode(app, key, provider).await?,
        AppMode::CommandBar => handle_command_bar(app, key),
        AppMode::PromptPicker => handle_prompt_picker(app, key),
        AppMode::ModelSelect => handle_model_select(app, key),
        AppMode::ProviderSelect => handle_provider_select(app, key),
        AppMode::Help => {
            // Any key besides Ctrl+H (handled above) closes help.
            if code == KeyCode::Esc {
                app.mode = AppMode::Normal;
            }
        }
        AppMode::Settings => {
            if code == KeyCode::Esc {
                app.mode = AppMode::Normal;
            }
        }
        AppMode::ClipboardManager => handle_clipboard_manager(app, key),
        AppMode::HistoryBrowser => handle_history_browser(app, key),
    }

    Ok(())
}

// ── Normal mode ─────────────────────────────────────────────────────────────

async fn handle_normal_mode(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    provider: &Arc<dyn AiProvider>,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    match app.input_mode {
        // ── Normal / vim-navigation ─────────────────────────────────────
        InputMode::Normal => {
            if mods.contains(KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('k') => {
                        app.mode = AppMode::CommandBar;
                        app.command_bar_input.clear();
                        app.command_bar_select_index = 0;
                        app.command_bar_category = 0;
                    }
                    KeyCode::Char('n') => app.new_conversation(),
                    KeyCode::Char('p') => {
                        app.mode = AppMode::PromptPicker;
                        app.prompt_filter.clear();
                        app.prompt_select_index = 0;
                        app.prompt_category_index = 0;
                        app.prompt_focus_right = false;
                    }
                    KeyCode::Char('m') => {
                        app.mode = AppMode::ModelSelect;
                        app.model_select_index = app
                            .available_models
                            .iter()
                            .position(|m| m == &app.selected_model)
                            .unwrap_or(0);
                    }
                    KeyCode::Char('t') => {
                        app.mode = AppMode::ProviderSelect;
                        app.provider_select_index = app
                            .available_providers
                            .iter()
                            .position(|p| p == &app.selected_provider)
                            .unwrap_or(0);
                    }
                    KeyCode::Char('y') => copy_last_assistant_message(app),
                    KeyCode::Char('l') => clear_conversation(app),
                    KeyCode::Char('b') => {
                        app.mode = AppMode::ClipboardManager;
                        app.clipboard_search.clear();
                        app.clipboard_select_index = 0;
                    }
                    KeyCode::Char('o') => {
                        app.history_entries =
                            history::list_conversations().unwrap_or_default();
                        app.history_select_index = 0;
                        app.history_search.clear();
                        app.mode = AppMode::HistoryBrowser;
                    }
                    _ => {}
                }
                return Ok(());
            }

            match code {
                KeyCode::Char('i') => app.input_mode = InputMode::Insert,
                KeyCode::Char('/') => {
                    app.mode = AppMode::CommandBar;
                    app.command_bar_input.clear();
                    app.command_bar_select_index = 0;
                    app.command_bar_category = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => app.scroll_down(),
                KeyCode::Char('k') | KeyCode::Up => app.scroll_up(),
                KeyCode::Tab => cycle_conversation(app),
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }

        // ── Insert / typing mode ────────────────────────────────────────
        InputMode::Insert => {
            if mods.contains(KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('k') => {
                        app.mode = AppMode::CommandBar;
                        app.command_bar_input.clear();
                        app.command_bar_select_index = 0;
                        app.command_bar_category = 0;
                    }
                    KeyCode::Char('n') => app.new_conversation(),
                    KeyCode::Char('p') => {
                        app.mode = AppMode::PromptPicker;
                        app.prompt_filter.clear();
                        app.prompt_select_index = 0;
                        app.prompt_category_index = 0;
                        app.prompt_focus_right = false;
                    }
                    KeyCode::Char('m') => {
                        app.mode = AppMode::ModelSelect;
                        app.model_select_index = app
                            .available_models
                            .iter()
                            .position(|m| m == &app.selected_model)
                            .unwrap_or(0);
                    }
                    KeyCode::Char('t') => {
                        app.mode = AppMode::ProviderSelect;
                        app.provider_select_index = app
                            .available_providers
                            .iter()
                            .position(|p| p == &app.selected_provider)
                            .unwrap_or(0);
                    }
                    KeyCode::Char('v') => {
                        if let Ok(text) = clipboard::paste_from_clipboard() {
                            for ch in text.chars() {
                                app.insert_char(ch);
                            }
                        }
                    }
                    KeyCode::Char('y') => copy_last_assistant_message(app),
                    KeyCode::Char('l') => clear_conversation(app),
                    KeyCode::Char('b') => {
                        app.mode = AppMode::ClipboardManager;
                        app.clipboard_search.clear();
                        app.clipboard_select_index = 0;
                    }
                    KeyCode::Char('o') => {
                        app.history_entries =
                            history::list_conversations().unwrap_or_default();
                        app.history_select_index = 0;
                        app.history_search.clear();
                        app.mode = AppMode::HistoryBrowser;
                    }
                    KeyCode::Char('w') => {
                        // Delete word before cursor.
                        let before = &app.input[..app.cursor_position];
                        let trimmed = before.trim_end();
                        let new_pos = trimmed
                            .rfind(|c: char| c.is_whitespace())
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        app.input.drain(new_pos..app.cursor_position);
                        app.cursor_position = new_pos;
                    }
                    _ => {}
                }
                return Ok(());
            }

            match code {
                KeyCode::Enter => {
                    if mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT) {
                        // Insert newline for multi-line input
                        app.insert_char('\n');
                    } else {
                        // Submit message
                        if app.is_streaming {
                            return Ok(());
                        }
                        if let Some(text) = app.submit_input() {
                            submit_message(app, &text, provider).await;
                        }
                    }
                }
                KeyCode::Esc => app.input_mode = InputMode::Normal,
                KeyCode::Backspace => app.delete_char(),
                KeyCode::Left => app.move_cursor_left(),
                KeyCode::Right => app.move_cursor_right(),
                KeyCode::Char(c) => app.insert_char(c),
                _ => {}
            }
        }
    }

    Ok(())
}

// ── Command bar ─────────────────────────────────────────────────────────────

fn handle_command_bar(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Use the helper from the UI module to get the selected prompt.
            if let Some(prompt) = ui::command_bar::selected_prompt(app) {
                let template = if app.input.is_empty() {
                    prompt.template.replace("{{input}}", "")
                } else {
                    prompt.template.replace("{{input}}", &app.input)
                };
                app.input = template;
                app.cursor_position = app.input.len();
                app.input_mode = InputMode::Insert;
                app.status_message = Some(format!("Loaded prompt: {}", prompt.name));
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            let cat_count = prompts::categories().len() + 1; // +1 for "All"
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.command_bar_category = if app.command_bar_category == 0 {
                    cat_count - 1
                } else {
                    app.command_bar_category - 1
                };
            } else {
                app.command_bar_category = (app.command_bar_category + 1) % cat_count;
            }
            app.command_bar_select_index = 0;
        }
        KeyCode::BackTab => {
            // Shift+Tab also reported as BackTab on some terminals.
            let cat_count = prompts::categories().len() + 1;
            app.command_bar_category = if app.command_bar_category == 0 {
                cat_count - 1
            } else {
                app.command_bar_category - 1
            };
            app.command_bar_select_index = 0;
        }
        KeyCode::Backspace => {
            app.command_bar_input.pop();
            app.command_bar_select_index = 0;
        }
        KeyCode::Up => {
            app.command_bar_select_index = app.command_bar_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::command_bar::matched_prompt_count(app);
            if app.command_bar_select_index + 1 < count {
                app.command_bar_select_index += 1;
            }
        }
        KeyCode::Char(c) => {
            app.command_bar_input.push(c);
            app.command_bar_select_index = 0;
        }
        _ => {}
    }
}

// ── Prompt picker ───────────────────────────────────────────────────────────

fn handle_prompt_picker(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            app.prompt_focus_right = !app.prompt_focus_right;
            if app.prompt_focus_right {
                app.prompt_select_index = 0;
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.prompt_focus_right {
                let count = ui::prompt_picker::visible_prompt_count(app);
                if app.prompt_select_index + 1 < count {
                    app.prompt_select_index += 1;
                }
            } else {
                let cat_count = prompts::categories().len();
                if app.prompt_category_index + 1 < cat_count {
                    app.prompt_category_index += 1;
                    app.prompt_select_index = 0;
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.prompt_focus_right {
                app.prompt_select_index = app.prompt_select_index.saturating_sub(1);
            } else {
                if app.prompt_category_index > 0 {
                    app.prompt_category_index -= 1;
                    app.prompt_select_index = 0;
                }
            }
        }
        KeyCode::Enter => {
            let all = prompts::all_prompts();
            let cats = prompts::categories();
            let selected_cat = cats
                .get(app.prompt_category_index)
                .cloned()
                .unwrap_or_default();
            let filtered: Vec<&prompts::SmartPrompt> =
                all.iter().filter(|p| p.category == selected_cat).collect();

            if let Some(prompt) = filtered.get(app.prompt_select_index) {
                let template = if app.input.is_empty() {
                    prompt.template.replace("{{input}}", "")
                } else {
                    prompt.template.replace("{{input}}", &app.input)
                };
                app.input = template;
                app.cursor_position = app.input.len();
                app.input_mode = InputMode::Insert;
                app.status_message = Some(format!("Loaded prompt: {}", prompt.name));
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(c) => {
            // Characters typed while in the prompt picker go to the filter.
            app.prompt_filter.push(c);
            app.prompt_select_index = 0;
        }
        KeyCode::Backspace => {
            app.prompt_filter.pop();
            app.prompt_select_index = 0;
        }
        _ => {}
    }
}

// ── Model selection ─────────────────────────────────────────────────────────

fn handle_model_select(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.model_select_index + 1 < app.available_models.len() {
                app.model_select_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.model_select_index = app.model_select_index.saturating_sub(1);
        }
        KeyCode::Enter => {
            if let Some(model) = app.available_models.get(app.model_select_index) {
                app.selected_model = model.clone();
                app.status_message = Some(format!("Model set to {model}"));
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Provider selection ─────────────────────────────────────────────────────

fn handle_provider_select(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Up | KeyCode::Char('k') => {
            app.provider_select_index = app.provider_select_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.provider_select_index + 1 < app.available_providers.len() {
                app.provider_select_index += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(provider_name) = app.available_providers.get(app.provider_select_index) {
                app.selected_provider = provider_name.clone();
                app.provider_changed = true;
                app.status_message = Some(format!("Provider switched to {}", provider_name));
            }
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

// ── Clipboard manager ──────────────────────────────────────────────────────

fn handle_clipboard_manager(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let filtered = app.clipboard_manager.search(&app.clipboard_search);
            if let Some(&(original_idx, _)) = filtered.get(app.clipboard_select_index) {
                match app.clipboard_manager.copy_to_system(original_idx) {
                    Ok(()) => {
                        app.status_message = Some("Copied to clipboard".into());
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Clipboard error: {e}"));
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('d') if app.clipboard_search.is_empty() => {
            let filtered = app.clipboard_manager.search(&app.clipboard_search);
            if let Some(&(original_idx, _)) = filtered.get(app.clipboard_select_index) {
                app.clipboard_manager.remove(original_idx);
                let new_count = app.clipboard_manager.search(&app.clipboard_search).len();
                if app.clipboard_select_index >= new_count && new_count > 0 {
                    app.clipboard_select_index = new_count - 1;
                } else if new_count == 0 {
                    app.clipboard_select_index = 0;
                }
                let _ = app.clipboard_manager.save();
            }
        }
        KeyCode::Up => {
            app.clipboard_select_index = app.clipboard_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::clipboard_manager::matched_entry_count(app);
            if app.clipboard_select_index + 1 < count {
                app.clipboard_select_index += 1;
            }
        }
        KeyCode::Backspace => {
            app.clipboard_search.pop();
            app.clipboard_select_index = 0;
        }
        KeyCode::Char(c) => {
            app.clipboard_search.push(c);
            app.clipboard_select_index = 0;
        }
        _ => {}
    }
}

// ── History browser ────────────────────────────────────────────────────────

fn handle_history_browser(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let filtered = filtered_history_entries(app);
            if let Some(record) = filtered.get(app.history_select_index).cloned() {
                let conv = app::Conversation {
                    id: record.id.clone(),
                    title: record.title.clone(),
                    messages: record
                        .messages
                        .iter()
                        .map(|m| (m.role.clone(), m.content.clone()))
                        .collect(),
                    created_at: record.created_at,
                };
                app.conversations.push(conv);
                app.active_conversation = app.conversations.len() - 1;
                app.scroll_offset = 0;
                app.streaming_response.clear();
                app.is_streaming = false;
                app.status_message =
                    Some(format!("Loaded conversation: {}", record.title));
                app.mode = AppMode::Normal;
            }
        }
        KeyCode::Char('d') if app.history_search.is_empty() => {
            let filtered = filtered_history_entries(app);
            if let Some(record) = filtered.get(app.history_select_index).cloned() {
                let _ = history::delete_conversation(&record.id);
                app.history_entries.retain(|r| r.id != record.id);
                let new_count = filtered_history_entries(app).len();
                if app.history_select_index >= new_count && new_count > 0 {
                    app.history_select_index = new_count - 1;
                } else if new_count == 0 {
                    app.history_select_index = 0;
                }
                app.status_message = Some("Conversation deleted".into());
            }
        }
        KeyCode::Up | KeyCode::Char('k') if app.history_search.is_empty() => {
            app.history_select_index = app.history_select_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') if app.history_search.is_empty() => {
            let count = ui::history_browser::filtered_history_count(app);
            if app.history_select_index + 1 < count {
                app.history_select_index += 1;
            }
        }
        KeyCode::Up => {
            app.history_select_index = app.history_select_index.saturating_sub(1);
        }
        KeyCode::Down => {
            let count = ui::history_browser::filtered_history_count(app);
            if app.history_select_index + 1 < count {
                app.history_select_index += 1;
            }
        }
        KeyCode::Backspace => {
            app.history_search.pop();
            app.history_select_index = 0;
        }
        KeyCode::Char(c) => {
            app.history_search.push(c);
            app.history_select_index = 0;
        }
        _ => {}
    }
}

fn filtered_history_entries(app: &App) -> Vec<history::ConversationRecord> {
    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::SkimMatcherV2;
    let matcher = SkimMatcherV2::default();
    let query = &app.history_search;
    app.history_entries
        .iter()
        .filter(|record| {
            if query.is_empty() {
                return true;
            }
            if matcher.fuzzy_match(&record.title, query).is_some() {
                return true;
            }
            for msg in &record.messages {
                if matcher.fuzzy_match(&msg.content, query).is_some() {
                    return true;
                }
            }
            false
        })
        .cloned()
        .collect()
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Submit the user's message and spawn a streaming response task.
///
/// Handles slash commands (`/help`, `/clear`, `/new`, `/model`, `/models`,
/// `/url`) before falling through to the normal AI chat path.
async fn submit_message(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    // ── Slash-command dispatch ──────────────────────────────────────────
    if text.starts_with('/') {
        if handle_slash_command(app, text, provider).await {
            return;
        }
        // Not a recognised command — treat as a normal message.
    }

    send_to_ai(app, text, provider).await;
}

/// Handle slash commands. Returns `true` if the command was recognised and
/// handled (so the caller should *not* forward the text to the AI).
async fn handle_slash_command(
    app: &mut App,
    text: &str,
    provider: &Arc<dyn AiProvider>,
) -> bool {
    let trimmed = text.trim();

    // /help — show available commands as an assistant message.
    if trimmed == "/help" {
        let help = "\
Available Commands\n\
══════════════════\n\
\n\
Chat\n\
  /clear              Clear current conversation\n\
  /new                Start new conversation\n\
  /rename <title>     Rename current conversation\n\
  /export             Export conversation to markdown\n\
\n\
AI Provider\n\
  /provider <name>    Switch provider\n\
  /providers          List available providers\n\
  /model <name>       Switch model\n\
  /models             List available models\n\
  /code on|off        Toggle code mode (Claude Code only)\n\
  /cwd <dir>          Set working directory\n\
\n\
Knowledge & Context\n\
  /kb add <dir>       Add directory to knowledge base\n\
  /kb search <query>  Search knowledge base\n\
  /kb list            List knowledge bases\n\
  /kb status          Show KB statistics\n\
  /kb clear           Clear knowledge base\n\
  /url <url> [q]      Scrape URL for context\n\
\n\
Automation\n\
  /auto list          List automations\n\
  /auto run <name>    Run automation\n\
  /auto info <name>   Show automation details\n\
  /auto create <name> Create custom automation\n\
  /auto delete <name> Delete custom automation\n\
\n\
System\n\
  /status             Show system status\n\
  /help               Show this help";
        app.current_conversation_mut()
            .messages
            .push(("assistant".into(), help.into()));
        app.scroll_offset = 0;
        return true;
    }

    // /clear — clear conversation.
    if trimmed == "/clear" {
        clear_conversation(app);
        return true;
    }

    // /new — new conversation.
    if trimmed == "/new" {
        app.new_conversation();
        return true;
    }

    // /models — list available models in a status message.
    if trimmed == "/models" {
        let list = app
            .available_models
            .iter()
            .enumerate()
            .map(|(i, m)| {
                if *m == app.selected_model {
                    format!("  {} {} (active)", i + 1, m)
                } else {
                    format!("  {} {}", i + 1, m)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let msg = format!("Available models:\n{list}");
        app.current_conversation_mut()
            .messages
            .push(("assistant".into(), msg));
        app.scroll_offset = 0;
        return true;
    }

    // /model <name> — switch model.
    if let Some(rest) = trimmed.strip_prefix("/model ") {
        let name = rest.trim();
        if name.is_empty() {
            app.status_message = Some("Usage: /model <model-name>".into());
            return true;
        }
        // Try exact match first, then prefix match.
        let matched = app
            .available_models
            .iter()
            .find(|m| m.as_str() == name)
            .or_else(|| {
                app.available_models
                    .iter()
                    .find(|m| m.starts_with(name))
            })
            .cloned();
        match matched {
            Some(model) => {
                app.selected_model = model.clone();
                app.status_message = Some(format!("Model set to {model}"));
            }
            None => {
                app.status_message = Some(format!("Unknown model: {name}"));
            }
        }
        return true;
    }

    // /providers — list available providers with descriptions.
    if trimmed == "/providers" {
        let current = &app.selected_provider;
        let list = format!(
            "Available providers:\n\n\
             {} claude_code  - Claude Code (subscription, no API key)\n\
             {} ollama      - Ollama (local, no API key)\n\
             {} openai      - OpenAI (requires OPENAI_API_KEY)\n\
             {} openrouter  - OpenRouter (requires OPENROUTER_API_KEY)\n\n\
             Current: {}\n\
             Switch with: /provider <name> or Ctrl+T",
            if current == "claude_code" || current == "claude" { "*" } else { " " },
            if current == "ollama" { "*" } else { " " },
            if current == "openai" { "*" } else { " " },
            if current == "openrouter" { "*" } else { " " },
            current
        );
        app.add_assistant_message(list);
        app.scroll_offset = 0;
        return true;
    }

    // /provider — bare command shows current provider.
    if trimmed == "/provider" {
        app.add_assistant_message(format!(
            "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter",
            app.selected_provider
        ));
        app.scroll_offset = 0;
        return true;
    }

    // /provider <name> — switch provider.
    if let Some(rest) = trimmed.strip_prefix("/provider ") {
        let name = rest.trim();
        if name.is_empty() {
            app.add_assistant_message(format!(
                "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter",
                app.selected_provider
            ));
            app.scroll_offset = 0;
            return true;
        }
        let valid = ["claude_code", "claude", "ollama", "openai", "openrouter"];
        if valid.contains(&name) {
            app.selected_provider = name.to_string();
            app.provider_changed = true;
            app.status_message = Some(format!("Switched to provider: {name}"));
        } else {
            app.add_assistant_message(format!(
                "Unknown provider: {name}\nAvailable: claude_code, ollama, openai, openrouter"
            ));
        }
        return true;
    }

    // /url <url> [question] — scrape URL and use as context.
    if let Some(rest) = trimmed.strip_prefix("/url ") {
        let rest = rest.trim();
        if rest.is_empty() {
            app.status_message = Some("Usage: /url <url> [question]".into());
            return true;
        }

        // Split into URL and optional question.
        let (url, question) = match rest.find(|c: char| c.is_whitespace()) {
            Some(pos) => {
                let u = &rest[..pos];
                let q = rest[pos..].trim();
                if q.is_empty() {
                    (u, None)
                } else {
                    (u, Some(q.to_string()))
                }
            }
            None => (rest, None),
        };

        app.status_message = Some(format!("Scraping {url}..."));

        match scraper::scrape_url(url).await {
            Ok(result) => {
                let title_str = result
                    .title
                    .as_deref()
                    .map(|t| format!(" ({t})"))
                    .unwrap_or_default();
                let context_msg = format!(
                    "Context from {}{title_str} [{} words]:\n\n{}",
                    result.url, result.word_count, result.content
                );
                // Add scraped content as a system message for context.
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context_msg));

                let user_msg = match question {
                    Some(q) => q,
                    None => format!("I've loaded content from {url}. Please summarise it."),
                };
                app.status_message =
                    Some(format!("Scraped {url} ({} words)", result.word_count));

                // Now send the user's question (or default) to the AI with the
                // scraped context already in the conversation.
                app.add_user_message(user_msg);
                app.scroll_offset = 0;
                send_to_ai_from_history(app, provider).await;
            }
            Err(e) => {
                app.status_message = Some(format!("Scrape failed: {e}"));
            }
        }
        return true;
    }

    // /code — toggle code mode (tool/file access for Claude Code).
    if trimmed == "/code" || trimmed.starts_with("/code ") {
        let rest = trimmed.strip_prefix("/code").unwrap_or("").trim();
        if rest == "on" {
            if app.selected_provider == "claude_code" || app.selected_provider == "claude" {
                app.code_mode = true;
                app.provider_changed = true;
                app.status_message = Some("Code mode ON - Claude has file & terminal access".into());
            } else {
                app.add_assistant_message(
                    "Code mode is only available with the claude_code provider.".into(),
                );
            }
        } else if rest == "off" {
            app.code_mode = false;
            app.provider_changed = true;
            app.status_message = Some("Code mode OFF - chat only".into());
        } else {
            let status = if app.code_mode { "ON" } else { "OFF" };
            app.add_assistant_message(format!(
                "Code mode: {status}\n\
                 Use /code on to enable file & terminal access.\n\
                 Use /code off for chat-only mode."
            ));
        }
        app.scroll_offset = 0;
        return true;
    }

    // /cwd <dir> — set working directory for Claude Code file access.
    if trimmed == "/cwd" || trimmed.starts_with("/cwd ") {
        let rest = trimmed.strip_prefix("/cwd").unwrap_or("").trim();
        if rest.is_empty() {
            let current = app
                .working_dir
                .as_deref()
                .unwrap_or("(not set)");
            app.add_assistant_message(format!(
                "Working directory: {current}\n\
                 Use /cwd <path> to set a directory for Claude Code file access."
            ));
        } else {
            let path = std::path::Path::new(rest);
            if path.is_dir() {
                app.working_dir = Some(rest.to_string());
                app.provider_changed = true;
                app.status_message = Some(format!("Working directory set to {rest}"));
            } else {
                app.status_message = Some(format!("Not a directory: {rest}"));
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // /auto — automation commands.
    if trimmed == "/auto" || trimmed.starts_with("/auto ") {
        handle_auto_command(app, trimmed, provider).await;
        return true;
    }

    // /kb — knowledge base commands.
    if trimmed == "/kb" || trimmed.starts_with("/kb ") {
        handle_kb_command(app, trimmed);
        return true;
    }

    // /status — show comprehensive system status.
    if trimmed == "/status" {
        let code_status = if app.code_mode { "ON" } else { "OFF" };
        let conv = app.current_conversation();
        let conv_title = &conv.title;
        let conv_msg_count = conv.messages.len();

        // Gather KB stats.
        let (kb_docs, kb_chunks) = match knowledge::KnowledgeBase::load("default") {
            Ok(kb) => (kb.documents.len(), kb.total_chunks()),
            Err(_) => (0, 0),
        };

        let clip_count = app.clipboard_manager.entries().len();
        let history_count = history::list_conversations()
            .map(|v| v.len())
            .unwrap_or(0);

        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(".config"))
            .join("nerve")
            .join("config.toml");
        let history_path = history::history_dir();

        let status = format!(
            "Nerve v0.1.0\n\
             \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n\
             Provider:  {}\n\
             Model:     {}\n\
             Code Mode: {}\n\
             \n\
             Conversations: {}\n\
             Current:       \"{}\" ({} messages)\n\
             \n\
             Knowledge Base: {} documents, {} chunks\n\
             Clipboard:      {} entries\n\
             History:        {} saved conversations\n\
             \n\
             Config:  {}\n\
             History: {}",
            app.selected_provider,
            app.selected_model,
            code_status,
            app.conversations.len(),
            conv_title,
            conv_msg_count,
            kb_docs,
            kb_chunks,
            clip_count,
            history_count,
            config_path.display(),
            history_path.display(),
        );
        app.add_assistant_message(status);
        app.scroll_offset = 0;
        return true;
    }

    // /export — export current conversation to markdown.
    if trimmed == "/export" {
        let conv = app.current_conversation();
        let export_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("nerve")
            .join("exports");
        std::fs::create_dir_all(&export_dir).ok();

        let filename = format!(
            "conversation_{}.md",
            conv.id.chars().take(8).collect::<String>()
        );
        let path = export_dir.join(&filename);

        let mut content = format!(
            "# {}\nModel: {} | Provider: {}\nDate: {}\n\n---\n\n",
            conv.title,
            app.selected_model,
            app.selected_provider,
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
        );

        for (role, msg) in &conv.messages {
            let label = match role.as_str() {
                "user" => "You",
                "assistant" => "AI",
                "system" => "System",
                _ => role,
            };
            content.push_str(&format!("## {}\n{}\n\n---\n\n", label, msg));
        }

        match std::fs::write(&path, &content) {
            Ok(()) => app.status_message = Some(format!("Exported to {}", path.display())),
            Err(e) => app.status_message = Some(format!("Export error: {e}")),
        }
        return true;
    }

    // /rename <title> — rename the current conversation.
    if trimmed == "/rename" || trimmed.starts_with("/rename ") {
        let rest = trimmed.strip_prefix("/rename").unwrap_or("").trim();
        if rest.is_empty() {
            app.add_assistant_message("Usage: /rename <new title>".into());
        } else {
            let new_title = rest.to_string();
            app.current_conversation_mut().title = new_title.clone();
            app.status_message = Some(format!("Renamed to: {new_title}"));
        }
        app.scroll_offset = 0;
        return true;
    }

    false
}

/// Handle `/kb` sub-commands for the knowledge base system.
fn handle_kb_command(app: &mut App, trimmed: &str) {
    let rest = trimmed.strip_prefix("/kb").unwrap_or("").trim();

    // /kb add <directory>
    if let Some(dir_path) = rest.strip_prefix("add ") {
        let dir_path = dir_path.trim();
        if dir_path.is_empty() {
            app.status_message = Some("Usage: /kb add <directory>".into());
            return;
        }
        let path = std::path::Path::new(dir_path);
        if !path.is_dir() {
            app.status_message = Some(format!("Not a directory: {dir_path}"));
            return;
        }
        let mut kb = knowledge::KnowledgeBase::load("default")
            .unwrap_or_else(|_| knowledge::KnowledgeBase::new("default".into()));
        match knowledge::ingest_directory(path, &mut kb) {
            Ok(count) => {
                if let Err(e) = kb.save() {
                    app.status_message =
                        Some(format!("Ingested {count} docs but failed to save: {e}"));
                } else {
                    let msg = format!(
                        "Ingested {count} document(s) from {dir_path}\n\
                         KB now has {} chunks across {} documents ({} words)",
                        kb.total_chunks(),
                        kb.documents.len(),
                        kb.total_words()
                    );
                    app.current_conversation_mut()
                        .messages
                        .push(("assistant".into(), msg));
                    app.scroll_offset = 0;
                    app.status_message = Some(format!("KB: ingested {count} documents"));
                }
            }
            Err(e) => {
                app.status_message = Some(format!("Ingest failed: {e}"));
            }
        }
        return;
    }

    // /kb list
    if rest == "list" {
        match knowledge::KnowledgeBase::list_all() {
            Ok(names) if names.is_empty() => {
                app.current_conversation_mut().messages.push((
                    "assistant".into(),
                    "No knowledge bases found. Use /kb add <directory> to create one.".into(),
                ));
                app.scroll_offset = 0;
            }
            Ok(names) => {
                let mut lines = vec!["Knowledge bases:".to_string()];
                for name in &names {
                    if let Ok(kb) = knowledge::KnowledgeBase::load(name) {
                        lines.push(format!(
                            "  {} — {} docs, {} chunks, {} words",
                            name,
                            kb.documents.len(),
                            kb.total_chunks(),
                            kb.total_words()
                        ));
                    } else {
                        lines.push(format!("  {} — (could not load)", name));
                    }
                }
                app.current_conversation_mut()
                    .messages
                    .push(("assistant".into(), lines.join("\n")));
                app.scroll_offset = 0;
            }
            Err(e) => {
                app.status_message = Some(format!("Failed to list KBs: {e}"));
            }
        }
        return;
    }

    // /kb search <query>
    if let Some(query) = rest.strip_prefix("search ") {
        let query = query.trim();
        if query.is_empty() {
            app.status_message = Some("Usage: /kb search <query>".into());
            return;
        }
        match knowledge::KnowledgeBase::load("default") {
            Ok(kb) => {
                let results = knowledge::search_knowledge(&kb, query, 5);
                if results.is_empty() {
                    app.current_conversation_mut().messages.push((
                        "assistant".into(),
                        format!("No results found for: {query}"),
                    ));
                } else {
                    let mut lines = vec![format!("Knowledge base results for \"{query}\":")];
                    for (i, r) in results.iter().enumerate() {
                        let preview: String = r.chunk.content.chars().take(200).collect();
                        lines.push(format!(
                            "\n{}. [{}] (score: {:.1})\n   {}{}",
                            i + 1,
                            r.document_title,
                            r.score,
                            preview,
                            if r.chunk.content.len() > 200 { "..." } else { "" }
                        ));
                    }
                    app.current_conversation_mut()
                        .messages
                        .push(("assistant".into(), lines.join("\n")));
                }
                app.scroll_offset = 0;
            }
            Err(_) => {
                app.status_message = Some(
                    "No default knowledge base found. Use /kb add <directory> first.".into(),
                );
            }
        }
        return;
    }

    // /kb clear
    if rest == "clear" {
        let kb = knowledge::KnowledgeBase::new("default".into());
        match kb.save() {
            Ok(()) => {
                app.current_conversation_mut()
                    .messages
                    .push(("assistant".into(), "Default knowledge base cleared.".into()));
                app.scroll_offset = 0;
                app.status_message = Some("KB cleared".into());
            }
            Err(e) => {
                app.status_message = Some(format!("Failed to clear KB: {e}"));
            }
        }
        return;
    }

    // /kb status (or bare /kb)
    if rest == "status" || rest.is_empty() {
        match knowledge::KnowledgeBase::load("default") {
            Ok(kb) => {
                let msg = format!(
                    "Knowledge base \"default\":\n  Documents: {}\n  Chunks: {}\n  \
                     Total words: {}\n  Created: {}\n  Updated: {}",
                    kb.documents.len(),
                    kb.total_chunks(),
                    kb.total_words(),
                    kb.created_at.format("%Y-%m-%d %H:%M"),
                    kb.updated_at.format("%Y-%m-%d %H:%M")
                );
                app.current_conversation_mut()
                    .messages
                    .push(("assistant".into(), msg));
                app.scroll_offset = 0;
            }
            Err(_) => {
                app.current_conversation_mut().messages.push((
                    "assistant".into(),
                    "No default knowledge base found. Use /kb add <directory> to create one."
                        .into(),
                ));
                app.scroll_offset = 0;
            }
        }
        return;
    }

    app.status_message = Some(format!(
        "Unknown /kb command: {rest}. Try /kb add, /kb list, /kb search, /kb clear, or /kb status."
    ));
}

/// Handle `/auto` sub-commands.
async fn handle_auto_command(
    app: &mut App,
    trimmed: &str,
    provider: &Arc<dyn AiProvider>,
) {
    let rest = trimmed.strip_prefix("/auto").unwrap_or("").trim();

    // /auto list (or bare /auto)
    if rest.is_empty() || rest == "list" {
        let all = automation::all_automations();
        if all.is_empty() {
            app.add_assistant_message("No automations available.".into());
        } else {
            let mut msg = String::from("Available automations:\n\n");
            let builtin_names: Vec<String> = automation::builtin_automations()
                .iter()
                .map(|a| a.name.clone())
                .collect();

            for auto in &all {
                let tag = if builtin_names.contains(&auto.name) {
                    " [built-in]"
                } else {
                    " [custom]"
                };
                msg.push_str(&format!(
                    "  {} — {} ({} steps){}\n",
                    auto.name,
                    auto.description,
                    auto.steps.len(),
                    tag,
                ));
            }
            app.add_assistant_message(msg);
        }
        app.scroll_offset = 0;
        return;
    }

    // /auto info <name>
    if let Some(name) = rest.strip_prefix("info ") {
        let name = name.trim();
        if name.is_empty() {
            app.status_message = Some("Usage: /auto info <name>".into());
            return;
        }
        match automation::find_automation(name) {
            Ok(auto) => {
                let mut msg = format!("Automation: {}\n", auto.name);
                msg.push_str(&format!("Description: {}\n", auto.description));
                msg.push_str(&format!("Steps: {}\n\n", auto.steps.len()));
                for (i, step) in auto.steps.iter().enumerate() {
                    msg.push_str(&format!("  Step {}: {}\n", i + 1, step.name));
                    let model_str = step.model.as_deref().unwrap_or("(default)");
                    msg.push_str(&format!("    Model: {}\n", model_str));
                    msg.push_str(&format!("    Prompt: {}\n\n", step.prompt_template));
                }
                app.add_assistant_message(msg);
            }
            Err(_) => {
                app.status_message = Some(format!("Automation '{name}' not found"));
            }
        }
        app.scroll_offset = 0;
        return;
    }

    // /auto delete <name>
    if let Some(name) = rest.strip_prefix("delete ") {
        let name = name.trim();
        if name.is_empty() {
            app.status_message = Some("Usage: /auto delete <name>".into());
            return;
        }
        // Prevent deleting built-in automations.
        let builtin_names: Vec<String> = automation::builtin_automations()
            .iter()
            .map(|a| a.name.to_lowercase())
            .collect();
        if builtin_names.contains(&name.to_lowercase()) {
            app.status_message = Some("Cannot delete built-in automations".into());
            return;
        }
        match automation::delete_automation(name) {
            Ok(()) => {
                app.status_message = Some(format!("Deleted automation '{name}'"));
            }
            Err(e) => {
                app.status_message = Some(format!("Delete failed: {e}"));
            }
        }
        return;
    }

    // /auto create <name>
    if let Some(name) = rest.strip_prefix("create ") {
        let name = name.trim();
        if name.is_empty() {
            app.status_message = Some("Usage: /auto create <name>".into());
            return;
        }
        let auto = automation::Automation::new(name.to_string(), "Custom automation".into());
        match automation::save_automation(&auto) {
            Ok(()) => {
                let dir = dirs::config_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from(".config"))
                    .join("nerve")
                    .join("automations");
                let sanitized: String = name
                    .to_lowercase()
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '-'
                        }
                    })
                    .collect();
                let msg = format!(
                    "Created automation '{name}'. Edit the TOML file to add steps:\n\
                     \n  {}/{sanitized}.toml\n\n\
                     Example step format in the TOML:\n\n\
                     [[steps]]\n\
                     name = \"Step Name\"\n\
                     prompt_template = \"Your prompt with {{{{input}}}} and {{{{prev_output}}}}\"\n",
                    dir.display(),
                );
                app.add_assistant_message(msg);
            }
            Err(e) => {
                app.status_message = Some(format!("Create failed: {e}"));
            }
        }
        app.scroll_offset = 0;
        return;
    }

    // /auto run <name>
    if let Some(name) = rest.strip_prefix("run ") {
        let name = name.trim();
        if name.is_empty() {
            app.status_message = Some("Usage: /auto run <name>".into());
            return;
        }
        match automation::find_automation(name) {
            Ok(auto) => {
                if auto.steps.is_empty() {
                    app.status_message = Some(format!("Automation '{name}' has no steps"));
                    return;
                }

                // Use the current input buffer, or fall back to the last user
                // message in the conversation.
                let input = if !app.input.trim().is_empty() {
                    let text = app.input.trim().to_string();
                    app.input.clear();
                    app.cursor_position = 0;
                    text
                } else {
                    app.current_conversation()
                        .messages
                        .iter()
                        .rev()
                        .find(|(role, _)| role == "user")
                        .map(|(_, content)| content.clone())
                        .unwrap_or_default()
                };

                if input.is_empty() {
                    app.status_message = Some(
                        "No input provided. Type something first or have a previous message."
                            .into(),
                    );
                    return;
                }

                run_automation(app, &auto, &input, provider).await;
            }
            Err(_) => {
                app.status_message = Some(format!("Automation '{name}' not found"));
            }
        }
        return;
    }

    // Unknown /auto sub-command.
    app.status_message =
        Some("Unknown /auto command. Use: list, run, create, delete, info".into());
}

/// Execute an automation pipeline. Intermediate steps run non-streaming;
/// the final step streams to chat via the existing mechanism.
async fn run_automation(
    app: &mut App,
    automation: &automation::Automation,
    input: &str,
    provider: &Arc<dyn AiProvider>,
) {
    let start = std::time::Instant::now();
    let mut prev_output = String::new();
    let total_steps = automation.steps.len();

    app.status_message = Some(format!("Running automation: {}", automation.name));

    for (i, step) in automation.steps.iter().enumerate() {
        let model_owned = step
            .model
            .clone()
            .unwrap_or_else(|| app.selected_model.clone());
        let prompt = step
            .prompt_template
            .replace("{{input}}", input)
            .replace("{{prev_output}}", &prev_output);

        app.status_message = Some(format!(
            "Automation step {}/{}: {}...",
            i + 1,
            total_steps,
            step.name,
        ));

        if i == total_steps - 1 {
            // Last step: stream to chat using the existing streaming mechanism.
            app.add_user_message(format!(
                "[Automation: {} - Step {}/{}]",
                automation.name,
                i + 1,
                total_steps,
            ));
            app.scroll_offset = 0;

            let messages = vec![ChatMessage::user(&prompt)];
            let (tx, rx) = mpsc::unbounded_channel();

            app.stream_rx = Some(rx);
            app.is_streaming = true;
            app.streaming_response.clear();

            let provider = Arc::clone(provider);
            tokio::spawn(async move {
                if let Err(e) = provider
                    .chat_stream(&messages, &model_owned, tx.clone())
                    .await
                {
                    let _ = tx.send(StreamEvent::Error(e.to_string()));
                }
            });

            let elapsed = start.elapsed().as_millis();
            app.status_message = Some(format!(
                "Automation '{}' complete ({total_steps} steps, {elapsed}ms)",
                automation.name,
            ));
        } else {
            // Intermediate steps: non-streaming.
            let messages = vec![ChatMessage::user(&prompt)];
            match provider.chat(&messages, &model_owned).await {
                Ok(response) => prev_output = response,
                Err(e) => {
                    app.add_assistant_message(format!(
                        "Automation error at step {}: {e}",
                        i + 1,
                    ));
                    app.status_message =
                        Some(format!("Automation failed at step {}", i + 1));
                    return;
                }
            }
        }
    }
}

/// Build the messages array from conversation history and start streaming.
async fn send_to_ai(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    app.add_user_message(text.to_string());
    app.scroll_offset = 0;
    send_to_ai_from_history(app, provider).await;
}

/// Start a streaming AI request using the current conversation history.
/// Assumes the caller has already added the user message to the conversation.
async fn send_to_ai_from_history(app: &mut App, provider: &Arc<dyn AiProvider>) {
    // Find the most recent user message for KB context lookup.
    let user_message = app
        .current_conversation()
        .messages
        .iter()
        .rev()
        .find(|(role, _)| role == "user")
        .map(|(_, content)| content.clone())
        .unwrap_or_default();

    let mut messages: Vec<ChatMessage> = app
        .current_conversation()
        .messages
        .iter()
        .filter_map(|(role, content)| match role.as_str() {
            "user" => Some(ChatMessage::user(content)),
            "assistant" => Some(ChatMessage::assistant(content)),
            "system" => Some(ChatMessage::system(content)),
            _ => None,
        })
        .collect();

    // If a knowledge base exists, search for relevant context and inject it.
    if !user_message.is_empty() {
        if let Ok(kb) = knowledge::KnowledgeBase::load("default") {
            if !kb.chunks.is_empty() {
                let results = knowledge::search_knowledge(&kb, &user_message, 3);
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
                             to the user's query:\n\n{}\n\n\
                             Use this context to inform your response if relevant.",
                            context
                        )),
                    );
                }
            }
        }
    }

    let model = app.selected_model.clone();
    let (tx, rx) = mpsc::unbounded_channel();

    app.stream_rx = Some(rx);
    app.is_streaming = true;
    app.streaming_response.clear();

    let provider = Arc::clone(provider);
    tokio::spawn(async move {
        if let Err(e) = provider.chat_stream(&messages, &model, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error(e.to_string()));
        }
    });
}

/// Copy the last assistant message to the system clipboard.
fn copy_last_assistant_message(app: &mut App) {
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
                app.status_message = Some("Copied to clipboard".into());
            }
            Err(e) => {
                app.status_message = Some(format!("Clipboard error: {e}"));
            }
        },
        None => {
            app.status_message = Some("No assistant message to copy".into());
        }
    }
}

/// Clear the active conversation's messages and reset streaming state.
fn clear_conversation(app: &mut App) {
    app.current_conversation_mut().messages.clear();
    app.streaming_response.clear();
    app.is_streaming = false;
    app.stream_rx = None;
    app.scroll_offset = 0;
    app.status_message = Some("Conversation cleared".into());
}

/// Cycle to the next conversation (wraps around).
fn cycle_conversation(app: &mut App) {
    if app.conversations.len() > 1 {
        app.active_conversation = (app.active_conversation + 1) % app.conversations.len();
        app.scroll_offset = 0;
        app.status_message = Some(format!(
            "Switched to conversation {}",
            app.active_conversation + 1
        ));
    }
}
