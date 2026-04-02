//! Info commands: /help, /status, /version, /about, /summary, /context, /tokens

use crate::app::App;

/// Handle info-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str) -> bool {
    if trimmed == "/help" {
        return handle_help(app);
    }

    if trimmed == "/status" {
        return handle_status(app);
    }

    if trimmed == "/summary" {
        return handle_summary(app);
    }

    if trimmed == "/tokens" {
        return handle_tokens(app);
    }

    if trimmed == "/context" {
        return handle_context(app);
    }

    if trimmed == "/compact" {
        return handle_compact(app);
    }

    false
}

fn handle_help(app: &mut App) -> bool {
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
  /copy <n>           Copy message #n (counting from bottom)\n\
  /copy code          Copy last code block from AI response\n\
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
  /ollama             Manage Ollama models (list/pull/remove)\n\
  /code on|off        Toggle code mode (Claude Code only)\n\
  /agent on|off|status Toggle agent mode (AI tool loop)\n\
  /agent undo          Roll back to pre-agent git checkpoint\n\
  /agent diff          Show what the agent changed (git diff)\n\
  /agent commit [msg]  Commit agent changes\n\
  /mode <name>        Switch mode (efficient/thorough/agent/learning/auto/code/review)\n\
  /autocontext        Auto-gather project context (alias: /ac)\n\
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
Power User\n\
  /alias              List all aliases\n\
  /alias <n> <cmd>    Create alias (e.g. /alias t /run cargo test)\n\
  /!!                 Recall last input (press Enter to send)\n\
  /repeat             Same as /!!\n\
  Up/Down (Insert)    Browse input history\n\
  Home/End (Insert)   Jump to start/end of input\n\
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
  /map [depth]        Show project map (file tree + symbols)\n\
  /tree [depth]       Alias for /map\n\
\n\
Usage & Cost\n\
  /usage              Show session usage stats (estimated)\n\
  /cost               Alias for /usage\n\
  /limit              Show spending limit info\n\
  /limit on           Enable spending limit\n\
  /limit off          Disable spending limit\n\
  /limit set <$>      Set spending limit amount\n\
\n\
System\n\
  /status             Show system status\n\
  /help               Show this help";
    app.current_conversation_mut()
        .messages
        .push(("assistant".into(), help.into()));
    app.scroll_offset = 0;
    true
}

fn handle_status(app: &mut App) -> bool {
    let code_status = if app.code_mode { "ON" } else { "OFF" };
    let conv = app.current_conversation();
    let conv_title = &conv.title;
    let conv_msg_count = conv.messages.len();

    // Gather KB stats.
    let (kb_docs, kb_chunks) = match crate::knowledge::KnowledgeBase::load("default") {
        Ok(kb) => (kb.documents.len(), kb.total_chunks()),
        Err(_) => (0, 0),
    };

    let clip_count = app.clipboard_manager.entries().len();
    let history_count = crate::history::list_conversations()
        .map(|v| v.len())
        .unwrap_or(0);

    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("nerve")
        .join("config.toml");
    let history_path = crate::history::history_dir();

    let agent_status = if app.agent_mode { "ON" } else { "OFF" };
    let mut status = format!(
        "Nerve v0.1.0\n{}\n\
         Provider:   {}\n\
         Model:      {}\n\
         Code Mode:  {}\n\
         Agent Mode: {}\n\
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
        "=".repeat(25),
        app.selected_provider,
        app.selected_model,
        code_status,
        agent_status,
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

    // Usage section
    status.push_str("\n\nUsage\n");
    status.push_str(&format!("  Requests: {}\n", app.usage_stats.total_requests));
    status.push_str(&format!(
        "  Tokens:   ~{} sent, ~{} received\n",
        app.usage_stats.total_tokens_sent, app.usage_stats.total_tokens_received
    ));
    status.push_str(&format!("  Cost:     {}\n", app.usage_stats.format_cost()));
    if app.spending_limit.enabled {
        status.push_str(&format!(
            "  Limit:    ${:.2}/session\n",
            app.spending_limit.max_cost_usd
        ));
    }

    app.add_assistant_message(status);
    app.scroll_offset = 0;
    true
}

fn handle_summary(app: &mut App) -> bool {
    let conv = app.current_conversation();
    if conv.messages.is_empty() {
        app.add_assistant_message("No messages to summarize.".into());
        return true;
    }

    let mut summary = format!("Conversation Summary: {}\n", conv.title);
    summary.push_str(&format!("{}\n\n", "=".repeat(40)));

    let user_count = conv.messages.iter().filter(|(r, _)| r == "user").count();
    let ai_count = conv
        .messages
        .iter()
        .filter(|(r, _)| r == "assistant")
        .count();
    let total_words: usize = conv
        .messages
        .iter()
        .map(|(_, c)| c.split_whitespace().count())
        .sum();
    let total_tokens = conv
        .messages
        .iter()
        .map(|(_, c)| c.len() / 4 + 1)
        .sum::<usize>();

    summary.push_str(&format!("Messages: {} user, {} AI\n", user_count, ai_count));
    summary.push_str(&format!("Words: {}\n", total_words));
    summary.push_str(&format!("Estimated tokens: ~{}\n\n", total_tokens));

    summary.push_str("Topics discussed:\n");
    for (i, (role, content)) in conv.messages.iter().enumerate() {
        if role == "user" {
            let brief: String = content.chars().take(80).collect();
            summary.push_str(&format!("  {}. {}", i / 2 + 1, brief));
            if content.len() > 80 {
                summary.push_str("...");
            }
            summary.push('\n');
        }
    }

    app.add_assistant_message(summary);
    app.scroll_offset = 0;
    true
}

fn handle_compact(app: &mut App) -> bool {
    let limit = crate::agent::context::ContextManager::recommended_limit(&app.selected_provider);
    let cm = crate::agent::context::ContextManager::new(limit);
    let messages = app.current_conversation().messages.clone();

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
            - crate::agent::context::ContextManager::conversation_tokens(
                &app.current_conversation().messages,
            );
        app.set_status(format!(
            "Compacted: {} \u{2192} {} messages (~{} tokens saved)",
            before, after, saved_tokens
        ));
    }
    true
}

fn handle_tokens(app: &mut App) -> bool {
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
        msg.push_str(&format!(
            "  {:>3}. [{:>9}] ~{:>6} tokens\n",
            i + 1,
            role,
            tokens
        ));
    }

    if pct > 70.0 {
        msg.push_str(&format!(
            "\nWarning: {:.0}% of context used. Consider /compact to save tokens.",
            pct
        ));
    }

    app.add_assistant_message(msg);
    app.scroll_offset = 0;
    true
}

fn handle_context(app: &mut App) -> bool {
    let conv = app.current_conversation();
    let mut ctx = String::from("Current AI Context:\n\n");

    for (i, (role, content)) in conv.messages.iter().enumerate() {
        let tokens = crate::agent::context::ContextManager::estimate_tokens(content);
        let preview: String = content.chars().take(60).collect();
        let ellipsis = if content.len() > 60 { "..." } else { "" };

        ctx.push_str(&format!(
            "  {:>3}. [{}] ~{} tokens: {}{}\n",
            i + 1,
            role,
            tokens,
            preview,
            ellipsis
        ));
    }

    let total = crate::agent::context::ContextManager::conversation_tokens(&conv.messages);
    ctx.push_str(&format!(
        "\nTotal: {} messages, ~{} tokens estimated\n",
        conv.messages.len(),
        total
    ));

    let ws_for_ctx = app
        .cached_workspace
        .as_ref()
        .cloned()
        .or_else(crate::workspace::detect_workspace);
    if let Some(ws) = ws_for_ctx {
        ctx.push_str(&format!("Workspace: {} ({:?})\n", ws.name, ws.project_type));
    }

    app.add_assistant_message(ctx);
    app.scroll_offset = 0;
    true
}
