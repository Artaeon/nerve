mod agent;
mod ai;
mod app;
mod automation;
mod clipboard;
mod clipboard_manager;
mod config;
mod daemon;
mod files;
mod history;
mod keybinds;
mod knowledge;
mod prompts;
mod scaffold;
mod scraper;
mod shell;
mod ui;
mod plugins;
mod session;
mod workspace;

use std::io::{self, Read as _};
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use ai::provider::{AiProvider, ChatMessage, StreamEvent};
use ai::{ClaudeCodeProvider, CopilotProvider, OpenAiProvider};
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

    /// Provider to use (claude_code, openai, ollama, openrouter, copilot)
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

    /// Resume the last session
    #[arg(short = 'c', long = "continue")]
    continue_session: bool,
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
    run_tui(provider, config, cli.continue_session).await
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
        "copilot" | "gh" => {
            Ok(Box::new(CopilotProvider::new()))
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

async fn run_tui(provider: Arc<dyn AiProvider>, config: Config, continue_session: bool) -> anyhow::Result<()> {
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

    // Auto-detect workspace and inject system prompt with project context.
    if let Some(ws) = workspace::detect_workspace() {
        let sys_prompt = ws.to_system_prompt();
        app.current_conversation_mut()
            .messages
            .insert(0, ("system".into(), sys_prompt));
        app.status_message = Some(format!(
            "Detected {:?} project: {}",
            ws.project_type, ws.name
        ));
    }

    // Load plugins from ~/.config/nerve/plugins/
    let loaded_plugins = plugins::load_plugins();
    if !loaded_plugins.is_empty() {
        app.set_status(format!("{} plugin(s) loaded", loaded_plugins.len()));
    }
    app.plugins = loaded_plugins;

    // Restore previous session if --continue was passed.
    if continue_session {
        match session::load_last_session() {
            Ok(sess) => {
                session::restore_session_to_app(&sess, &mut app);
                app.set_status(format!("Resumed session ({} conversation(s))", app.conversations.len()));
            }
            Err(_) => {
                app.set_status("No previous session found");
            }
        }
    }

    let result = event_loop(&mut terminal, &mut app, &provider, &config).await;

    // Auto-save session on every quit path.
    let sess = session::session_from_app(&app);
    let _ = session::save_session(&sess);

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
        // Auto-clear status messages after 5 seconds.
        if let Some(time) = app.status_time {
            if time.elapsed() > std::time::Duration::from_secs(5) {
                app.status_message = None;
                app.status_time = None;
            }
        }

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
                Event::Resize(_, _) => {
                    // Terminal resized — the next draw call will pick up the new
                    // dimensions automatically.  Nothing explicit to do here.
                }
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


                            // Agent mode: check for tool calls and execute them
                            if app.agent_mode {
                                let last_response = app
                                    .current_conversation()
                                    .messages
                                    .last()
                                    .filter(|(r, _)| r == "assistant")
                                    .map(|(_, c)| c.clone());

                                if let Some(response) = last_response {
                                    let tool_calls =
                                        crate::agent::tools::parse_tool_calls(&response);

                                    if !tool_calls.is_empty()
                                        && app.agent_iterations < 10
                                    {
                                        app.agent_iterations += 1;
                                        app.set_status(format!(
                                            "Agent executing {} tool(s)... (iteration {})",
                                            tool_calls.len(),
                                            app.agent_iterations
                                        ));

                                        // Execute tools and build results message
                                        let mut results =
                                            String::from("Tool execution results:\n\n");
                                        for call in &tool_calls {
                                            let result =
                                                crate::agent::tools::execute_tool(call);
                                            let status = if result.success {
                                                "SUCCESS"
                                            } else {
                                                "FAILED"
                                            };
                                            results.push_str(&format!(
                                                "<tool_result>\ntool: {}\nstatus: {status}\noutput:\n{}\n</tool_result>\n\n",
                                                result.tool, result.output
                                            ));
                                        }

                                        // Add tool results as a user message
                                        app.add_user_message(results);

                                        // Apply context management based on provider
                                        let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
                                        let context_mgr =
                                            crate::agent::context::ContextManager::new(
                                                limit,
                                            );

                                        // First compact tool results, then overall conversation
                                        let tool_compacted = context_mgr.compact_tool_results(
                                            &app.current_conversation().messages,
                                        );
                                        let compacted = context_mgr.compact_messages(
                                            &tool_compacted,
                                        );
                                        let messages: Vec<ChatMessage> = compacted
                                            .iter()
                                            .filter_map(|(role, content)| {
                                                match role.as_str() {
                                                    "user" => {
                                                        Some(ChatMessage::user(content))
                                                    }
                                                    "assistant" => {
                                                        Some(ChatMessage::assistant(
                                                            content,
                                                        ))
                                                    }
                                                    "system" => {
                                                        Some(ChatMessage::system(content))
                                                    }
                                                    _ => None,
                                                }
                                            })
                                            .collect();

                                        // Trigger another AI call
                                        let model = app.selected_model.clone();
                                        let (tx, new_rx) =
                                            tokio::sync::mpsc::unbounded_channel();
                                        app.stream_rx = Some(new_rx);
                                        app.is_streaming = true;
                                        app.streaming_response.clear();
                                        app.streaming_start =
                                            Some(std::time::Instant::now());

                                        let provider_clone = Arc::clone(&provider);
                                        tokio::spawn(async move {
                                            if let Err(e) = provider_clone
                                                .chat_stream(
                                                    &messages,
                                                    &model,
                                                    tx.clone(),
                                                )
                                                .await
                                            {
                                                let _ = tx.send(
                                                    StreamEvent::Error(e.to_string()),
                                                );
                                            }
                                        });

                                        // Do not mark finished; loop continues
                                        // with the new stream receiver
                                        break;
                                    } else if app.agent_iterations > 0 {
                                        // No more tool calls or max iterations
                                        app.set_status(format!(
                                            "Agent completed in {} iteration(s)",
                                            app.agent_iterations
                                        ));
                                        app.agent_iterations = 0;
                                    }
                                }
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
    config: &Config,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    // Escape while streaming = stop generation
    if app.is_streaming && code == KeyCode::Esc {
        app.finish_streaming();
        app.set_status("Generation stopped");
        return Ok(());
    }

    // ── Global keys (always active) ─────────────────────────────────────
    if mods.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('c') | KeyCode::Char('d') => {
                // Graceful shutdown: save state before quitting.
                let _ = app.clipboard_manager.save();
                // Save current conversation if it has messages.
                if !app.current_conversation().messages.is_empty() {
                    let conv = app.current_conversation();
                    let record = history::ConversationRecord {
                        id: conv.id.clone(),
                        title: conv.title.clone(),
                        messages: conv
                            .messages
                            .iter()
                            .map(|(role, content)| history::MessageRecord {
                                role: role.clone(),
                                content: content.clone(),
                                timestamp: chrono::Utc::now(),
                            })
                            .collect(),
                        model: app.selected_model.clone(),
                        created_at: conv.created_at,
                        updated_at: chrono::Utc::now(),
                    };
                    let _ = history::save_conversation(&record);
                }
                // Drop the stream receiver to stop any in-progress generation.
                app.stream_rx = None;
                app.is_streaming = false;
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
            KeyCode::Char('f') => {
                app.mode = AppMode::SearchOverlay;
                app.search_query.clear();
                app.search_results.clear();
                app.search_current = 0;
                return Ok(());
            }
            _ => {}
        }
    }

    // ── Dispatch by mode ────────────────────────────────────────────────
    match app.mode {
        AppMode::Normal => handle_normal_mode(app, key, provider, config).await?,
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
        AppMode::SearchOverlay => handle_search(app, key),
    }

    Ok(())
}

// ── Common Ctrl handler ────────────────────────────────────────────────────

/// Handle Ctrl+<key> commands that work in both Normal and Insert modes.
/// Returns `true` if the key was handled.
async fn handle_common_ctrl(
    app: &mut App,
    code: KeyCode,
    provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<bool> {
    match code {
        KeyCode::Char('k') => {
            app.mode = AppMode::CommandBar;
            app.command_bar_input.clear();
            app.command_bar_select_index = 0;
            app.command_bar_category = 0;
            Ok(true)
        }
        KeyCode::Char('n') => {
            app.new_conversation();
            Ok(true)
        }
        KeyCode::Char('p') => {
            app.mode = AppMode::PromptPicker;
            app.prompt_filter.clear();
            app.prompt_select_index = 0;
            app.prompt_category_index = 0;
            app.prompt_focus_right = false;
            Ok(true)
        }
        KeyCode::Char('m') => {
            app.mode = AppMode::ModelSelect;
            app.model_select_index = app
                .available_models
                .iter()
                .position(|m| m == &app.selected_model)
                .unwrap_or(0);
            Ok(true)
        }
        KeyCode::Char('t') => {
            app.mode = AppMode::ProviderSelect;
            app.provider_select_index = app
                .available_providers
                .iter()
                .position(|p| p == &app.selected_provider)
                .unwrap_or(0);
            Ok(true)
        }
        KeyCode::Char('y') => {
            copy_last_assistant_message(app);
            Ok(true)
        }
        KeyCode::Char('l') => {
            clear_conversation(app);
            Ok(true)
        }
        KeyCode::Char('b') => {
            app.mode = AppMode::ClipboardManager;
            app.clipboard_search.clear();
            app.clipboard_select_index = 0;
            Ok(true)
        }
        KeyCode::Char('o') => {
            app.history_entries =
                history::list_conversations().unwrap_or_default();
            app.history_select_index = 0;
            app.history_search.clear();
            app.mode = AppMode::HistoryBrowser;
            Ok(true)
        }
        KeyCode::Char('r') => {
            regenerate_response(app, provider, config).await;
            Ok(true)
        }
        KeyCode::Char('e') => {
            edit_last_message(app);
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Normal mode ─────────────────────────────────────────────────────────────

async fn handle_normal_mode(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    provider: &Arc<dyn AiProvider>,
    config: &Config,
) -> anyhow::Result<()> {
    let code = key.code;
    let mods = key.modifiers;

    match app.input_mode {
        // ── Normal / vim-navigation ─────────────────────────────────────
        InputMode::Normal => {
            if mods.contains(KeyModifiers::CONTROL) {
                if handle_common_ctrl(app, code, provider, config).await? {
                    return Ok(());
                }
                // No mode-specific Ctrl keys in Normal mode.
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
                KeyCode::BackTab => cycle_conversation_back(app),
                KeyCode::Char('q') => app.should_quit = true,
                KeyCode::Char('x') => delete_last_exchange(app),
                _ => {}
            }
        }

        // ── Insert / typing mode ────────────────────────────────────────
        InputMode::Insert => {
            if mods.contains(KeyModifiers::CONTROL) {
                if handle_common_ctrl(app, code, provider, config).await? {
                    return Ok(());
                }
                // Insert-mode-specific Ctrl keys.
                match code {
                    KeyCode::Char('v') => {
                        if let Ok(text) = clipboard::paste_from_clipboard() {
                            for ch in text.chars() {
                                app.insert_char(ch);
                            }
                        }
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
                KeyCode::Tab => {
                    if app.input.starts_with('/') {
                        let partial = &app.input[1..]; // strip the /
                        let commands = [
                            "help", "clear", "new", "model", "models", "provider",
                            "providers", "code", "cwd", "url", "kb", "auto", "status",
                            "export", "rename", "system", "workspace", "run",
                            "pipe", "diff", "test", "build", "git",
                            "agent", "cd", "summary", "compact", "context", "tokens",
                            "branch", "session",
                        ];
                        let matches: Vec<&&str> = commands
                            .iter()
                            .filter(|cmd| cmd.starts_with(partial))
                            .collect();

                        if matches.len() == 1 {
                            // Exact completion
                            app.input = format!("/{} ", matches[0]);
                            app.cursor_position = app.input.len();
                        } else if matches.len() > 1 {
                            // Show options in status
                            let options = matches
                                .iter()
                                .map(|c| format!("/{c}"))
                                .collect::<Vec<_>>()
                                .join("  ");
                            app.status_message = Some(options);

                            // Complete common prefix
                            let common = common_prefix(&matches);
                            if common.len() > partial.len() {
                                app.input = format!("/{common}");
                                app.cursor_position = app.input.len();
                            }
                        }
                    }
                }
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
                app.set_status(format!("Model set to {model}"));
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
                app.set_status(format!("Provider switched to {}", provider_name));
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

// ── Search overlay ─────────────────────────────────────────────────────────

fn handle_search(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            // Jump to next match
            if !app.search_results.is_empty() {
                app.search_current = (app.search_current + 1) % app.search_results.len();
                app.status_message = Some(format!(
                    "Match {}/{}",
                    app.search_current + 1,
                    app.search_results.len()
                ));
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            update_search_results(app);
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            update_search_results(app);
        }
        _ => {}
    }
}

fn update_search_results(app: &mut App) {
    let query = app.search_query.to_lowercase();
    if query.is_empty() {
        app.search_results.clear();
        return;
    }
    app.search_results = app
        .current_conversation()
        .messages
        .iter()
        .enumerate()
        .filter(|(_, (_, content))| content.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
    app.search_current = 0;
}

/// Find the longest common prefix among a set of string slices.
fn common_prefix(strings: &[&&str]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = strings[0];
    let mut prefix_len = first.len();
    for s in &strings[1..] {
        prefix_len = first
            .chars()
            .zip(s.chars())
            .take_while(|(a, b)| a == b)
            .count()
            .min(prefix_len);
    }
    first[..prefix_len].to_string()
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Submit the user's message and spawn a streaming response task.
///
/// Handles slash commands (`/help`, `/clear`, `/new`, `/model`, `/models`,
/// `/url`, `/run`, `/pipe`, `/diff`, `/test`, `/build`, `/git`) before
/// falling through to the normal AI chat path.
async fn submit_message(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) {
    if app.is_streaming {
        app.set_status("Already streaming \u{2014} press Esc to cancel first");
        return;
    }

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
  /delete             Delete current conversation\n\
  /delete all         Delete all conversations\n\
  /rename <title>     Rename current conversation\n\
  /export             Export conversation to markdown\n\
  /copy               Copy last AI response to clipboard\n\
  /copy all           Copy entire conversation\n\
  /copy last          Copy last message (any role)\n\
  /system <prompt>    Set system prompt for conversation\n\
  /system             Show current system prompt\n\
  /system clear       Remove system prompt\n\
\n\
AI Provider\n\
  /provider <name>    Switch provider\n\
  /providers          List available providers\n\
  /model <name>       Switch model\n\
  /models             List available models\n\
  /code on|off        Toggle code mode (Claude Code only)\n\
  /agent on|off       Toggle agent mode (AI tool loop)\n\
  /cwd <dir>          Set working directory\n\
  /cd <dir>           Change working directory\n\
\n\
Knowledge & Context\n\
  /file <path>        Read file as context\n\
  /file <path>:S-E    Read specific line range\n\
  /files <p1> <p2>    Read multiple files\n\
  /summary            Summarize current conversation\n\
  /compact            Compact conversation (save tokens)\n\
  /context            Show current AI context window\n\
  /tokens             Show token usage breakdown\n\
  /kb add <dir>       Add directory to knowledge base\n\
  /kb search <query>  Search knowledge base\n\
  /kb list            List knowledge bases\n\
  /kb status          Show KB statistics\n\
  /kb clear           Clear knowledge base\n\
  /url <url> [q]      Scrape URL for context\n\
\n\
Shell & Git\n\
  /run <command>      Run shell command and show output\n\
  /pipe <command>     Run command and add output as context\n\
  /diff [args]        Show git diff (adds as context)\n\
  /test               Auto-detect and run project tests\n\
  /build              Auto-detect and run project build\n\
  /git [subcommand]   Quick git operations (status/log/diff/branch)\n\
\n\
Project Scaffolding\n\
  /template list      List available project templates\n\
  /template <name>    Create project from template\n\
  /scaffold <desc>    AI-generate a project from description\n\
\n\
Automation\n\
  /auto list          List automations\n\
  /auto run <name>    Run automation\n\
  /auto info <name>   Show automation details\n\
  /auto create <name> Create custom automation\n\
  /auto delete <name> Delete custom automation\n\
\n\
Keybindings\n\
  Esc (streaming)     Stop generation\n\
  Ctrl+R              Regenerate last response\n\
  Ctrl+E              Edit last message\n\
  Ctrl+F              Search in conversation\n\
  Tab                 Next conversation\n\
  Shift+Tab           Previous conversation\n\
  x (Normal mode)     Delete last exchange\n\
\n\
Plugins\n\
  /plugin list        List installed plugins\n\
  /plugin init        Create example plugin\n\
  /plugin reload      Reload all plugins\n\
\n\
Sessions\n\
  /session            Show session info\n\
  /session save       Save current session\n\
  /session list       List saved sessions\n\
  /session restore    Restore last session\n\
  nerve --continue    Resume last session on startup\n\
\n\
Branching\n\
  /branch save [name] Save conversation branch point\n\
  /branch list        List saved branches\n\
  /branch restore <n> Restore a saved branch\n\
  /branch delete <n>  Delete a branch\n\
  /branch diff <n>    Compare current with a branch\n\
  /br                 Shorthand for /branch\n\
\n\
Workspace\n\
  /workspace          Show detected project info\n\
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
                app.set_status(format!("Model set to {model}"));
            }
            None => {
                let available = app.available_models.join(", ");
                app.set_status(format!("Unknown model: {name}. Available: {available}"));
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
             {} openrouter  - OpenRouter (requires OPENROUTER_API_KEY)\n\
             {} copilot     - GitHub Copilot (requires gh CLI with Copilot extension)\n\n\
             Current: {}\n\
             Switch with: /provider <name> or Ctrl+T",
            if current == "claude_code" || current == "claude" { "*" } else { " " },
            if current == "ollama" { "*" } else { " " },
            if current == "openai" { "*" } else { " " },
            if current == "openrouter" { "*" } else { " " },
            if current == "copilot" || current == "gh" { "*" } else { " " },
            current
        );
        app.add_assistant_message(list);
        app.scroll_offset = 0;
        return true;
    }

    // /provider — bare command shows current provider.
    if trimmed == "/provider" {
        app.add_assistant_message(format!(
            "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter, copilot",
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
                "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter, copilot",
                app.selected_provider
            ));
            app.scroll_offset = 0;
            return true;
        }
        let valid = ["claude_code", "claude", "ollama", "openai", "openrouter", "copilot", "gh"];
        if valid.contains(&name) {
            app.selected_provider = name.to_string();
            app.provider_changed = true;
            app.set_status(format!("Switched to provider: {name}"));
        } else {
            app.add_assistant_message(format!(
                "Unknown provider: {name}\nAvailable: claude_code, ollama, openai, openrouter, copilot"
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

    // /agent — toggle agent mode (AI can use tools in a loop).
    if trimmed == "/agent" || trimmed.starts_with("/agent ") {
        let rest = trimmed.strip_prefix("/agent").unwrap_or("").trim();
        match rest {
            "on" => {
                app.agent_mode = true;
                // Inject tools system prompt
                let tools_prompt = crate::agent::tools::tools_system_prompt();
                app.current_conversation_mut()
                    .messages
                    .retain(|(r, c)| {
                        !(r == "system"
                            && c.contains("You have access to the following tools"))
                    });
                app.current_conversation_mut()
                    .messages
                    .insert(0, ("system".into(), tools_prompt));
                app.set_status(
                    "Agent mode ON \u{2014} AI can read/write files, run commands",
                );
            }
            "off" => {
                app.agent_mode = false;
                app.current_conversation_mut()
                    .messages
                    .retain(|(r, c)| {
                        !(r == "system"
                            && c.contains("You have access to the following tools"))
                    });
                app.set_status("Agent mode OFF \u{2014} chat only");
            }
            _ => {
                let status = if app.agent_mode { "ON" } else { "OFF" };
                app.add_assistant_message(format!(
                    "Agent mode: {status}\n\n\
                     When ON, the AI can:\n\
                     - Read and write files\n\
                     - Run shell commands\n\
                     - Search code\n\
                     - Create directories\n\n\
                     Usage: /agent on | /agent off"
                ));
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // /cd — change working directory.
    if trimmed == "/cd" || trimmed.starts_with("/cd ") {
        let rest = trimmed.strip_prefix("/cd").unwrap_or("").trim();
        if rest.is_empty() {
            let cwd = std::env::current_dir().unwrap_or_default();
            app.add_assistant_message(format!(
                "Current directory: {}",
                cwd.display()
            ));
        } else {
            let target = rest;
            let target_path = if target.starts_with("~/") {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(&target[2..])
            } else {
                std::path::PathBuf::from(target)
            };

            match std::env::set_current_dir(&target_path) {
                Ok(()) => {
                    app.set_status(format!(
                        "Changed to {}",
                        target_path.display()
                    ));
                    // Re-detect workspace
                    if let Some(ws) = crate::workspace::detect_workspace() {
                        app.set_status(format!(
                            "Changed to {} \u{2014} detected {:?} project: {}",
                            target_path.display(),
                            ws.project_type,
                            ws.name
                        ));
                    }
                }
                Err(e) => app.set_status(format!("Error: {e}")),
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // /code — toggle code mode (tool/file access for Claude Code).
    if trimmed == "/code" || trimmed.starts_with("/code ") {
        let rest = trimmed.strip_prefix("/code").unwrap_or("").trim();
        if rest == "on" {
            if app.selected_provider == "claude_code" || app.selected_provider == "claude" {
                app.code_mode = true;
                app.provider_changed = true;
                app.set_status("Code mode ON - Claude has file & terminal access");
            } else {
                app.add_assistant_message(
                    "Code mode is only available with the claude_code provider.".into(),
                );
            }
        } else if rest == "off" {
            app.code_mode = false;
            app.provider_changed = true;
            app.set_status("Code mode OFF - chat only");
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

    // /file <path> — read a file and add it as context.
    // /file <path>:<start>-<end> — read specific line range.
    if trimmed == "/file" || trimmed.starts_with("/file ") {
        let rest = trimmed.strip_prefix("/file").unwrap_or("").trim();
        if rest.is_empty() {
            app.add_assistant_message(
                "Usage: /file <path> or /file <path>:<start>-<end>\n\
                 Reads a file and adds it as context for the AI."
                    .into(),
            );
            app.scroll_offset = 0;
            return true;
        }
        let path_arg = rest;

        // Check for line range: path:10-20
        let result = if let Some((path, range)) = path_arg.split_once(':') {
            if let Some((start, end)) = range.split_once('-') {
                let s: usize = start.parse().unwrap_or(1);
                let e: usize = end.parse().unwrap_or(usize::MAX);
                files::read_file_range(path, s, e)
            } else {
                files::read_file_context(path)
            }
        } else {
            files::read_file_context(path_arg)
        };

        match result {
            Ok(fc) => {
                let formatted = files::format_file_for_context(&fc);
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), formatted));
                app.status_message = Some(format!("Added {} ({} lines)", fc.path, fc.line_count));
            }
            Err(e) => {
                app.status_message = Some(format!("Error: {e}"));
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // /files <path1> <path2> ... — read multiple files as context.
    if trimmed == "/files" || trimmed.starts_with("/files ") {
        let rest = trimmed.strip_prefix("/files").unwrap_or("").trim();
        if rest.is_empty() {
            app.add_assistant_message("Usage: /files <path1> <path2> ...".into());
            app.scroll_offset = 0;
            return true;
        }
        let paths: Vec<&str> = rest.split_whitespace().collect();
        let mut added = 0;
        for path in &paths {
            match files::read_file_context(path) {
                Ok(fc) => {
                    let formatted = files::format_file_for_context(&fc);
                    app.current_conversation_mut()
                        .messages
                        .push(("system".into(), formatted));
                    added += 1;
                }
                Err(e) => {
                    app.status_message = Some(format!("Error reading {path}: {e}"));
                }
            }
        }
        if added > 0 {
            app.status_message = Some(format!("Added {added} file(s) as context"));
        }
        app.scroll_offset = 0;
        return true;
    }

    // /template — project templates.
    if trimmed == "/template" || trimmed.starts_with("/template ") {
        let rest = trimmed.strip_prefix("/template").unwrap_or("").trim();
        let args: Vec<&str> = rest.split_whitespace().collect();

        if args.is_empty() || args[0] == "list" {
            let templates = scaffold::list_templates();
            let mut msg = "Available templates:\n\n".to_string();
            for (name, lang, desc) in &templates {
                msg.push_str(&format!("  {name:<20} [{lang}] {desc}\n"));
            }
            msg.push_str("\nUsage: /template <name> [project-name]");
            app.add_assistant_message(msg);
            app.scroll_offset = 0;
            return true;
        }

        let template_name = args[0];
        let project_name = args
            .get(1)
            .copied()
            .unwrap_or(template_name)
            .to_string();

        match scaffold::get_template(template_name) {
            Some(mut template) => {
                // Replace placeholders
                for file in &mut template.files {
                    file.content = file.content.replace("{{name}}", &project_name);
                    file.content = file.content
                        .replace("{{description}}", &format!("A {project_name} project"));
                }

                let target = std::env::current_dir()
                    .unwrap_or_default()
                    .join(&project_name);
                match scaffold::write_template(&template, &target) {
                    Ok(count) => {
                        let next_steps = match template.language.as_str() {
                            "Rust" => "cargo build\ncargo run\n",
                            "Node.js" | "React" => "npm install\nnpm start\n",
                            "Python" => "pip install -e .\npython -m app\n",
                            "Go" => "go build\ngo run .\n",
                            _ => "",
                        };
                        app.add_assistant_message(format!(
                            "Created {} project '{}' at {}\n\
                             {} files written.\n\n\
                             Next steps:\n```\ncd {}\n{}```",
                            template.language,
                            project_name,
                            target.display(),
                            count,
                            project_name,
                            next_steps,
                        ));
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Error: {e}"));
                    }
                }
            }
            None => {
                app.add_assistant_message(format!(
                    "Template '{}' not found. Use /template list to see available templates.",
                    template_name
                ));
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // /scaffold — AI-powered project scaffolding.
    if trimmed == "/scaffold" || trimmed.starts_with("/scaffold ") {
        let rest = trimmed.strip_prefix("/scaffold").unwrap_or("").trim();
        if rest.is_empty() {
            app.add_assistant_message(
                "Usage: /scaffold <project description>\n\n\
                 Example: /scaffold a REST API in Rust with JWT auth and PostgreSQL\n\n\
                 This uses Claude Code to generate a complete project structure.\n\
                 Note: requires /code on for file creation."
                    .into(),
            );
            app.scroll_offset = 0;
            return true;
        }

        let description = rest.to_string();
        let prompt = format!(
            "Create a complete, production-ready project structure for: {description}\n\n\
             Requirements:\n\
             - Create all necessary files and directories\n\
             - Include proper package/build configuration\n\
             - Include a README.md with setup instructions\n\
             - Include a .gitignore\n\
             - Include basic tests\n\
             - Follow best practices for the chosen language/framework\n\
             - Make it immediately runnable after setup\n\
             - Use modern, up-to-date dependencies"
        );

        // Submit this as a regular message to the AI
        app.add_user_message(prompt);
        app.scroll_offset = 0;
        send_to_ai_from_history(app, provider).await;
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

    // /workspace (or /ws) — show detected workspace info.
    if trimmed == "/workspace" || trimmed == "/ws" {
        match workspace::detect_workspace() {
            Some(ws) => {
                let info = format!(
                    "Workspace detected:\n\n\
                     Project: {}\n\
                     Type: {:?}\n\
                     Root: {}\n\
                     Tech: {}\n\
                     Files: {}",
                    ws.name,
                    ws.project_type,
                    ws.root.display(),
                    ws.tech_stack.join(", "),
                    ws.key_files.join(", ")
                );
                app.add_assistant_message(info);
            }
            None => {
                app.add_assistant_message("No project detected in current directory.".into());
            }
        }
        app.scroll_offset = 0;
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

    // /system — set, show, or clear a custom system prompt.
    if trimmed == "/system" || trimmed.starts_with("/system ") {
        let rest = trimmed.strip_prefix("/system").unwrap_or("").trim();
        if rest.is_empty() {
            // Show current system prompt
            let sys = app.current_conversation().messages.iter()
                .find(|(r, _)| r == "system")
                .map(|(_, c)| c.clone());
            match sys {
                Some(prompt) => app.add_assistant_message(format!("Current system prompt:\n\n{prompt}")),
                None => app.add_assistant_message("No system prompt set. Use /system <prompt> to set one.".into()),
            }
        } else if rest == "clear" {
            app.current_conversation_mut().messages.retain(|(r, _)| r != "system");
            app.status_message = Some("System prompt cleared".into());
        } else {
            let prompt = rest.to_string();
            // Remove any existing system prompt
            app.current_conversation_mut().messages.retain(|(r, _)| r != "system");
            // Insert at the beginning
            app.current_conversation_mut().messages.insert(0, ("system".into(), prompt));
            app.status_message = Some("System prompt set".into());
        }
        app.scroll_offset = 0;
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


    // /delete — delete current conversation (or all).
    if trimmed == "/delete" || trimmed.starts_with("/delete ") {
        let rest = trimmed.strip_prefix("/delete").unwrap_or("").trim();
        if rest == "all" {
            app.conversations.clear();
            app.conversations.push(app::Conversation::new());
            app.active_conversation = 0;
            app.scroll_offset = 0;
            app.status_message = Some("All conversations deleted".into());
        } else {
            if app.conversations.len() <= 1 {
                // Last conversation — clear it instead
                app.current_conversation_mut().messages.clear();
                app.current_conversation_mut().title = "New Conversation".into();
                app.status_message = Some("Conversation cleared".into());
            } else {
                app.conversations.remove(app.active_conversation);
                if app.active_conversation >= app.conversations.len() {
                    app.active_conversation = app.conversations.len() - 1;
                }
                app.scroll_offset = 0;
                app.status_message = Some("Conversation deleted".into());
            }
        }
        return true;
    }

    // /copy — copy messages to clipboard.
    if trimmed == "/copy" || trimmed.starts_with("/copy ") {
        let rest = trimmed.strip_prefix("/copy").unwrap_or("").trim();
        let conv = app.current_conversation();
        let text = match rest {
            "all" => {
                conv.messages
                    .iter()
                    .map(|(role, content)| format!("{}: {}", role, content))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
            "last" => conv
                .messages
                .last()
                .map(|(_, c)| c.clone())
                .unwrap_or_default(),
            _ => {
                // Default: last AI response
                conv.messages
                    .iter()
                    .rev()
                    .find(|(r, _)| r == "assistant")
                    .map(|(_, c)| c.clone())
                    .unwrap_or_default()
            }
        };
        if text.is_empty() {
            app.status_message = Some("Nothing to copy".into());
        } else {
            match clipboard::copy_to_clipboard(&text) {
                Ok(()) => app.status_message = Some("Copied to clipboard".into()),
                Err(e) => app.status_message = Some(format!("Clipboard error: {e}")),
            }
        }
        return true;
    }

    // ── Shell commands ────────────────────────────────────────────────────

    // /run <command> (or /! <command>) — run a shell command and show output.
    if trimmed.starts_with("/run ") || trimmed.starts_with("/! ") {
        let rest = if let Some(r) = trimmed.strip_prefix("/run ") {
            r.trim()
        } else {
            trimmed.strip_prefix("/! ").unwrap_or("").trim()
        };
        if rest.is_empty() {
            app.add_assistant_message(
                "Usage: /run <command>\nExecutes a shell command and shows the output.".into(),
            );
            return true;
        }
        let cmd = rest.to_string();
        if is_dangerous_command(&cmd) {
            app.set_status("Blocked: this command looks dangerous. Use your terminal directly.");
            return true;
        }
        app.status_message = Some(format!("Running: {cmd}"));
        match shell::run_command(&cmd) {
            Ok(result) => {
                let output = shell::format_command_output(&result);
                app.add_assistant_message(output);
            }
            Err(e) => {
                app.status_message = Some(format!("Error: {e}"));
            }
        }
        return true;
    }

    // /pipe <command> — run command and add output as context.
    if trimmed.starts_with("/pipe ") {
        let rest = trimmed.strip_prefix("/pipe ").unwrap_or("").trim();
        if rest.is_empty() {
            app.add_assistant_message(
                "Usage: /pipe <command>\nRuns a command and adds its output as context.".into(),
            );
            return true;
        }
        let cmd = rest.to_string();
        app.status_message = Some(format!("Running: {cmd}"));
        match shell::run_command(&cmd) {
            Ok(result) => {
                let context = shell::format_command_for_context(&result);
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context));
                app.status_message = Some(format!(
                    "Added output of '{}' as context ({} lines)",
                    cmd,
                    result.stdout.lines().count()
                ));
            }
            Err(e) => {
                app.status_message = Some(format!("Error: {e}"));
            }
        }
        return true;
    }

    // /diff [args] — show git diff and add as context.
    if trimmed == "/diff" || trimmed.starts_with("/diff ") {
        let diff_args = trimmed
            .strip_prefix("/diff")
            .unwrap_or("")
            .trim()
            .to_string();
        match shell::git_diff(&diff_args) {
            Ok(result) => {
                if result.stdout.trim().is_empty() {
                    app.add_assistant_message("No changes detected (git diff is empty).".into());
                } else {
                    let label = if diff_args.is_empty() {
                        String::new()
                    } else {
                        format!(" {diff_args}")
                    };
                    let context =
                        format!("Git diff{}:\n\n```diff\n{}\n```", label, result.stdout);
                    app.current_conversation_mut()
                        .messages
                        .push(("system".into(), context));
                    app.add_assistant_message(format!(
                        "Diff loaded ({} lines). Ask me anything about it.",
                        result.stdout.lines().count()
                    ));
                }
            }
            Err(e) => app.status_message = Some(format!("Error: {e}")),
        }
        return true;
    }

    // /test — auto-detect and run project tests.
    if trimmed == "/test" {
        let cmd = shell::detect_test_command();
        app.status_message = Some(format!("Running: {cmd}"));
        match shell::run_command(cmd) {
            Ok(result) => {
                let output = shell::format_command_output(&result);
                let context = shell::format_command_for_context(&result);
                app.current_conversation_mut()
                    .messages
                    .push(("system".into(), context));
                app.add_assistant_message(output);
                if !result.success {
                    app.status_message =
                        Some("Tests FAILED \u{2014} ask me to help fix them".into());
                } else {
                    app.status_message = Some("Tests passed".into());
                }
            }
            Err(e) => app.status_message = Some(format!("Error: {e}")),
        }
        return true;
    }

    // /build — auto-detect and run project build.
    if trimmed == "/build" {
        let cmd = shell::detect_build_command();
        app.status_message = Some(format!("Running: {cmd}"));
        match shell::run_command(cmd) {
            Ok(result) => {
                let output = shell::format_command_output(&result);
                if !result.success {
                    let context = shell::format_command_for_context(&result);
                    app.current_conversation_mut()
                        .messages
                        .push(("system".into(), context));
                    app.add_assistant_message(output);
                    app.status_message =
                        Some("Build FAILED \u{2014} ask me to help fix it".into());
                } else {
                    app.add_assistant_message(output);
                    app.status_message = Some("Build succeeded".into());
                }
            }
            Err(e) => app.status_message = Some(format!("Error: {e}")),
        }
        return true;
    }

    // /git [subcommand] — quick git operations.
    if trimmed == "/git" || trimmed.starts_with("/git ") {
        let rest = trimmed.strip_prefix("/git").unwrap_or("").trim();
        let args: Vec<&str> = rest.split_whitespace().collect();
        let subcmd = args.first().copied().unwrap_or("status");
        let cmd = match subcmd {
            "status" | "s" => "git status --short".to_string(),
            "log" | "l" => {
                let n = args
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(10);
                format!("git log --oneline -{n}")
            }
            "diff" | "d" => "git diff".to_string(),
            "branch" | "b" => "git branch -a".to_string(),
            _ => format!("git {rest}"),
        };
        match shell::run_command(&cmd) {
            Ok(result) => {
                let output = shell::format_command_output(&result);
                app.add_assistant_message(output);
            }
            Err(e) => app.status_message = Some(format!("Error: {e}")),
        }
        return true;
    }

    // /session — session management (save, list, restore, info).
    if trimmed == "/session" || trimmed.starts_with("/session ") {
        let rest = trimmed.strip_prefix("/session").unwrap_or("").trim();
        let args: Vec<&str> = rest.split_whitespace().collect();
        let subcmd = args.first().copied().unwrap_or("info");
        match subcmd {
            "save" => {
                let sess = session::session_from_app(app);
                match session::save_session(&sess) {
                    Ok(()) => app.set_status("Session saved"),
                    Err(e) => app.set_status(format!("Error: {e}")),
                }
            }
            "list" => {
                match session::list_sessions() {
                    Ok(sessions) => {
                        if sessions.is_empty() {
                            app.add_assistant_message("No saved sessions.".into());
                        } else {
                            let mut msg = "Saved sessions:\n\n".to_string();
                            for (id, date, count) in &sessions {
                                msg.push_str(&format!("  {} — {} ({} conv)\n",
                                    &id[..8], date.format("%Y-%m-%d %H:%M"), count));
                            }
                            msg.push_str("\nResume with: nerve --continue");
                            app.add_assistant_message(msg);
                        }
                    }
                    Err(e) => app.set_status(format!("Error: {e}")),
                }
            }
            "restore" => {
                match session::load_last_session() {
                    Ok(sess) => {
                        session::restore_session_to_app(&sess, app);
                        app.set_status(format!("Session restored ({} conversations)", app.conversations.len()));
                    }
                    Err(e) => app.set_status(format!("Error: {e}")),
                }
            }
            _ => {
                let sess_info = format!(
                    "Session Info:\n  Conversations: {}\n  Active: {}\n  Model: {}\n  Provider: {}\n\nCommands:\n  /session save     Save current session\n  /session list     List saved sessions\n  /session restore  Restore last session\n  nerve --continue  Resume on startup",
                    app.conversations.len(),
                    app.current_conversation().title,
                    app.selected_model,
                    app.selected_provider
                );
                app.add_assistant_message(sess_info);
            }
        }
        return true;
    }

    // /summary — generate a summary of the current conversation.
    if trimmed == "/summary" {
        let conv = app.current_conversation();
        if conv.messages.is_empty() {
            app.add_assistant_message("No messages to summarize.".into());
            return true;
        }

        let mut summary = format!("Conversation Summary: {}\n", conv.title);
        summary.push_str(&format!("{}\n\n", "=".repeat(40)));

        let user_count = conv.messages.iter().filter(|(r, _)| r == "user").count();
        let ai_count = conv.messages.iter().filter(|(r, _)| r == "assistant").count();
        let total_words: usize = conv.messages.iter().map(|(_, c)| c.split_whitespace().count()).sum();
        let total_tokens = conv.messages.iter().map(|(_, c)| c.len() / 4 + 1).sum::<usize>();

        summary.push_str(&format!("Messages: {} user, {} AI\n", user_count, ai_count));
        summary.push_str(&format!("Words: {}\n", total_words));
        summary.push_str(&format!("Estimated tokens: ~{}\n\n", total_tokens));

        summary.push_str("Topics discussed:\n");
        // Extract topics from user messages
        for (i, (role, content)) in conv.messages.iter().enumerate() {
            if role == "user" {
                let brief: String = content.chars().take(80).collect();
                summary.push_str(&format!("  {}. {}", i / 2 + 1, brief));
                if content.len() > 80 { summary.push_str("..."); }
                summary.push('\n');
            }
        }

        app.add_assistant_message(summary);
        app.scroll_offset = 0;
        return true;
    }

    // /compact — manually trigger context compaction.
    if trimmed == "/compact" {
        let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
        let cm = crate::agent::context::ContextManager::new(limit);
        let messages = app.current_conversation().messages.clone();

        // First compact tool results if in agent mode, then overall
        let after_tools = if app.agent_mode {
            cm.compact_tool_results(&messages)
        } else {
            messages.clone()
        };
        let compacted = cm.compact_messages(&after_tools);

        let before = messages.len();
        let after = compacted.len();

        if before == after {
            app.set_status("Conversation already compact");
        } else {
            app.current_conversation_mut().messages = compacted;
            let saved_tokens = crate::agent::context::ContextManager::conversation_tokens(&messages)
                - crate::agent::context::ContextManager::conversation_tokens(&app.current_conversation().messages);
            app.set_status(format!("Compacted: {} \u{2192} {} messages (~{} tokens saved)", before, after, saved_tokens));
        }
        return true;
    }

    // /tokens — show token usage breakdown.
    if trimmed == "/tokens" {
        let conv = app.current_conversation();
        let total = crate::agent::context::ContextManager::conversation_tokens(&conv.messages);
        let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
        let pct = (total as f64 / limit as f64 * 100.0).min(100.0);

        let mut msg = format!("Token Usage\n{}\n\n", "=".repeat(30));
        msg.push_str(&format!("Estimated tokens: ~{}\n", total));
        msg.push_str(&format!("Provider limit:   ~{}\n", limit));
        msg.push_str(&format!("Usage:            {:.1}%\n\n", pct));

        msg.push_str("Breakdown by message:\n");
        for (i, (role, content)) in conv.messages.iter().enumerate() {
            let tokens = crate::agent::context::ContextManager::estimate_tokens(content);
            msg.push_str(&format!("  {:>3}. [{:>9}] ~{:>6} tokens\n", i + 1, role, tokens));
        }

        if pct > 70.0 {
            msg.push_str(&format!("\nWarning: {:.0}% of context used. Consider /compact to save tokens.", pct));
        }

        app.add_assistant_message(msg);
        app.scroll_offset = 0;
        return true;
    }

    // /context — show what context the AI currently has.
    if trimmed == "/context" {
        let conv = app.current_conversation();
        let mut ctx = String::from("Current AI Context:\n\n");

        for (i, (role, content)) in conv.messages.iter().enumerate() {
            let tokens = crate::agent::context::ContextManager::estimate_tokens(content);
            let preview: String = content.chars().take(60).collect();
            let ellipsis = if content.len() > 60 { "..." } else { "" };

            ctx.push_str(&format!("  {:>3}. [{}] ~{} tokens: {}{}\n",
                i + 1, role, tokens, preview, ellipsis));
        }

        let total = crate::agent::context::ContextManager::conversation_tokens(&conv.messages);
        ctx.push_str(&format!("\nTotal: {} messages, ~{} tokens estimated\n", conv.messages.len(), total));

        // Show workspace if detected
        if let Some(ws) = crate::workspace::detect_workspace() {
            ctx.push_str(&format!("Workspace: {} ({:?})\n", ws.name, ws.project_type));
        }

        app.add_assistant_message(ctx);
        app.scroll_offset = 0;
        return true;
    }

    // /plugin — manage plugins.
    if trimmed == "/plugin" || trimmed == "/plugins"
        || trimmed.starts_with("/plugin ") || trimmed.starts_with("/plugins ")
    {
        let rest = trimmed
            .strip_prefix("/plugins")
            .or_else(|| trimmed.strip_prefix("/plugin"))
            .unwrap_or("")
            .trim();
        let args: Vec<&str> = rest.split_whitespace().collect();
        let subcmd = args.first().copied().unwrap_or("list");

        match subcmd {
            "list" => {
                let all = plugins::list_all_plugins();
                if all.is_empty() {
                    app.add_assistant_message(format!(
                        "No plugins installed.\n\nPlugin directory: {}\n\nCreate a plugin:\n  /plugin init\n\nOr create manually:\n  mkdir -p {}/my-plugin\n  # Add plugin.toml and a script",
                        plugins::plugins_dir().display(),
                        plugins::plugins_dir().display()
                    ));
                } else {
                    let mut msg = "Installed plugins:\n\n".to_string();
                    for (manifest, loaded) in &all {
                        let status = if *loaded { "enabled" } else { "disabled" };
                        msg.push_str(&format!(
                            "  /{:<15} {} (v{}) [{}]\n    {}\n\n",
                            manifest.command, manifest.name, manifest.version, status, manifest.description
                        ));
                    }
                    app.add_assistant_message(msg);
                }
            }
            "init" => {
                match plugins::create_example_plugin() {
                    Ok(path) => {
                        app.add_assistant_message(format!(
                            "Created example plugin at:\n  {}\n\nEdit plugin.toml and run.sh to customize.\nRestart Nerve to load new plugins.",
                            path.display()
                        ));
                    }
                    Err(e) => app.set_status(format!("Error: {e}")),
                }
            }
            "reload" => {
                app.plugins = plugins::load_plugins();
                app.set_status(format!("{} plugin(s) loaded", app.plugins.len()));
            }
            _ => {
                app.add_assistant_message("Usage: /plugin list | /plugin init | /plugin reload".into());
            }
        }
        app.scroll_offset = 0;
        return true;
    }

    // Check plugins — dispatch to any plugin whose command matches.
    {
        let cmd = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let (plugin_cmd, plugin_args) = match cmd.find(char::is_whitespace) {
            Some(pos) => (&cmd[..pos], cmd[pos..].trim()),
            None => (cmd, ""),
        };
        for plugin in &app.plugins {
            if plugin.manifest.command == plugin_cmd {
                match plugin.execute(plugin_args, "") {
                    Ok(output) => {
                        app.add_assistant_message(output);
                    }
                    Err(e) => {
                        app.set_status(format!("Plugin error: {e}"));
                    }
                }
                return true;
            }
        }
    }

    // /branch — conversation branching.
    if trimmed == "/branch" || trimmed == "/br"
        || trimmed.starts_with("/branch ") || trimmed.starts_with("/br ")
    {
        let rest = if trimmed.starts_with("/branch") {
            trimmed.strip_prefix("/branch").unwrap_or("").trim()
        } else {
            trimmed.strip_prefix("/br").unwrap_or("").trim()
        };
        let args: Vec<&str> = rest.split_whitespace().collect();
        let subcmd = args.first().copied().unwrap_or("list");
        match subcmd {
            "save" | "create" => {
                let name = if args.len() > 1 {
                    args[1..].join(" ")
                } else {
                    format!("Branch {}", app.branches.len() + 1)
                };
                app.create_branch(name.clone());
                app.set_status(format!("Branch saved: {name}"));
            }
            "list" => {
                if app.branches.is_empty() {
                    app.add_assistant_message(
                        "No branches saved.\n\nUsage:\n  /branch save [name]  Save current state\n  /branch restore <n>  Restore a branch\n  /branch delete <n>   Delete a branch\n  /branch diff <n>     Compare with a branch".into()
                    );
                } else {
                    let mut msg = "Saved branches:\n\n".to_string();
                    for (i, branch) in app.branches.iter().enumerate() {
                        let msg_count = branch.messages.len();
                        let time = branch.created_at.format("%H:%M:%S");
                        msg.push_str(&format!("  {}. {} ({} messages, saved at {})\n", i + 1, branch.name, msg_count, time));
                    }
                    msg.push_str("\nUsage: /branch restore <number> | /branch delete <number>");
                    app.add_assistant_message(msg);
                }
            }
            "restore" | "load" => {
                if let Some(idx_str) = args.get(1) {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let idx = idx.saturating_sub(1); // 1-indexed
                        if idx < app.branches.len() {
                            let name = app.branches[idx].name.clone();
                            app.restore_branch(idx);
                            app.set_status(format!("Restored branch: {name}"));
                        } else {
                            app.set_status("Invalid branch number");
                        }
                    } else {
                        app.set_status("Usage: /branch restore <number>");
                    }
                } else {
                    app.set_status("Usage: /branch restore <number>");
                }
            }
            "delete" | "rm" => {
                if let Some(idx_str) = args.get(1) {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let idx = idx.saturating_sub(1);
                        if idx < app.branches.len() {
                            let name = app.branches[idx].name.clone();
                            app.delete_branch(idx);
                            app.set_status(format!("Deleted branch: {name}"));
                        } else {
                            app.set_status("Invalid branch number");
                        }
                    }
                }
            }
            "diff" => {
                if let Some(idx_str) = args.get(1) {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        let idx = idx.saturating_sub(1);
                        if let Some(branch) = app.branches.get(idx) {
                            let current = &app.current_conversation().messages;
                            let branched = &branch.messages;

                            let common = current.iter().zip(branched.iter())
                                .take_while(|(a, b)| a == b)
                                .count();

                            let mut msg = format!("Diff with branch '{}'\n{}\n\n", branch.name, "=".repeat(30));
                            msg.push_str(&format!("Common messages: {}\n", common));
                            msg.push_str(&format!("Current has {} more message(s)\n", current.len().saturating_sub(common)));
                            msg.push_str(&format!("Branch has {} more message(s)\n\n", branched.len().saturating_sub(common)));

                            if current.len() > common {
                                msg.push_str("Current (diverged):\n");
                                for (role, content) in &current[common..] {
                                    let brief: String = content.chars().take(80).collect();
                                    msg.push_str(&format!("  [{role}] {brief}...\n"));
                                }
                            }
                            if branched.len() > common {
                                msg.push_str("\nBranch (diverged):\n");
                                for (role, content) in &branched[common..] {
                                    let brief: String = content.chars().take(80).collect();
                                    msg.push_str(&format!("  [{role}] {brief}...\n"));
                                }
                            }

                            app.add_assistant_message(msg);
                        }
                    }
                }
            }
            _ => {
                app.add_assistant_message("Usage: /branch save|list|restore|delete|diff".into());
            }
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
            app.streaming_start = Some(std::time::Instant::now());

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
    if app.is_streaming {
        app.set_status("Already streaming \u{2014} press Esc to cancel first");
        return;
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

    // Apply context management based on provider
    let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
    let cm = crate::agent::context::ContextManager::new(limit);

    // First compact tool results if in agent mode
    let conversation_messages = if app.agent_mode {
        cm.compact_tool_results(&app.current_conversation().messages)
    } else {
        app.current_conversation().messages.clone()
    };

    // Then compact the overall conversation if needed
    let final_messages = cm.compact_messages(&conversation_messages);

    let mut messages: Vec<ChatMessage> = final_messages
        .iter()
        .filter_map(|(role, content)| match role.as_str() {
            // Expand @file references in user messages so the AI sees file
            // contents while the conversation history keeps the original text.
            "user" => {
                let expanded = files::expand_file_references(content);
                Some(ChatMessage::user(expanded))
            }
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

    // Update total_tokens_used tracker
    app.total_tokens_used = crate::agent::context::ContextManager::conversation_tokens(
        &app.current_conversation().messages,
    );

    let model = app.selected_model.clone();
    let (tx, rx) = mpsc::unbounded_channel();

    app.stream_rx = Some(rx);
    app.is_streaming = true;
    app.streaming_response.clear();
    app.streaming_start = Some(std::time::Instant::now());

    let provider = Arc::clone(provider);
    tokio::spawn(async move {
        if let Err(e) = provider.chat_stream(&messages, &model, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error(e.to_string()));
        }
    });
}

/// Regenerate the last assistant response by removing it and re-sending.
async fn regenerate_response(app: &mut App, provider: &Arc<dyn AiProvider>, _config: &Config) {
    if app.is_streaming { return; }

    let conv = app.current_conversation_mut();
    // Remove the last assistant message
    if let Some(pos) = conv.messages.iter().rposition(|(role, _)| role == "assistant") {
        conv.messages.remove(pos);
    } else {
        app.set_status("No response to regenerate");
        return;
    }

    // Apply context management based on provider
    let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
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
    app.stream_rx = Some(rx);
    app.is_streaming = true;
    app.streaming_response.clear();
    app.streaming_start = Some(std::time::Instant::now());
    app.scroll_offset = 0;
    app.set_status("Regenerating...");

    let provider = Arc::clone(provider);
    tokio::spawn(async move {
        if let Err(e) = provider.chat_stream(&messages, &model, tx.clone()).await {
            let _ = tx.send(StreamEvent::Error(e.to_string()));
        }
    });
}

/// Edit the last user message: load it back into the input buffer and remove
/// it (plus any assistant response after it) from the conversation.
fn edit_last_message(app: &mut App) {
    if app.is_streaming { return; }

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
fn delete_last_exchange(app: &mut App) {
    if app.is_streaming { return; }
    let conv = app.current_conversation_mut();
    if conv.messages.is_empty() { return; }

    // Remove last message
    let last_role = conv.messages.last().map(|(r, _)| r.clone());
    conv.messages.pop();

    // If we removed an assistant message, also remove the preceding user message
    if last_role.as_deref() == Some("assistant") {
        if conv.messages.last().map(|(r, _)| r.as_str()) == Some("user") {
            conv.messages.pop();
        }
    }

    app.set_status("Deleted last exchange");
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
fn clear_conversation(app: &mut App) {
    app.current_conversation_mut().messages.clear();
    app.streaming_response.clear();
    app.is_streaming = false;
    app.stream_rx = None;
    app.scroll_offset = 0;
    app.set_status("Conversation cleared");
}

/// Cycle to the next conversation (wraps around).
fn cycle_conversation(app: &mut App) {
    if app.conversations.len() > 1 {
        app.active_conversation = (app.active_conversation + 1) % app.conversations.len();
        app.scroll_offset = 0;
        app.set_status(format!(
            "Switched to conversation {}",
            app.active_conversation + 1
        ));
    }
}

fn cycle_conversation_back(app: &mut App) {
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

/// Returns `true` if the command matches a well-known destructive pattern
/// that should never be run from within Nerve.
fn is_dangerous_command(cmd: &str) -> bool {
    let dangerous = [
        "rm -rf /",
        "rm -rf /*",
        "mkfs",
        "dd if=",
        "> /dev/sd",
        "chmod -R 777 /",
    ];
    dangerous.iter().any(|d| cmd.contains(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_prefix_single() {
        let strs = ["hello"];
        let refs: Vec<&&str> = strs.iter().collect();
        assert_eq!(common_prefix(&refs), "hello");
    }

    #[test]
    fn test_common_prefix_multiple() {
        let strs = ["model", "models"];
        let refs: Vec<&&str> = strs.iter().collect();
        assert_eq!(common_prefix(&refs), "model");
    }

    #[test]
    fn test_common_prefix_none() {
        let strs = ["abc", "xyz"];
        let refs: Vec<&&str> = strs.iter().collect();
        assert_eq!(common_prefix(&refs), "");
    }

    #[test]
    fn test_common_prefix_empty() {
        let refs: Vec<&&str> = vec![];
        assert_eq!(common_prefix(&refs), "");
    }

    #[test]
    fn edit_last_message_loads_input() {
        let mut app = App::new();
        app.add_user_message("hello world".into());
        app.add_assistant_message("hi there".into());
        edit_last_message(&mut app);
        assert_eq!(app.input, "hello world");
        assert_eq!(app.cursor_position, 11);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn edit_last_message_no_messages() {
        let mut app = App::new();
        edit_last_message(&mut app);
        assert!(app.input.is_empty());
        assert_eq!(app.status_message.as_deref(), Some("No message to edit"));
    }

    #[test]
    fn delete_last_exchange_removes_pair() {
        let mut app = App::new();
        app.add_user_message("question".into());
        app.add_assistant_message("answer".into());
        delete_last_exchange(&mut app);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn delete_last_exchange_single_message() {
        let mut app = App::new();
        app.add_user_message("question".into());
        delete_last_exchange(&mut app);
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn cycle_conversation_wraps() {
        let mut app = App::new();
        app.new_conversation();
        app.new_conversation();
        assert_eq!(app.active_conversation, 2);
        cycle_conversation(&mut app);
        assert_eq!(app.active_conversation, 0); // wraps
    }

    #[test]
    fn cycle_conversation_back_wraps() {
        let mut app = App::new();
        app.new_conversation();
        app.new_conversation();
        // active_conversation is 2 (last)
        // Go to first
        app.active_conversation = 0;
        cycle_conversation_back(&mut app);
        assert_eq!(app.active_conversation, 2); // wraps to last
    }

    #[test]
    fn update_search_results_finds_matches() {
        let mut app = App::new();
        app.add_user_message("hello world".into());
        app.add_assistant_message("goodbye world".into());
        app.add_user_message("hello again".into());
        app.search_query = "hello".into();
        update_search_results(&mut app);
        assert_eq!(app.search_results.len(), 2);
        assert_eq!(app.search_results, vec![0, 2]);
    }

    #[test]
    fn update_search_results_empty_query() {
        let mut app = App::new();
        app.add_user_message("test".into());
        app.search_query = String::new();
        update_search_results(&mut app);
        assert!(app.search_results.is_empty());
    }

    #[test]
    fn update_search_results_case_insensitive() {
        let mut app = App::new();
        app.add_user_message("Hello World".into());
        app.search_query = "hello".into();
        update_search_results(&mut app);
        assert_eq!(app.search_results.len(), 1);
    }

    #[test]
    fn delete_last_exchange_empty_conversation() {
        let mut app = App::new();
        delete_last_exchange(&mut app); // Should not panic
        assert!(app.current_conversation().messages.is_empty());
    }

    #[test]
    fn regenerate_with_no_assistant_message() {
        let mut app = App::new();
        app.add_user_message("hello".into());
        // No assistant message — edit should still work on user msg
        edit_last_message(&mut app);
        assert_eq!(app.input, "hello");
    }

    #[test]
    fn expand_file_references_preserves_nonfile_at() {
        // @username should not be expanded
        let text = "hello @john how are you";
        let expanded = files::expand_file_references(text);
        assert_eq!(expanded, text); // No dots/slashes, so no expansion
    }

    // ── is_dangerous_command ───────────────────────────────────────────

    #[test]
    fn dangerous_rm_rf_root() {
        assert!(is_dangerous_command("rm -rf /"));
        assert!(is_dangerous_command("rm -rf /*"));
        assert!(is_dangerous_command("sudo rm -rf /"));
    }

    #[test]
    fn dangerous_mkfs() {
        assert!(is_dangerous_command("mkfs.ext4 /dev/sda1"));
    }

    #[test]
    fn dangerous_dd() {
        assert!(is_dangerous_command("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn safe_commands_pass() {
        assert!(!is_dangerous_command("ls -la"));
        assert!(!is_dangerous_command("rm file.txt"));
        assert!(!is_dangerous_command("cargo test"));
        assert!(!is_dangerous_command("git status"));
    }

    // ── set_status auto-clear ─────────────────────────────────────────

    #[test]
    fn set_status_populates_both_fields() {
        let mut app = App::new();
        app.set_status("hello");
        assert_eq!(app.status_message.as_deref(), Some("hello"));
        assert!(app.status_time.is_some());
    }

    // ── is_dangerous_command: additional safe commands ────────────────

    #[test]
    fn test_is_dangerous_command_safe_commands() {
        assert!(!is_dangerous_command("ls -la"));
        assert!(!is_dangerous_command("cargo test"));
        assert!(!is_dangerous_command("git status"));
        assert!(!is_dangerous_command("echo hello"));
    }

    #[test]
    fn test_is_dangerous_command_dangerous() {
        assert!(is_dangerous_command("rm -rf /"));
        assert!(is_dangerous_command("rm -rf /*"));
        assert!(is_dangerous_command("sudo rm -rf /home; rm -rf /"));
    }

    #[test]
    fn test_is_dangerous_command_chmod_root() {
        assert!(is_dangerous_command("chmod -R 777 /"));
    }

    #[test]
    fn test_is_dangerous_command_dd() {
        assert!(is_dangerous_command("dd if=/dev/urandom of=/dev/sda"));
    }

    #[test]
    fn test_is_dangerous_redirect_to_dev() {
        assert!(is_dangerous_command("echo foo > /dev/sda"));
    }
}
