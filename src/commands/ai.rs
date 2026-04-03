//! AI commands: /provider, /providers, /model, /models, /ollama, /agent,
//! /mode, /autocontext, /code, /cwd, /cd

use crate::app::{self, App};

/// Handle AI-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str) -> bool {
    if trimmed == "/models" {
        return handle_models(app);
    }

    if let Some(rest) = trimmed.strip_prefix("/model ") {
        return handle_model_switch(app, rest);
    }

    if trimmed == "/providers" {
        return handle_providers(app);
    }

    if trimmed == "/provider" {
        return handle_provider_bare(app);
    }

    if let Some(rest) = trimmed.strip_prefix("/provider ") {
        return handle_provider_switch(app, rest);
    }

    if crate::shell::matches_command(trimmed, "/ollama") {
        return handle_ollama(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/agent") {
        return handle_agent(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/cd") {
        return handle_cd(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/code") {
        return handle_code(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/cwd") {
        return handle_cwd(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/mode") {
        return handle_mode(app, trimmed);
    }

    if trimmed == "/autocontext" || trimmed == "/ac" {
        return handle_autocontext(app);
    }

    false
}

fn handle_models(app: &mut App) -> bool {
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
    true
}

fn handle_model_switch(app: &mut App, rest: &str) -> bool {
    let name = rest.trim();
    if name.is_empty() {
        app.set_status("Usage: /model <model-name>");
        return true;
    }
    let matched = app
        .available_models
        .iter()
        .find(|m| m.as_str() == name)
        .or_else(|| app.available_models.iter().find(|m| m.starts_with(name)))
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
    true
}

fn handle_providers(app: &mut App) -> bool {
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
        if current == "claude_code" || current == "claude" {
            "*"
        } else {
            " "
        },
        if current == "ollama" { "*" } else { " " },
        if current == "openai" { "*" } else { " " },
        if current == "openrouter" { "*" } else { " " },
        if current == "copilot" || current == "gh" {
            "*"
        } else {
            " "
        },
        current
    );
    app.add_assistant_message(list);
    app.scroll_offset = 0;
    true
}

fn handle_provider_bare(app: &mut App) -> bool {
    app.add_assistant_message(format!(
        "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter, copilot",
        app.selected_provider
    ));
    app.scroll_offset = 0;
    true
}

fn handle_provider_switch(app: &mut App, rest: &str) -> bool {
    let name = rest.trim();
    if name.is_empty() {
        app.add_assistant_message(format!(
            "Current provider: {}\nUse /provider <name> to switch.\nAvailable: claude_code, ollama, openai, openrouter, copilot",
            app.selected_provider
        ));
        app.scroll_offset = 0;
        return true;
    }
    let valid = [
        "claude_code",
        "claude",
        "ollama",
        "openai",
        "openrouter",
        "copilot",
        "gh",
    ];
    if valid.contains(&name) {
        app.selected_provider = name.to_string();
        app.provider_changed = true;
        app.selected_model = crate::default_model_for_provider(&app.selected_provider).into();
        app.available_models = crate::models_for_provider(&app.selected_provider);
        app.set_status(format!("Switched to provider: {name}"));
    } else {
        app.add_assistant_message(format!(
            "Unknown provider: {name}\nAvailable: claude_code, ollama, openai, openrouter, copilot"
        ));
    }
    true
}

fn handle_ollama(app: &mut App, trimmed: &str) -> bool {
    let ollama_rest = trimmed.strip_prefix("/ollama").unwrap_or("").trim();
    let ollama_args: Vec<String> = ollama_rest
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let subcmd = ollama_args.first().map(|s| s.as_str()).unwrap_or("list");
    match subcmd {
        "list" => {
            let models = crate::detect_ollama_models();
            let mut msg = "Ollama models on this machine:\n\n".to_string();
            for model in &models {
                let active = if *model == app.selected_model {
                    " (active)"
                } else {
                    ""
                };
                msg.push_str(&format!("  {model}{active}\n"));
            }
            msg.push_str("\nUse: /model <name> to switch\nPull new: /ollama pull <name>");
            app.add_assistant_message(msg);
        }
        "pull" => {
            if let Some(model_name) = ollama_args.get(1) {
                app.set_status(format!("Pulling {model_name}... (this may take a while)"));
                app.add_assistant_message(format!(
                    "Pulling Ollama model: {model_name}\n\n\
                     This runs in the background. \
                     Use `/ollama list` to check when it's available."
                ));
                let name = model_name.clone();
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("ollama")
                        .args(["pull", &name])
                        .output();
                });
            } else {
                app.set_status("Usage: /ollama pull <model_name>".to_string());
            }
        }
        "remove" | "rm" => {
            if let Some(model_name) = ollama_args.get(1) {
                match std::process::Command::new("ollama")
                    .args(["rm", model_name])
                    .output()
                {
                    Ok(output) if output.status.success() => {
                        app.set_status(format!("Removed {model_name}"));
                        if app.selected_provider == "ollama" {
                            app.available_models = crate::detect_ollama_models();
                        }
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        app.set_status(format!("Failed: {stderr}"));
                    }
                    Err(e) => app.report_error(e),
                }
            } else {
                app.set_status("Usage: /ollama remove <model_name>".to_string());
            }
        }
        _ => {
            app.add_assistant_message(
                "Ollama Commands:\n\n  \
                 /ollama list          Show installed models\n  \
                 /ollama pull <name>   Download a model\n  \
                 /ollama remove <name> Remove a model\n\n\
                 Popular models: llama3, mistral, codellama, qwen2.5"
                    .into(),
            );
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_agent(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/agent").unwrap_or("").trim();
    match rest {
        "on" => {
            app.agent_mode = true;
            let tools_prompt = crate::agent::tools::tools_system_prompt();
            app.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("You have access to the following tools")
                        || c.contains("You are Nerve, an AI coding assistant")))
            });
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), tools_prompt));
            let ws_for_agent = app
                .cached_workspace
                .clone()
                .or_else(crate::workspace::detect_workspace);
            if let Some(ws) = ws_for_agent {
                let project_map = crate::workspace::generate_project_map(&ws.root, 3);
                let map_context = if project_map.len() > 2000 {
                    format!("{}...\n[Project map truncated]", &project_map[..2000])
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
            let git_status =
                crate::shell::run_command("git rev-parse --is-inside-work-tree 2>/dev/null");
            if let Ok(ref result) = git_status {
                if result.stdout.trim() == "true" {
                    let stash_result = crate::shell::run_command(
                        "git stash push -m 'nerve-agent-checkpoint' --include-untracked 2>/dev/null",
                    );
                    if let Ok(ref sr) = stash_result {
                        if sr.stdout.contains("Saved") {
                            app.agent_has_stash = true;
                            app.set_status(
                                "Agent mode ON \u{2014} git checkpoint saved, AI has tool access",
                            );
                        } else {
                            app.agent_has_stash = false;
                            app.set_status("Agent mode ON \u{2014} AI has tool access");
                        }
                    } else {
                        app.set_status("Agent mode ON \u{2014} AI has tool access");
                    }
                } else {
                    app.set_status(
                        "Agent mode ON \u{2014} AI has tool access (no git repo detected)",
                    );
                }
            } else {
                app.set_status("Agent mode ON \u{2014} AI has tool access (no git repo detected)");
            }
        }
        "off" => {
            app.agent_mode = false;
            app.agent_iterations = 0;
            crate::agent::tools::reset_tool_counter();
            app.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("You have access to the following tools")
                        || c.contains("You are Nerve, an AI coding assistant")))
            });
            app.set_status("Agent mode OFF \u{2014} chat only");
        }
        "status" => {
            let tools = crate::agent::tools::available_tools();
            let mut msg = format!(
                "Agent Mode: {}\n\n",
                if app.agent_mode { "ACTIVE" } else { "INACTIVE" }
            );
            msg.push_str(&format!(
                "Iterations this task: {}/10\n",
                app.agent_iterations
            ));
            msg.push_str(&format!("Available tools ({}):\n", tools.len()));
            for tool in &tools {
                msg.push_str(&format!("  - {}: {}\n", tool.name, tool.description));
            }
            msg.push_str(&format!(
                "\nCode mode: {}\n",
                if app.code_mode { "ON" } else { "OFF" }
            ));
            if let Some(ref dir) = app.working_dir {
                msg.push_str(&format!("Working directory: {dir}\n"));
            }
            app.add_assistant_message(msg);
        }
        "undo" | "rollback" => {
            if !app.agent_has_stash {
                app.set_status("No agent checkpoint to restore");
            } else {
                match crate::shell::run_command("git stash pop") {
                    Ok(result) => {
                        if result.success {
                            app.agent_has_stash = false;
                            app.set_status("Agent changes rolled back to checkpoint");
                        } else {
                            app.set_status(format!("Rollback failed: {}", result.stderr));
                        }
                    }
                    Err(e) => app.set_status(format!("Rollback error: {e}")),
                }
            }
        }
        "diff" => match crate::shell::run_command("git diff") {
            Ok(result) => {
                if result.stdout.trim().is_empty() {
                    app.add_assistant_message("No changes detected since agent started.".into());
                } else {
                    let diff = format!("Agent changes:\n\n```diff\n{}\n```", result.stdout);
                    app.add_assistant_message(diff);
                }
            }
            Err(e) => app.report_error(e),
        },
        _ if rest.starts_with("commit") => {
            let commit_rest = rest.strip_prefix("commit").unwrap_or("").trim();
            let msg = if commit_rest.is_empty() {
                "Changes made by Nerve agent".to_string()
            } else {
                commit_rest.to_string()
            };
            let cmd = format!(
                "git add -A && git commit -m '{}'",
                msg.replace('\'', "'\\''"),
            );
            match crate::shell::run_command(&cmd) {
                Ok(result) => {
                    if result.success {
                        app.agent_has_stash = false;
                        app.set_status("Agent changes committed");
                    } else {
                        app.set_status(format!("Commit failed: {}", result.stderr));
                    }
                }
                Err(e) => app.report_error(e),
            }
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
                 Usage: /agent on | off | status | undo | diff | commit [message]"
            ));
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_cd(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/cd").unwrap_or("").trim();
    if rest.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_default();
        app.add_assistant_message(format!("Current directory: {}", cwd.display()));
    } else {
        let target = rest;
        let target_path = if let Some(stripped) = target.strip_prefix("~/") {
            dirs::home_dir().unwrap_or_default().join(stripped)
        } else {
            std::path::PathBuf::from(target)
        };

        match std::env::set_current_dir(&target_path) {
            Ok(()) => {
                app.set_status(format!("Changed to {}", target_path.display()));
                let ws = crate::workspace::detect_workspace();
                if let Some(ref ws) = ws {
                    app.set_status(format!(
                        "Changed to {} \u{2014} detected {:?} project: {}",
                        target_path.display(),
                        ws.project_type,
                        ws.name
                    ));
                }
                app.cached_workspace = ws;
            }
            Err(e) => app.report_error(e),
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_code(app: &mut App, trimmed: &str) -> bool {
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
    true
}

fn handle_cwd(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/cwd").unwrap_or("").trim();
    if rest.is_empty() {
        let current = app.working_dir.as_deref().unwrap_or("(not set)");
        app.add_assistant_message(format!(
            "Working directory: {current}\n\
             Use /cwd <path> to set a directory for Claude Code file access."
        ));
    } else {
        let path = std::path::Path::new(rest);
        if path.is_dir() {
            app.working_dir = Some(rest.to_string());
            app.provider_changed = true;
            app.set_status(format!("Working directory set to {rest}"));
        } else {
            app.set_status(format!("Not a directory: {rest}"));
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_mode(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/mode").unwrap_or("").trim();
    match rest {
        "efficient" | "eco" => {
            app.active_mode = app::NerveMode::Efficient;
            app.mode_name = "efficient".into();
            let sys = "You are a helpful assistant. Be concise and direct. \
                       Avoid unnecessary explanations. Use bullet points over paragraphs. \
                       Show code without verbose commentary. One-sentence answers when possible.";
            app.current_conversation_mut()
                .messages
                .retain(|(r, c)| !(r == "system" && c.contains("Be concise")));
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys.into()));
            app.set_status("Mode: Efficient \u{2014} concise responses, auto-compact enabled");
        }
        "thorough" | "detailed" => {
            app.active_mode = app::NerveMode::Thorough;
            app.mode_name = "thorough".into();
            let sys = "You are a thorough, expert assistant. Provide detailed explanations with examples. \
                       Show your reasoning step by step. Include edge cases and caveats. \
                       When showing code, explain the key design decisions.";
            app.current_conversation_mut()
                .messages
                .retain(|(r, c)| !(r == "system" && c.contains("thorough")));
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys.into()));
            app.set_status("Mode: Thorough \u{2014} detailed responses with explanations");
        }
        "agent" => {
            app.active_mode = app::NerveMode::Agent;
            app.mode_name = "agent".into();
            app.agent_mode = true;
            let tools_prompt = crate::agent::tools::tools_system_prompt();
            app.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("You have access to the following tools")
                        || c.contains("You are Nerve, an AI coding assistant")))
            });
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), tools_prompt));
            app.set_status("Mode: Agent \u{2014} AI has tool access for coding tasks");
        }
        "learning" | "teach" => {
            app.active_mode = app::NerveMode::Learning;
            app.mode_name = "learning".into();
            let sys = "You are a patient teacher. Explain concepts from first principles. \
                       Use analogies and real-world examples. Break complex topics into simple steps. \
                       After explaining, ask a question to check understanding. \
                       Provide exercises when appropriate.";
            app.current_conversation_mut()
                .messages
                .retain(|(r, c)| !(r == "system" && c.contains("patient teacher")));
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys.into()));
            app.set_status("Mode: Learning \u{2014} explanations optimized for understanding");
        }
        "auto" => {
            app.active_mode = app::NerveMode::Efficient;
            app.mode_name = "auto".into();
            let mut sys = String::from(
                "You are a helpful coding assistant. Be concise and practical. \
                 When showing code, include only the relevant parts. \
                 When explaining, use bullet points. \
                 Always consider the project context provided.",
            );

            if let Some(ref ws) = app.cached_workspace {
                sys.push_str(&format!(
                    "\n\nProject: {} ({:?})\nTech: {}",
                    ws.name,
                    ws.project_type,
                    ws.tech_stack.join(", ")
                ));
            }

            let cwd = std::env::current_dir().unwrap_or_default();
            if let Ok(entries) = std::fs::read_dir(&cwd) {
                let key_files: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        matches!(
                            name.as_str(),
                            "README.md"
                                | "Cargo.toml"
                                | "package.json"
                                | "pyproject.toml"
                                | "go.mod"
                                | "Makefile"
                        )
                    })
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                if !key_files.is_empty() {
                    sys.push_str(&format!("\nKey files: {}", key_files.join(", ")));
                }
            }

            app.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("Be concise") || c.contains("helpful coding assistant")))
            });
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys));
            app.set_status("Mode: Auto \u{2014} workspace-aware, auto-compact, concise");
        }
        "code" => {
            app.active_mode = app::NerveMode::Efficient;
            app.mode_name = "code".into();
            let sys = "You are an expert programmer. Follow these rules strictly:\n\
                       1. Show ONLY code \u{2014} no explanations unless asked\n\
                       2. Include ALL imports and dependencies\n\
                       3. Code must be complete and runnable (no placeholders like '...')\n\
                       4. Follow the project's existing style and conventions\n\
                       5. Add comments only where the logic isn't obvious\n\
                       6. Handle errors properly \u{2014} no unwrap() or bare exceptions\n\
                       7. If asked to modify existing code, show only the changed parts with enough context to locate them";
            app.current_conversation_mut()
                .messages
                .retain(|(r, c)| !(r == "system" && c.contains("expert programmer")));
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys.into()));
            app.set_status("Mode: Code \u{2014} code-only responses, no verbose explanations");
        }
        "review" => {
            app.active_mode = app::NerveMode::Thorough;
            app.mode_name = "review".into();
            let sys = "You are a senior code reviewer. For every piece of code:\n\
                       1. Check for bugs, edge cases, and error handling gaps\n\
                       2. Check for security issues (injection, auth, data exposure)\n\
                       3. Check for performance issues (complexity, allocations, queries)\n\
                       4. Check for readability (naming, structure, unnecessary complexity)\n\
                       5. Rate severity: CRITICAL / WARNING / SUGGESTION\n\
                       6. Provide the fix for each issue found\n\
                       7. End with an overall quality score (1-10)";
            app.current_conversation_mut()
                .messages
                .retain(|(r, c)| !(r == "system" && c.contains("senior code reviewer")));
            app.current_conversation_mut()
                .messages
                .insert(0, ("system".into(), sys.into()));
            app.set_status("Mode: Review \u{2014} structured code review with severity ratings");
        }
        "standard" | "default" | "reset" => {
            app.active_mode = app::NerveMode::Standard;
            app.mode_name = "standard".into();
            app.agent_mode = false;
            app.current_conversation_mut().messages.retain(|(r, c)| {
                !(r == "system"
                    && (c.contains("Be concise")
                        || c.contains("thorough")
                        || c.contains("You have access to the following tools")
                        || c.contains("You are Nerve, an AI coding assistant")
                        || c.contains("patient teacher")
                        || c.contains("helpful coding assistant")
                        || c.contains("expert programmer")
                        || c.contains("senior code reviewer")))
            });
            app.set_status("Mode: Standard \u{2014} default behavior");
        }
        _ => {
            let current = format!("{:?}", app.active_mode);
            app.add_assistant_message(format!(
                "Current mode: {current}\n\n\
                 Available modes:\n\n\
                 /mode standard    Default behavior\n\
                 /mode efficient   Concise responses, saves tokens\n\
                 /mode thorough    Detailed explanations with examples\n\
                 /mode agent       Coding agent with file/command tools\n\
                 /mode learning    Patient explanations, exercises\n\
                 /mode auto        Workspace-aware, auto-context, auto-compact\n\
                 /mode code        Code-only responses, no verbose explanations\n\
                 /mode review      Structured code review with severity ratings\n\n\
                 Each mode adjusts the AI's system prompt and context behavior."
            ));
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_autocontext(app: &mut App) -> bool {
    let mut context_parts: Vec<String> = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();

    if let Some(ref ws) = app.cached_workspace {
        context_parts.push(format!(
            "Project: {} ({:?})\nTech: {}\nRoot: {}",
            ws.name,
            ws.project_type,
            ws.tech_stack.join(", "),
            ws.root.display()
        ));
    }

    for filename in &[
        "README.md",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
    ] {
        let path = cwd.join(filename);
        if path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let truncated: String = content.chars().take(500).collect();
            context_parts.push(format!("{}:\n{}", filename, truncated));
        }
    }

    if let Some(ref ws) = app.cached_workspace {
        let map = crate::workspace::generate_project_map(&ws.root, 2);
        let truncated: String = map.chars().take(1000).collect();
        context_parts.push(truncated);
    }

    if let Ok(result) = crate::shell::run_command("git log --oneline -5 2>/dev/null")
        && result.success
        && !result.stdout.trim().is_empty()
    {
        context_parts.push(format!("Recent commits:\n{}", result.stdout));
    }

    if context_parts.is_empty() {
        app.set_status("No project context detected");
    } else {
        let full_context = context_parts.join("\n\n---\n\n");
        let token_est = full_context.len() / 4;
        app.current_conversation_mut()
            .messages
            .push(("system".into(), full_context));
        app.set_status(format!("Auto-context loaded (~{} tokens)", token_est));
    }
    true
}
