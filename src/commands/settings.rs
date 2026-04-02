//! Settings commands: /theme, /alias, /limit, /usage, /plugin

use crate::app::App;
use crate::config;
use crate::plugins;

/// Handle settings-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str) -> bool {
    if trimmed == "/theme" || trimmed.starts_with("/theme ") {
        return handle_theme(app, trimmed);
    }

    if trimmed == "/alias" || trimmed.starts_with("/alias ") {
        return handle_alias(app, trimmed);
    }

    if trimmed == "/usage" || trimmed == "/cost" {
        return handle_usage(app);
    }

    if trimmed == "/limit" || trimmed.starts_with("/limit ") {
        return handle_limit(app, trimmed);
    }

    if trimmed == "/plugin"
        || trimmed == "/plugins"
        || trimmed.starts_with("/plugin ")
        || trimmed.starts_with("/plugins ")
    {
        return handle_plugin(app, trimmed);
    }

    false
}

fn handle_theme(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/theme").unwrap_or("").trim();
    let presets = config::theme_presets();
    if rest.is_empty() || rest == "list" {
        let mut msg = "Available themes:\n\n".to_string();
        for (i, (name, _)) in presets.iter().enumerate() {
            let marker = if i == app.theme_index {
                "\u{25ba} "
            } else {
                "  "
            };
            msg.push_str(&format!("{marker}{name}\n"));
        }
        msg.push_str("\nUsage: /theme <name> or /theme <number>");
        app.add_assistant_message(msg);
    } else {
        let query = rest.to_lowercase();
        if let Some(idx) = query.parse::<usize>().ok().and_then(|n| {
            if n > 0 && n <= presets.len() {
                Some(n - 1)
            } else {
                None
            }
        }) {
            app.theme_index = idx;
            app.set_status(format!("Theme: {}", presets[idx].0));
        } else if let Some(idx) = presets
            .iter()
            .position(|(name, _)| name.to_lowercase().contains(&query))
        {
            app.theme_index = idx;
            app.set_status(format!("Theme: {}", presets[idx].0));
        } else {
            app.set_status("Theme not found. Use /theme list");
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_alias(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/alias").unwrap_or("").trim();
    if rest.is_empty() {
        if app.aliases.is_empty() {
            app.add_assistant_message(
                "No aliases set.\n\nUsage: /alias <name> <command>\nExample: /alias r /run cargo test".into(),
            );
        } else {
            let mut msg = "Aliases:\n\n".to_string();
            let mut sorted: Vec<_> = app.aliases.iter().collect();
            sorted.sort_by_key(|(k, _)| (*k).clone());
            for (name, cmd) in &sorted {
                msg.push_str(&format!("  /{name} \u{2192} {cmd}\n"));
            }
            app.add_assistant_message(msg);
        }
    } else {
        let parts: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let command = parts[1].trim().to_string();
            app.set_status(format!("Alias set: /{name} \u{2192} {command}"));
            app.aliases.insert(name, command);
        } else {
            app.set_status("Usage: /alias <name> <command>");
        }
    }
    app.scroll_offset = 0;
    true
}

fn handle_usage(app: &mut App) -> bool {
    let stats = &app.usage_stats;
    let msg = format!(
        "Session Usage (estimated)\n\
         {}\n\
         \n\
         Requests: {}\n\
         Tokens sent: ~{}\n\
         Tokens received: ~{}\n\
         Estimated cost: {}\n\
         \n\
         Provider: {}\n\
         Model: {}",
        "=".repeat(25),
        stats.total_requests,
        stats.total_tokens_sent,
        stats.total_tokens_received,
        stats.format_cost(),
        app.selected_provider,
        app.selected_model,
    );
    app.add_assistant_message(msg);
    app.scroll_offset = 0;
    true
}

fn handle_limit(app: &mut App, trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix("/limit").unwrap_or("").trim();
    let args: Vec<&str> = rest.split_whitespace().collect();
    let subcmd = args.first().copied().unwrap_or("info");
    match subcmd {
        "on" => {
            app.spending_limit.enabled = true;
            app.set_status(format!(
                "Spending limit ON: ${:.2}/session",
                app.spending_limit.max_cost_usd
            ));
        }
        "off" => {
            app.spending_limit.enabled = false;
            app.set_status("Spending limit OFF".to_string());
        }
        "set" => {
            if let Some(amount) = args.get(1).and_then(|s| s.parse::<f64>().ok()) {
                app.spending_limit.max_cost_usd = amount;
                app.spending_limit.enabled = true;
                app.set_status(format!("Spending limit set to ${:.2}/session", amount));
            } else {
                app.set_status("Usage: /limit set <amount_usd>".to_string());
            }
        }
        _ => {
            let limit = &app.spending_limit;
            let status = if limit.enabled { "ON" } else { "OFF" };
            app.add_assistant_message(format!(
                "Spending Limits\n\
                 {}\n\
                 \n\
                 Status: {}\n\
                 Max cost: ${:.2}/session\n\
                 Current cost: {}\n\
                 \n\
                 Commands:\n\
                 \x20 /limit on       Enable limit\n\
                 \x20 /limit off      Disable limit\n\
                 \x20 /limit set <$>  Set limit amount",
                "=".repeat(25),
                status,
                limit.max_cost_usd,
                app.usage_stats.format_cost()
            ));
            app.scroll_offset = 0;
        }
    }
    true
}

fn handle_plugin(app: &mut App, trimmed: &str) -> bool {
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
                        manifest.command,
                        manifest.name,
                        manifest.version,
                        status,
                        manifest.description
                    ));
                }
                app.add_assistant_message(msg);
            }
        }
        "init" => match plugins::create_example_plugin() {
            Ok(path) => {
                app.add_assistant_message(format!(
                    "Created example plugin at:\n  {}\n\nEdit plugin.toml and run.sh to customize.\nRestart Nerve to load new plugins.",
                    path.display()
                ));
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        },
        "reload" => {
            app.plugins = plugins::load_plugins();
            app.set_status(format!("{} plugin(s) loaded", app.plugins.len()));
        }
        _ => {
            app.add_assistant_message(
                "Usage: /plugin list | /plugin init | /plugin reload".into(),
            );
        }
    }
    app.scroll_offset = 0;
    true
}
