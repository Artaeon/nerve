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

    if trimmed == "/suggest" || trimmed.starts_with("/suggest ") {
        return handle_suggest(app, trimmed);
    }

    false
}

/// `/suggest <query>` — rank all known SmartPrompts against the query
/// using BM25 and show the top matches so the user can pick one by
/// number with `/prompt <name>` or Ctrl+K.
fn handle_suggest(app: &mut App, trimmed: &str) -> bool {
    let query = trimmed
        .strip_prefix("/suggest")
        .unwrap_or("")
        .trim()
        .to_string();
    if query.is_empty() {
        app.add_assistant_message(
            "Usage: /suggest <what you want to do>\n\
             \n\
             Example: /suggest fix a bug in my rust code\n\
             \n\
             Ranks the prompt library by relevance and returns the top 5 matches."
                .into(),
        );
        return true;
    }

    let results = crate::prompts::search::suggest(&query, 5);
    if results.is_empty() {
        app.add_assistant_message(format!(
            "No prompts matched \"{query}\".\n\nPress Ctrl+K to browse the full library."
        ));
        return true;
    }

    let mut out = format!("Top matches for \"{query}\":\n\n");
    for (i, (p, score)) in results.iter().enumerate() {
        out.push_str(&format!(
            "{n}. [{cat}] {name} (score {s:.2})\n   {desc}\n",
            n = i + 1,
            cat = p.category,
            name = p.name,
            s = score,
            desc = p.description,
        ));
    }
    out.push_str("\nOpen Ctrl+K to use any of these, or filter by the prompt name.");
    app.add_assistant_message(out);
    true
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
  /agent commit [msg]  Commit agent changes (runs tests first; blocked on failure)\n\
  /agent commit force [msg]  Commit without the test gate\n\
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
Project Memory (.nerve/ — injected into every prompt)\n\
  /init               Analyze repo and save an engineering brief\n\
  /remember <fact>    Persist a fact/convention about this project\n\
  /memory             Show project memory (brief + facts + decisions)\n\
  /decision <text>    Record a decision in the decision log\n\
  /decisions          Show the decision log\n\
  /changes            Show the agent change journal\n\
  /activity           Show recent auto-captured agent activity\n\
  /improve <idea>     Add an idea to the improvement backlog\n\
  /improvements       List backlog (done <id> to close)\n\
  /task <title>       Add a task to the persistent backlog\n\
  /tasks              List the task backlog\n\
  /task done|start|fail <id>  Update a task's status\n\
\n\
Shell & Git\n\
  /run <command>      Run shell command and show output\n\
  /pipe <command>     Run command and add output as context\n\
  /diff [args]        Show git diff (adds as context)\n\
  /test               Auto-detect and run project tests\n\
  /build              Auto-detect and run project build\n\
  /lint               Auto-detect and run linter\n\
  /format (/fmt)      Auto-detect and run code formatter\n\
  /search <pattern>   Search codebase with ripgrep\n\
  /git [subcommand]   Quick git operations (status/log/diff/branch)\n\
  /commit [message]   Stage all and commit (AI-generates message if omitted)\n\
  /stage [files...]   Stage files (all if no args)\n\
  /unstage [files...] Unstage files (all if no args)\n\
  /gitbranch [name]   Create/switch branch, or list branches\n\
  /gitbranch switch <name>  Switch to existing branch\n\
  /gitbranch delete <name>  Delete a branch\n\
  /stash [message]    Stash changes\n\
  /stash pop|list|show|drop|apply  Manage stashes\n\
  /log [n]            Show git log (default 10)\n\
  /gitstatus          Show full git status\n\
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
Prompt library\n\
  Ctrl+K              Browse the full SmartPrompt library\n\
  /suggest <query>    Rank prompts by relevance to a free-form query\n\
\n\
Multi-agent workflow\n\
  /workflow <task>    Run planner -> coder -> reviewer on a task\n\
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
        "Nerve v{}\n{}\n\
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
        env!("CARGO_PKG_VERSION"),
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
    // Same estimator as the status bar and compaction for a consistent number.
    let total_tokens = crate::agent::context::ContextManager::conversation_tokens(&conv.messages);

    summary.push_str(&format!("Messages: {user_count} user, {ai_count} AI\n"));
    summary.push_str(&format!("Words: {total_words}\n"));
    summary.push_str(&format!("Estimated tokens: ~{total_tokens}\n\n"));

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
    let limit = crate::agent::context::ContextManager::effective_limit(
        &app.selected_provider,
        app.context_limit_override,
    );
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
            "Compacted: {before} \u{2192} {after} messages (~{saved_tokens} tokens saved)"
        ));
    }
    true
}

fn handle_tokens(app: &mut App) -> bool {
    let conv = app.current_conversation();
    let total = crate::agent::context::ContextManager::conversation_tokens(&conv.messages);
    let limit = crate::agent::context::ContextManager::effective_limit(
        &app.selected_provider,
        app.context_limit_override,
    );
    let pct = (total as f64 / limit as f64 * 100.0).min(100.0);

    let mut msg = format!("Token Usage\n{}\n\n", "=".repeat(30));
    msg.push_str(&format!("Estimated tokens: ~{total}\n"));
    msg.push_str(&format!("Provider limit:   ~{limit}\n"));
    msg.push_str(&format!("Usage:            {pct:.1}%\n\n"));

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
            "\nWarning: {pct:.0}% of context used. Consider /compact to save tokens."
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── /help ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn help_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/help").await);
    }

    #[tokio::test]
    async fn help_contains_key_commands() {
        let mut app = App::new();
        handle(&mut app, "/help").await;
        let last = app.current_conversation().messages.last().unwrap();
        let text = &last.1;
        assert!(text.contains("/clear"));
        assert!(text.contains("/new"));
        assert!(text.contains("/model"));
        assert!(text.contains("/export"));
        assert!(text.contains("/help"));
        assert!(text.contains("/status"));
        assert!(text.contains("/agent"));
        assert!(text.contains("/file"));
        assert!(text.contains("/kb"));
        assert!(text.contains("/plugin"));
    }

    #[tokio::test]
    async fn help_added_as_assistant_message() {
        let mut app = App::new();
        handle(&mut app, "/help").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert_eq!(last.0, "assistant");
    }

    // ── /status ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn status_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/status").await);
    }

    #[tokio::test]
    async fn status_contains_system_info() {
        let mut app = App::new();
        handle(&mut app, "/status").await;
        let last = app.current_conversation().messages.last().unwrap();
        let text = &last.1;
        assert!(text.contains("Nerve"));
        assert!(text.contains("Provider"));
        assert!(text.contains("Model"));
        assert!(text.contains("Code Mode"));
        assert!(text.contains("Agent Mode"));
    }

    #[tokio::test]
    async fn status_shows_provider_and_model() {
        let mut app = App::new();
        app.selected_provider = "ollama".into();
        app.selected_model = "llama3".into();
        handle(&mut app, "/status").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("ollama"));
        assert!(last.1.contains("llama3"));
    }

    // ── /summary ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn summary_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/summary").await);
    }

    #[tokio::test]
    async fn summary_empty_conversation() {
        let mut app = App::new();
        handle(&mut app, "/summary").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("No messages to summarize"));
    }

    #[tokio::test]
    async fn summary_with_messages() {
        let mut app = App::new();
        app.add_user_message("What is Rust?".into());
        app.add_assistant_message("Rust is a systems programming language.".into());
        handle(&mut app, "/summary").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Conversation Summary"));
        assert!(last.1.contains("user"));
        assert!(last.1.contains("AI"));
    }

    // ── /tokens ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn tokens_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/tokens").await);
    }

    #[tokio::test]
    async fn tokens_shows_breakdown() {
        let mut app = App::new();
        app.add_user_message("hello world".into());
        handle(&mut app, "/tokens").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Token Usage"));
        assert!(last.1.contains("Estimated tokens"));
    }

    // ── /context ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn context_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/context").await);
    }

    #[tokio::test]
    async fn context_shows_messages() {
        let mut app = App::new();
        app.add_user_message("test".into());
        handle(&mut app, "/context").await;
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Current AI Context"));
        assert!(last.1.contains("Total"));
    }

    // ── /compact ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn compact_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/compact").await);
    }

    // ── Unrecognised commands ───────────────────────────────────────────

    #[tokio::test]
    async fn unrecognised_returns_false() {
        let mut app = App::new();
        assert!(!handle(&mut app, "/helpx").await);
        assert!(!handle(&mut app, "/statuses").await);
        assert!(!handle(&mut app, "/token").await);
        assert!(!handle(&mut app, "/summarize").await);
    }

    #[tokio::test]
    async fn suggest_bare_shows_usage() {
        let mut app = App::new();
        assert!(handle(&mut app, "/suggest").await);
        let last = app
            .current_conversation()
            .messages
            .last()
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        assert!(last.contains("Usage"), "expected usage help, got: {last}");
    }

    #[tokio::test]
    async fn suggest_with_query_lists_matches() {
        let mut app = App::new();
        assert!(handle(&mut app, "/suggest fix a bug in my code").await);
        let last = app
            .current_conversation()
            .messages
            .last()
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        assert!(last.contains("Top matches"));
        // "Fix Bug" is a built-in prompt and should rank highly for this query.
        assert!(
            last.contains("Fix Bug"),
            "expected Fix Bug in results, got:\n{last}"
        );
    }

    #[tokio::test]
    async fn suggest_with_nonsense_query_reports_no_match() {
        let mut app = App::new();
        assert!(handle(&mut app, "/suggest xyzzyqwerfloopunmatched").await);
        let last = app
            .current_conversation()
            .messages
            .last()
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        assert!(last.contains("No prompts matched"));
    }
}
