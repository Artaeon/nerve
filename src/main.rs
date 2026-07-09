mod agent;
mod ai;
mod app;
mod automation;
mod clipboard;
mod clipboard_manager;
mod commands;
mod completion;
mod config;
mod conversation;
#[cfg(unix)]
mod daemon;
mod design;
mod design_presets;
mod files;
mod history;
mod input;
mod keybinds;
mod knowledge;
mod memory_recall;
mod model_router;
mod plugins;
mod project;
mod prompts;
mod provider_health;
mod provider_setup;
mod queue;
mod remote;
mod scaffold;
mod scraper;
mod session;
mod shell;
mod splash;
mod ui;
mod usage;
mod verify;
mod worker;
mod workspace;

use std::io::{self, Read as _};
use std::sync::Arc;

use anyhow::Context;
use clap::{CommandFactory, Parser};
use crossterm::event::{self, Event};
#[cfg(test)]
use tokio::sync::mpsc;

use ai::provider::{AiProvider, ChatMessage, StreamEvent};
use app::App;
use clipboard_manager::ClipboardSource;
use config::Config;

pub(crate) use completion::{
    accept_autocomplete, common_prefix, complete_file_path, get_all_commands, list_file_matches,
    update_autocomplete,
};
#[cfg(test)]
pub(crate) use completion::{
    autocomplete_file_paths, find_common_prefix_strings, strip_autocomplete_description,
};
pub(crate) use conversation::{
    advance_pipeline_if_active, build_context_messages, clear_conversation,
    copy_last_assistant_message, cycle_conversation, cycle_conversation_back, delete_last_exchange,
    edit_last_message, format_agent_action_summary, generate_title, regenerate_response,
    run_agent_tools_task, send_to_ai_from_history, submit_message,
};
// Used only by the in-file test module now that the AgentToolsComplete rebuild
// goes through build_context_messages.
#[cfg(test)]
pub(crate) use conversation::inject_pipeline_role_prompts;
pub(crate) use input::handle_key_event;
#[cfg(test)]
pub(crate) use input::update_search_results;
pub(crate) use provider_setup::{
    create_provider, create_provider_from_app, default_model_for_provider, detect_ollama_models,
    list_models, load_last_provider, models_for_provider, provider_help_message,
    save_last_provider,
};
pub(crate) use splash::{render_goodbye, render_splash};

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "nerve", about = "Raw AI power in your terminal", version)]
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

    /// Start as background daemon (Unix only)
    #[arg(long)]
    daemon: bool,

    /// Send query to running daemon (Unix only)
    #[arg(long)]
    query: Option<String>,

    /// Stop the running daemon (Unix only)
    #[arg(long)]
    stop_daemon: bool,

    /// Submit a coding job to the running server; the current directory is the
    /// repo. The server runs it on its own branch and commits for review.
    #[arg(long, value_name = "PROMPT")]
    submit: Option<String>,

    /// With --submit: attach your last session (full conversation context) to
    /// the job, so the server resumes with everything you had — nothing lost.
    #[arg(long)]
    with_session: bool,

    /// List jobs on the running server (Unix only)
    #[arg(long)]
    jobs: bool,

    /// With --jobs: output the queue as JSON (used by remote clients).
    #[arg(long)]
    json: bool,

    /// Cancel a queued job by id (Unix only)
    #[arg(long, value_name = "ID")]
    cancel_job: Option<String>,

    /// Resume the last session
    #[arg(short = 'c', long = "continue")]
    continue_session: bool,

    /// Skip the splash screen
    #[arg(long)]
    no_splash: bool,

    /// Generate shell completions and exit
    #[arg(long, value_name = "SHELL")]
    completions: Option<clap_complete::Shell>,
}

/// Send a queue command to the running nerve server, turning a connection
/// failure into an actionable message instead of a raw socket error.
#[cfg(unix)]
async fn send_to_server(message: &str) -> anyhow::Result<String> {
    daemon::send_to_daemon(message).await.map_err(|e| {
        anyhow::anyhow!(
            "Could not reach the nerve server. Start it with `nerve --daemon` \
             (on this machine, or over SSH on your server). Details: {e}"
        )
    })
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

    // ── Shell completions ──────────────────────────────────────────
    if let Some(shell) = cli.completions {
        clap_complete::generate(shell, &mut Cli::command(), "nerve", &mut std::io::stdout());
        return Ok(());
    }

    // ── Daemon commands (no provider needed, Unix only) ──────────────
    #[cfg(unix)]
    {
        if cli.daemon {
            if daemon::is_daemon_running() {
                eprintln!("Nerve daemon is already running.");
                return Ok(());
            }
            return daemon::start_daemon().await;
        }
        if cli.stop_daemon {
            daemon::stop_daemon()?;
            println!("Nerve daemon stopped.");
            return Ok(());
        }
        if let Some(query) = &cli.query {
            let response = daemon::send_to_daemon(query).await?;
            println!("{response}");
            return Ok(());
        }
        // ── Queue client commands (talk to the running server) ──────────
        if let Some(prompt) = &cli.submit {
            let repo = std::env::current_dir()?
                .canonicalize()?
                .to_string_lossy()
                .to_string();
            let response = send_to_server(&format!("SUBMIT\t{repo}\t{prompt}")).await?;
            println!("{response}");
            // Optionally carry the full conversation context so the server
            // resumes exactly where the client left off. The job id is the
            // token after "job" in the "OK queued job <id> ..." reply.
            if cli.with_session {
                let id = response
                    .split_whitespace()
                    .skip_while(|w| *w != "job")
                    .nth(1);
                match (id, session::last_session_json()) {
                    (Some(id), Some(ctx)) => {
                        let ack = send_to_server(&format!("ATTACH\t{id}\t{ctx}")).await?;
                        println!("{ack}");
                    }
                    (Some(_), None) => {
                        eprintln!("(no saved session found to attach)");
                    }
                    _ => {}
                }
            }
            return Ok(());
        }
        if cli.jobs {
            let response = send_to_server(if cli.json { "LISTJSON" } else { "LIST" }).await?;
            println!("{response}");
            return Ok(());
        }
        if let Some(id) = &cli.cancel_job {
            let response = send_to_server(&format!("CANCEL\t{id}")).await?;
            println!("{response}");
            return Ok(());
        }
    }
    #[cfg(not(unix))]
    {
        if cli.daemon
            || cli.stop_daemon
            || cli.query.is_some()
            || cli.submit.is_some()
            || cli.jobs
            || cli.cancel_job.is_some()
        {
            eprintln!("The nerve server/queue is only supported on Unix systems.");
            return Ok(());
        }
    }

    let config = Config::load().context("failed to load configuration")?;

    // ── Provider health check + auto-fallback ───────────────────────────
    // Probe the chosen provider cheaply BEFORE the first prompt can fail.
    // If the default provider can't work on this machine, fall back to the
    // best available one so nerve works out of the box. An explicitly
    // requested provider (--provider) is never silently switched.
    let mut fallback_provider: Option<String> = None;
    let mut startup_note: Option<String> = None;
    {
        let requested = cli.provider.as_deref().unwrap_or(&config.default_provider);
        let report = provider_health::check_provider(requested, &config);
        if !report.healthy {
            if cli.provider.is_some() {
                eprintln!(
                    "Provider '{requested}' is not available: {}\n",
                    report.detail
                );
                eprintln!("{}", provider_help_message(requested));
                std::process::exit(1);
            }
            match provider_health::pick_fallback(requested, &config) {
                Some((fb, _detail)) => {
                    startup_note = Some(format!(
                        "{requested} unavailable ({}) — switched to {fb}. /provider to change",
                        report.detail
                    ));
                    fallback_provider = Some(fb);
                }
                None => {
                    eprintln!(
                        "Could not start the '{requested}' provider: {}\n",
                        report.detail
                    );
                    eprintln!("{}", provider_health::no_provider_guidance());
                    std::process::exit(1);
                }
            }
        }
    }
    let provider_arg = fallback_provider.as_deref().or(cli.provider.as_deref());

    let provider = match create_provider(&config, provider_arg) {
        Ok(p) => p,
        Err(e) => {
            // Show the friendly, provider-specific setup guidance instead of a
            // bare error — this is the first thing a new user hits if their key
            // / CLI / local server isn't configured yet.
            let provider_name = provider_arg.unwrap_or(&config.default_provider);
            eprintln!("Could not start the '{provider_name}' provider: {e}\n");
            eprintln!("{}", provider_help_message(provider_name));
            eprintln!(
                "\nThen re-run nerve, or switch provider with `nerve --provider <name>` \
                 (claude, openai, openrouter, ollama)."
            );
            std::process::exit(1);
        }
    };
    let provider: Arc<dyn AiProvider> = Arc::from(provider);

    // When we fell back, the configured default model belongs to the OLD
    // provider — use the fallback provider's default instead.
    let model = match (&cli.model, &fallback_provider) {
        (Some(m), _) => m.clone(),
        (None, Some(fb)) => provider_setup::default_model_for_provider(fb).to_string(),
        (None, None) => config.default_model.clone(),
    };

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
    let effective_provider = provider_arg.unwrap_or(&config.default_provider).to_string();
    run_tui(
        provider,
        config,
        cli.continue_session,
        cli.no_splash,
        cli.provider.is_some(),
        StartupState {
            provider: effective_provider,
            model,
            note: startup_note,
        },
    )
    .await
}

/// Provider/model resolution done in `run()` (after health checks and
/// fallback), handed to the TUI so its initial state matches what was
/// actually started.
struct StartupState {
    provider: String,
    model: String,
    note: Option<String>,
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

// ─── Interactive TUI ────────────────────────────────────────────────────────

async fn run_tui(
    provider: Arc<dyn AiProvider>,
    config: Config,
    continue_session: bool,
    no_splash: bool,
    provider_from_cli: bool,
    startup: StartupState,
) -> anyhow::Result<()> {
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

    // Show splash screen briefly.
    if !no_splash {
        terminal.draw(|frame| render_splash(frame, "Loading..."))?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    let init_status =
        |terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
         msg: &str| {
            if !no_splash {
                terminal.draw(|frame| render_splash(frame, msg)).ok();
            }
        };

    let mut app = App::new();
    app.selected_model = startup.model.clone();
    app.selected_provider = startup.provider.clone();
    app.auto_agent = config.auto_agent;
    app.workflow_auto_approve = config.workflow_auto_approve;
    app.auto_model_routing = config.auto_model_routing;
    app.auto_verify = config.auto_verify;
    app.command_timeout_secs = config.command_timeout_secs;
    app.git_user_name = config.git_user_name.clone().unwrap_or_default();
    app.git_user_email = config.git_user_email.clone().unwrap_or_default();
    app.context_limit_override = config.context_limit;
    app.remote_server = config.remote_server.clone();

    // Load last used provider if not specified via CLI — but never restore a
    // provider that can't work right now (e.g. the claude CLI was removed
    // since the last run); the health-checked startup provider stays.
    if !provider_from_cli
        && let Some((provider_name, model)) = load_last_provider()
        && provider_health::check_provider(&provider_name, &config).healthy
    {
        app.selected_provider = provider_name;
        app.selected_model = model;
        app.available_models = models_for_provider(&app.selected_provider);
        app.provider_changed = true;
    }

    // Surface the auto-fallback decision (if any) in the status bar so the
    // user knows which provider is actually answering.
    if let Some(note) = &startup.note {
        app.set_status(note.clone());
    }

    init_status(&mut terminal, "Detecting workspace...");

    // Auto-detect workspace and inject system prompt with project context.
    // Cache the full WorkspaceInfo so later commands don't re-scan the filesystem.
    let detected_workspace = workspace::detect_workspace();
    if let Some(ref ws) = detected_workspace {
        let sys_prompt = ws.to_system_prompt();
        app.current_conversation_mut()
            .messages
            .insert(0, ("system".into(), sys_prompt));
        app.detected_workspace = Some(format!(
            "{} ({:?}) \u{2014} {}",
            ws.name,
            ws.project_type,
            ws.tech_stack.join(", "),
        ));
    }
    app.cached_workspace = detected_workspace.clone();
    // Resolve the verify command now that the workspace is known: explicit
    // config override, else auto-detect from the workspace (Cargo/npm).
    app.verify_command = config.verify_command.clone().or_else(|| {
        detected_workspace
            .as_ref()
            .and_then(|ws| verify::detect_verify_command(&ws.root))
    });

    init_status(&mut terminal, "Loading plugins...");

    // Load plugins from ~/.config/nerve/plugins/
    let loaded_plugins = plugins::load_plugins();
    if !loaded_plugins.is_empty() {
        app.set_status(format!("{} plugin(s) loaded", loaded_plugins.len()));
    }
    app.plugins = loaded_plugins.clone();

    // Build startup status line.
    let mut info_parts = vec![format!(
        "{} > {}",
        app.selected_provider, app.selected_model
    )];
    if let Some(ref ws) = detected_workspace {
        info_parts.push(format!("{:?}: {}", ws.project_type, ws.name));
    }
    if !loaded_plugins.is_empty() {
        info_parts.push(format!("{} plugin(s)", loaded_plugins.len()));
    }
    info_parts.push("/help for commands".into());
    app.set_status(info_parts.join(" | "));

    if !no_splash {
        terminal.draw(|frame| render_splash(frame, "Ready!"))?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Restore previous session if --continue was passed.
    if continue_session {
        match session::load_last_session() {
            Ok(sess) => {
                session::restore_session_to_app(&sess, &mut app);
                app.set_status(format!(
                    "Resumed session ({} conversation(s))",
                    app.conversations.len()
                ));
            }
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("No such file") || msg.contains("not found") {
                    app.set_status("No previous session found");
                } else {
                    app.set_status(format!("Failed to restore session: {e}"));
                }
            }
        }
    }

    let result = event_loop(&mut terminal, &mut app, &provider, &config).await;

    // Auto-save session on every quit path.
    let sess = session::session_from_app(&app);
    if let Err(e) = session::save_session(&sess) {
        tracing::warn!("failed to save session on exit: {e}");
    }

    // Remember the last used provider+model for new sessions.
    save_last_provider(&app.selected_provider, &app.selected_model);

    // Show goodbye splash before leaving.
    if !no_splash {
        terminal.draw(|frame| render_goodbye(frame, &app))?;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

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
        // Auto-clear status messages. Errors/warnings linger longer (12s) than
        // transient confirmations (5s) so an actionable failure isn't gone
        // before the user can read it.
        if let Some(time) = app.status_time {
            let ttl = match app.status_message.as_deref() {
                Some(m) if ui::status_bar::is_error_status(m) => std::time::Duration::from_secs(12),
                _ => std::time::Duration::from_secs(5),
            };
            if time.elapsed() > ttl {
                app.status_message = None;
                app.status_time = None;
            }
        }

        // Advance animation frame counter (wraps on overflow).
        app.thinking_frame = app.thinking_frame.wrapping_add(1);

        // Draw the UI.
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Check if provider needs to be recreated.
        if app.provider_changed {
            match create_provider_from_app(config, app) {
                Ok(new_provider) => {
                    provider = Arc::from(new_provider);
                }
                Err(e) => {
                    let help = provider_help_message(&app.selected_provider);
                    app.add_assistant_message(format!("Provider error: {e}\n\n{help}"));
                }
            }
            app.provider_changed = false;
        }

        // Drain any pending command queued by the Nerve Bar.
        if let Some(cmd) = app.pending_command.take() {
            submit_message(app, &cmd, &provider).await;
        }

        // Adaptive poll: fast during streaming for smooth updates, slow when idle to save CPU.
        let poll_duration = if app.is_streaming {
            std::time::Duration::from_millis(16) // ~60fps for smooth streaming
        } else {
            std::time::Duration::from_millis(100) // 10fps when idle — saves CPU
        };

        if event::poll(poll_duration)? {
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

        // Execute any pending command (e.g. slash command selected from Nerve Bar).
        if let Some(cmd) = app.pending_command.take() {
            submit_message(app, &cmd, &provider).await;
        }

        // Drain any pending stream tokens.
        //
        // We temporarily take the receiver out of `app` so we can mutate
        // both the receiver and the rest of app without overlapping borrows.
        if app.is_streaming
            && let Some(mut rx) = app.stream_rx.take()
        {
            let mut finished = false;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    StreamEvent::Token(token) => app.append_to_streaming(&token),
                    StreamEvent::ToolStart { tool, summary } => {
                        app.active_tool = Some(tool.clone());
                        app.set_status(format!("\u{2699} {tool}: {summary}"));
                    }
                    StreamEvent::ToolDone {
                        tool,
                        success,
                        output_preview,
                    } => {
                        app.active_tool = None;
                        let icon = if success { "\u{2713}" } else { "\u{2717}" };
                        app.set_status(format!("{icon} {tool}: {output_preview}"));
                    }
                    StreamEvent::Done => {
                        // Grab the content before finish_streaming moves it.
                        let response_content = app.streaming_response.clone();
                        app.finish_streaming();
                        app.scroll_offset = 0; // Auto-scroll to show the new response

                        // Track usage. Use the token estimate of the payload
                        // ACTUALLY sent (recorded by build_context_messages:
                        // post-expansion/compaction, incl. injected system
                        // prompts) rather than re-summing the raw stored
                        // conversation, which ignored @file expansion and
                        // injected prompts and double-counted pre-compaction
                        // history.
                        {
                            let tokens_sent = app.last_sent_tokens;
                            let tokens_received =
                                crate::agent::context::ContextManager::estimate_tokens(
                                    &response_content,
                                );
                            app.usage_stats.record_request(
                                tokens_sent,
                                tokens_received,
                                &app.selected_provider,
                                &app.selected_model,
                            );
                        }

                        if !response_content.is_empty() {
                            app.clipboard_manager
                                .add(response_content, ClipboardSource::AiResponse);
                            let _ = app.clipboard_manager.save();
                        }

                        // Auto-set title from first user message (improved).
                        {
                            let conv = app.current_conversation_mut();
                            if conv.title == "New Conversation"
                                && let Some((_role, content)) =
                                    conv.messages.iter().find(|(r, _)| r == "user")
                            {
                                conv.title = generate_title(content);
                            }
                        }

                        // Auto-save conversation to history AND persist the
                        // session, so a later crash resumes exactly here. Both
                        // go through the shared helper / session_from_app so
                        // every save path writes identical records.
                        conversation::persist_current_conversation(app);
                        let _ = session::save_session(&session::session_from_app(app));

                        // Agent mode: check for tool calls and execute them
                        if app.agent_mode {
                            let last_response = app
                                .current_conversation()
                                .messages
                                .last()
                                .filter(|(r, _)| r == "assistant")
                                .map(|(_, c)| c.clone());

                            if let Some(response) = last_response {
                                let tool_calls = crate::agent::tools::parse_tool_calls(&response);

                                // Track file-editing so the verify gate only
                                // runs on turns that actually changed code, and
                                // remember the paths for the auto design-check.
                                for c in &tool_calls {
                                    if verify::is_write_tool(&c.tool) {
                                        app.agent_made_edits = true;
                                        if let Some(path) = c.args.get("path") {
                                            app.turn_edited_files.push(path.clone());
                                        }
                                    }
                                }

                                if !tool_calls.is_empty() && app.agent_iterations < 10 {
                                    app.agent_iterations += 1;

                                    // Show what the agent is doing in a
                                    // human-readable way (cheap; stays on
                                    // the UI thread).
                                    let action_summary = format_agent_action_summary(&tool_calls);
                                    app.set_status(format!(
                                        "Agent (step {}/10):\n{}",
                                        app.agent_iterations, action_summary
                                    ));

                                    // Move tool execution off the event
                                    // loop: a long-running command (test
                                    // run, `npm install`, etc.) would
                                    // otherwise freeze the UI for its
                                    // entire duration. The spawned runner
                                    // emits ToolStart / ToolDone events
                                    // per tool so the status bar stays
                                    // live, and finishes with
                                    // AgentToolsComplete carrying the
                                    // aggregated results message.
                                    let (tool_tx, tool_rx) = tokio::sync::mpsc::unbounded_channel();
                                    app.cancel_active_stream();
                                    app.stream_rx = Some(tool_rx);
                                    app.is_streaming = true;
                                    app.streaming_response.clear();
                                    app.streaming_start = Some(std::time::Instant::now());

                                    let timeout_secs = config.command_timeout_secs;
                                    // Derive the tool policy from the
                                    // pipeline role, if any. Outside of a
                                    // workflow, the normal agent mode has
                                    // full tool access.
                                    let policy = app
                                        .pipeline
                                        .as_ref()
                                        .and_then(|p| p.step.role())
                                        .map(|r| r.tool_policy)
                                        .unwrap_or(crate::agent::pipeline::ToolPolicy::Full);
                                    let handle = tokio::spawn(run_agent_tools_task(
                                        tool_calls,
                                        timeout_secs,
                                        policy,
                                        tool_tx,
                                    ));
                                    app.stream_abort = Some(handle.abort_handle());

                                    // Continue the drain loop against the
                                    // new receiver — the runner will feed
                                    // it.
                                    break;
                                } else if app.agent_iterations > 0 {
                                    // Agent turn finished. Before handing back,
                                    // run the project verify command if this
                                    // turn edited files — on failure its output
                                    // is fed back (as a normal tool result) so
                                    // the agent fixes its own mistakes instead
                                    // of leaving a broken build for the user.
                                    if verify::should_run_verify(
                                        app.auto_verify,
                                        app.verify_command.as_deref(),
                                        app.agent_made_edits,
                                        app.verify_rounds,
                                    ) {
                                        let cmd = app.verify_command.clone().unwrap();
                                        app.verify_rounds += 1;
                                        // Only re-verify if the agent makes NEW
                                        // edits in response to the output.
                                        app.agent_made_edits = false;
                                        app.agent_iterations += 1;
                                        app.set_status(format!("Verifying changes: {cmd}"));

                                        let verify_call = crate::agent::tools::ToolCall {
                                            tool: "run_command".into(),
                                            args: std::collections::HashMap::from([(
                                                "command".to_string(),
                                                cmd,
                                            )]),
                                        };
                                        let (tool_tx, tool_rx) =
                                            tokio::sync::mpsc::unbounded_channel();
                                        app.cancel_active_stream();
                                        app.stream_rx = Some(tool_rx);
                                        app.is_streaming = true;
                                        app.streaming_response.clear();
                                        app.streaming_start = Some(std::time::Instant::now());
                                        // Builds/type-checks are slower than a
                                        // normal command — give them room.
                                        let timeout_secs = config.command_timeout_secs.max(300);
                                        let handle = tokio::spawn(run_agent_tools_task(
                                            vec![verify_call],
                                            timeout_secs,
                                            crate::agent::pipeline::ToolPolicy::Full,
                                            tool_tx,
                                        ));
                                        app.stream_abort = Some(handle.abort_handle());
                                        break;
                                    }

                                    // No verify due — the turn is complete.
                                    app.set_status(format!(
                                        "Agent completed in {} iteration(s)",
                                        app.agent_iterations
                                    ));

                                    // Auto-capture a running record of what this
                                    // turn worked on so a later session has a
                                    // "recent activity" summary. Best-effort — a
                                    // failed write must never break the turn.
                                    if let Some(ws) = app.cached_workspace.clone() {
                                        let request = app
                                            .current_conversation()
                                            .messages
                                            .iter()
                                            .rev()
                                            .find(|(role, content)| {
                                                role == "user"
                                                    && !content.starts_with(
                                                        conversation::TOOL_RESULTS_PREFIX,
                                                    )
                                            })
                                            .map(|(_, c)| c.chars().take(200).collect::<String>())
                                            .unwrap_or_default();
                                        let _ = project::ProjectStore::for_workspace(&ws.root)
                                            .record_activity(
                                                &request,
                                                app.agent_made_edits,
                                                "none",
                                            );
                                    }

                                    // Auto design-check: after a UI turn, lint the
                                    // edited stylesheet/component files against the
                                    // project's design principles and surface any
                                    // inconsistencies. Advisory — never blocks.
                                    if let Some(ws) = app.cached_workspace.clone()
                                        && let Some(principles) =
                                            project::ProjectStore::for_workspace(&ws.root)
                                                .load_design()
                                    {
                                        let mut lines = Vec::new();
                                        for path in &app.turn_edited_files {
                                            if design::is_ui_file(path)
                                                && let Ok(content) = std::fs::read_to_string(path)
                                            {
                                                for f in design::lint_design(
                                                    path,
                                                    &content,
                                                    Some(&principles),
                                                )
                                                .into_iter()
                                                .take(6)
                                                {
                                                    lines.push(format!(
                                                        "- {}:{} [{}] {}",
                                                        path, f.line, f.rule, f.message
                                                    ));
                                                }
                                            }
                                        }
                                        if !lines.is_empty() {
                                            app.add_assistant_message(format!(
                                                "Design check against .nerve/design.md — \
                                                 {} issue(s) in edited UI files:\n{}",
                                                lines.len(),
                                                lines.join("\n")
                                            ));
                                        }
                                    }

                                    app.agent_iterations = 0;
                                    app.verify_rounds = 0;
                                    app.agent_made_edits = false;
                                }
                            }
                        }

                        // Auto-agent cleanup: once the whole request has
                        // finished (this Done carries no outstanding tool
                        // calls, so agent_iterations is back to 0), revert an
                        // intent-detected activation so the next plain message
                        // starts clean. Correctly handles turns that DID run
                        // tools (the flag now survives the tool loop).
                        app.revert_auto_agent_activation();

                        // Pipeline: the current role's turn is complete
                        // (no tool calls outstanding — those take the
                        // agent-mode branch above). Advance to the next
                        // role and spawn its streaming call, or finalise
                        // the workflow if we've hit Done.
                        advance_pipeline_if_active(app, &provider).await;

                        finished = true;
                        break;
                    }
                    StreamEvent::Error(e) => {
                        app.streaming_response.push_str(&format!("\n[Error: {e}]"));
                        app.finish_streaming();
                        // If a stream errors mid-workflow, tear the pipeline
                        // down instead of leaving it half-active — otherwise
                        // the user's next message would be silently consumed by
                        // the stuck pipeline / left in agent mode.
                        if app.pipeline.take().is_some() {
                            app.agent_mode = false;
                            app.set_status("Workflow stopped (stream error)");
                        } else {
                            // A stream error ends the request; force the tool
                            // loop closed so a silent auto-agent activation is
                            // reverted even if the error hit mid-loop.
                            app.agent_iterations = 0;
                            app.revert_auto_agent_activation();
                        }
                        finished = true;
                        break;
                    }
                    StreamEvent::AgentToolsComplete { user_message } => {
                        // The async tool runner has finished all tool
                        // calls for this agent turn. Inject the aggregated
                        // results as a user message, compact the
                        // conversation, and kick off the next LLM call on
                        // a fresh channel.
                        app.add_user_message(user_message);

                        // Rebuild the outgoing messages exactly like the initial
                        // send (shared helper) so the follow-up turn keeps the
                        // mode/pipeline prompt, KB context and @file expansion —
                        // previously the hand-rolled rebuild silently dropped
                        // these after the first tool round. Search KB/auto-context
                        // on the latest REAL user request, not the tool results we
                        // just injected as a user message.
                        let search_query = app
                            .current_conversation()
                            .messages
                            .iter()
                            .rev()
                            .find(|(role, content)| {
                                role == "user"
                                    && !content.starts_with(conversation::TOOL_RESULTS_PREFIX)
                            })
                            .map(|(_, c)| c.clone())
                            .unwrap_or_default();
                        let messages = build_context_messages(app, &search_query);

                        // Reuse the model routing picked for this turn so a
                        // multi-round agent turn never switches models between
                        // tool rounds (the continuation no longer carries the
                        // original request text to re-classify).
                        let model = app
                            .active_turn_model
                            .clone()
                            .unwrap_or_else(|| app.selected_model.clone());
                        let (tx, new_rx) = tokio::sync::mpsc::unbounded_channel();
                        app.cancel_active_stream();
                        app.stream_rx = Some(new_rx);
                        app.is_streaming = true;
                        app.streaming_response.clear();
                        app.streaming_start = Some(std::time::Instant::now());

                        let provider_clone = Arc::clone(&provider);
                        let handle = tokio::spawn(async move {
                            if let Err(e) = provider_clone
                                .chat_stream(&messages, &model, tx.clone())
                                .await
                            {
                                let _ = tx.send(StreamEvent::Error(e.to_string()));
                            }
                        });
                        app.stream_abort = Some(handle.abort_handle());
                        break;
                    }
                }
            }
            // Put the receiver back if we aren't finished AND the branch
            // handlers haven't already installed a new receiver (agent
            // mode and AgentToolsComplete both replace app.stream_rx).
            if !finished && app.stream_rx.is_none() {
                app.stream_rx = Some(rx);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
