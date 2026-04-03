//! File commands: /file, /files, /template, /scaffold, /workspace, /map, /tree

use std::sync::Arc;

use crate::ai::provider::AiProvider;
use crate::app::App;
use crate::files;
use crate::scaffold;
use crate::workspace;

/// Handle file-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) -> bool {
    if trimmed == "/file" || trimmed.starts_with("/file ") {
        return handle_file(app, trimmed);
    }

    if trimmed == "/files" || trimmed.starts_with("/files ") {
        return handle_files(app, trimmed);
    }

    if trimmed == "/template" || trimmed.starts_with("/template ") {
        return handle_template(app, trimmed);
    }

    if trimmed == "/scaffold" || trimmed.starts_with("/scaffold ") {
        return handle_scaffold(app, trimmed, provider).await;
    }

    if trimmed == "/workspace" || trimmed == "/ws" {
        return handle_workspace(app);
    }

    if trimmed == "/map"
        || trimmed == "/tree"
        || trimmed.starts_with("/map ")
        || trimmed.starts_with("/tree ")
    {
        return handle_map(app, trimmed);
    }

    false
}

fn handle_file(app: &mut App, trimmed: &str) -> bool {
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
            app.set_status(format!("Added {} ({} lines)", fc.path, fc.line_count));
        }
        Err(e) => {
            app.report_error(e);
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_files(app: &mut App, trimmed: &str) -> bool {
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
                app.set_status(format!("Error reading {path}: {e}"));
            }
        }
    }
    if added > 0 {
        app.set_status(format!("Added {added} file(s) as context"));
    }
    app.scroll_offset = 0;
    true
}

fn handle_template(app: &mut App, trimmed: &str) -> bool {
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
    let project_name = args.get(1).copied().unwrap_or(template_name).to_string();

    match scaffold::get_template(template_name) {
        Some(mut template) => {
            for file in &mut template.files {
                file.content = file.content.replace("{{name}}", &project_name);
                file.content = file
                    .content
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
                    app.report_error(e);
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
    true
}

async fn handle_scaffold(app: &mut App, trimmed: &str, provider: &Arc<dyn AiProvider>) -> bool {
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

    app.add_user_message(prompt);
    app.scroll_offset = 0;
    crate::send_to_ai_from_history(app, provider).await;
    true
}

fn handle_workspace(app: &mut App) -> bool {
    let ws_info = app
        .cached_workspace
        .clone()
        .or_else(workspace::detect_workspace);
    match ws_info {
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
    true
}

fn handle_map(app: &mut App, trimmed: &str) -> bool {
    let args_str = trimmed
        .strip_prefix("/map")
        .or_else(|| trimmed.strip_prefix("/tree"))
        .unwrap_or("")
        .trim();
    let depth = args_str.parse::<usize>().unwrap_or(3);
    let root = std::env::current_dir().unwrap_or_default();
    let map = crate::workspace::generate_project_map(&root, depth);
    app.add_assistant_message(map);
    app.scroll_offset = 0;
    true
}
