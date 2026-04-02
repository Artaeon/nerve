//! Knowledge commands: /kb, /url, /auto

use std::sync::Arc;

use crate::ai::provider::{AiProvider, ChatMessage, StreamEvent};
use crate::app::App;
use crate::automation;
use crate::knowledge;
use crate::scraper;

use tokio::sync::mpsc;

/// Handle knowledge-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) -> bool {
    if let Some(rest) = trimmed.strip_prefix("/url ") {
        return handle_url(app, rest, provider).await;
    }

    if trimmed == "/auto" || trimmed.starts_with("/auto ") {
        handle_auto(app, trimmed, provider).await;
        return true;
    }

    if trimmed == "/kb" || trimmed.starts_with("/kb ") {
        handle_kb(app, trimmed);
        return true;
    }

    false
}

async fn handle_url(app: &mut App, rest: &str, provider: &Arc<dyn AiProvider>) -> bool {
    let rest = rest.trim();
    if rest.is_empty() {
        app.set_status("Usage: /url <url> [question]");
        return true;
    }

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

    app.set_status(format!("Scraping {url}..."));

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
            app.current_conversation_mut()
                .messages
                .push(("system".into(), context_msg));

            let user_msg = match question {
                Some(q) => q,
                None => format!("I've loaded content from {url}. Please summarise it."),
            };
            app.set_status(format!("Scraped {url} ({} words)", result.word_count));

            app.add_user_message(user_msg);
            app.scroll_offset = 0;
            crate::send_to_ai_from_history(app, provider).await;
        }
        Err(e) => {
            app.set_status(format!("Scrape failed: {e}"));
        }
    }
    true
}

fn handle_kb(app: &mut App, trimmed: &str) {
    let rest = trimmed.strip_prefix("/kb").unwrap_or("").trim();

    // /kb add <directory>
    if let Some(dir_path) = rest.strip_prefix("add ") {
        let dir_path = dir_path.trim();
        if dir_path.is_empty() {
            app.set_status("Usage: /kb add <directory>");
            return;
        }
        let path = std::path::Path::new(dir_path);
        if !path.is_dir() {
            app.set_status(format!("Not a directory: {dir_path}"));
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
                    app.set_status(format!("KB: ingested {count} documents"));
                }
            }
            Err(e) => {
                app.set_status(format!("Ingest failed: {e}"));
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
                            "  {} \u{2014} {} docs, {} chunks, {} words",
                            name,
                            kb.documents.len(),
                            kb.total_chunks(),
                            kb.total_words()
                        ));
                    } else {
                        lines.push(format!("  {} \u{2014} (could not load)", name));
                    }
                }
                app.current_conversation_mut()
                    .messages
                    .push(("assistant".into(), lines.join("\n")));
                app.scroll_offset = 0;
            }
            Err(e) => {
                app.set_status(format!("Failed to list KBs: {e}"));
            }
        }
        return;
    }

    // /kb search <query>
    if let Some(query) = rest.strip_prefix("search ") {
        let query = query.trim();
        if query.is_empty() {
            app.set_status("Usage: /kb search <query>");
            return;
        }
        match knowledge::KnowledgeBase::load("default") {
            Ok(kb) => {
                let results = knowledge::search_knowledge(&kb, query, 5);
                if results.is_empty() {
                    app.current_conversation_mut()
                        .messages
                        .push(("assistant".into(), format!("No results found for: {query}")));
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
                            if r.chunk.content.len() > 200 {
                                "..."
                            } else {
                                ""
                            }
                        ));
                    }
                    app.current_conversation_mut()
                        .messages
                        .push(("assistant".into(), lines.join("\n")));
                }
                app.scroll_offset = 0;
            }
            Err(_) => {
                app.status_message =
                    Some("No default knowledge base found. Use /kb add <directory> first.".into());
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
                app.set_status("KB cleared");
            }
            Err(e) => {
                app.set_status(format!("Failed to clear KB: {e}"));
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

    app.set_status(format!(
        "Unknown /kb command: {rest}. Try /kb add, /kb list, /kb search, /kb clear, or /kb status."
    ));
}

async fn handle_auto(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) {
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
                    "  {} \u{2014} {} ({} steps){}\n",
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
            app.set_status("Usage: /auto info <name>");
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
                app.set_status(format!("Automation '{name}' not found"));
            }
        }
        app.scroll_offset = 0;
        return;
    }

    // /auto delete <name>
    if let Some(name) = rest.strip_prefix("delete ") {
        let name = name.trim();
        if name.is_empty() {
            app.set_status("Usage: /auto delete <name>");
            return;
        }
        let builtin_names: Vec<String> = automation::builtin_automations()
            .iter()
            .map(|a| a.name.to_lowercase())
            .collect();
        if builtin_names.contains(&name.to_lowercase()) {
            app.set_status("Cannot delete built-in automations");
            return;
        }
        match automation::delete_automation(name) {
            Ok(()) => {
                app.set_status(format!("Deleted automation '{name}'"));
            }
            Err(e) => {
                app.set_status(format!("Delete failed: {e}"));
            }
        }
        return;
    }

    // /auto create <name>
    if let Some(name) = rest.strip_prefix("create ") {
        let name = name.trim();
        if name.is_empty() {
            app.set_status("Usage: /auto create <name>");
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
                app.set_status(format!("Create failed: {e}"));
            }
        }
        app.scroll_offset = 0;
        return;
    }

    // /auto run <name>
    if let Some(name) = rest.strip_prefix("run ") {
        let name = name.trim();
        if name.is_empty() {
            app.set_status("Usage: /auto run <name>");
            return;
        }
        match automation::find_automation(name) {
            Ok(auto) => {
                if auto.steps.is_empty() {
                    app.set_status(format!("Automation '{name}' has no steps"));
                    return;
                }

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
                    app.set_status(
                        "No input provided. Type something first or have a previous message.",
                    );
                    return;
                }

                run_automation(app, &auto, &input, provider).await;
            }
            Err(_) => {
                app.set_status(format!("Automation '{name}' not found"));
            }
        }
        return;
    }

    app.set_status("Unknown /auto command. Use: list, run, create, delete, info");
}

/// Execute an automation pipeline.
async fn run_automation(
    app: &mut App,
    automation: &automation::Automation,
    input: &str,
    provider: &Arc<dyn AiProvider>,
) {
    let start = std::time::Instant::now();
    let mut prev_output = String::new();
    let total_steps = automation.steps.len();

    app.set_status(format!("Running automation: {}", automation.name));

    for (i, step) in automation.steps.iter().enumerate() {
        let model_owned = step
            .model
            .clone()
            .unwrap_or_else(|| app.selected_model.clone());
        let prompt = step
            .prompt_template
            .replace("{{input}}", input)
            .replace("{{prev_output}}", &prev_output);

        app.set_status(format!(
            "Automation step {}/{}: {}...",
            i + 1,
            total_steps,
            step.name,
        ));

        if i == total_steps - 1 {
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
            app.set_status(format!(
                "Automation '{}' complete ({total_steps} steps, {elapsed}ms)",
                automation.name,
            ));
        } else {
            let messages = vec![ChatMessage::user(&prompt)];
            match provider.chat(&messages, &model_owned).await {
                Ok(response) => prev_output = response,
                Err(e) => {
                    app.add_assistant_message(format!("Automation error at step {}: {e}", i + 1,));
                    app.set_status(format!("Automation failed at step {}", i + 1));
                    return;
                }
            }
        }
    }
}
