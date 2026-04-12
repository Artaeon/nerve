//! Settings commands: /theme, /alias, /limit, /usage, /plugin

use crate::app::App;
use crate::config;
use crate::plugins;

/// Handle settings-related commands. Returns `true` if the command was handled.
pub async fn handle(app: &mut App, trimmed: &str) -> bool {
    if crate::shell::matches_command(trimmed, "/theme") {
        return handle_theme(app, trimmed);
    }

    if crate::shell::matches_command(trimmed, "/alias") {
        return handle_alias(app, trimmed);
    }

    if trimmed == "/usage" || trimmed == "/cost" {
        return handle_usage(app);
    }

    if crate::shell::matches_command(trimmed, "/limit") {
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
                if amount > 0.0 && amount.is_finite() {
                    app.spending_limit.max_cost_usd = amount;
                    app.spending_limit.enabled = true;
                    app.set_status(format!("Spending limit set to ${amount:.2}/session"));
                } else {
                    app.set_status("Spending limit must be a positive number".to_string());
                }
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
            Err(e) => app.report_error(e),
        },
        "reload" => {
            app.plugins = plugins::load_plugins();
            app.set_status(format!("{} plugin(s) loaded", app.plugins.len()));
        }
        _ => {
            app.add_assistant_message("Usage: /plugin list | /plugin init | /plugin reload".into());
        }
    }
    app.scroll_offset = 0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── /theme ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn theme_bare_lists_themes() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Available themes"));
    }

    #[tokio::test]
    async fn theme_list_lists_themes() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme list").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Available themes"));
    }

    #[tokio::test]
    async fn theme_by_number_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme 1").await);
        assert_eq!(app.theme_index, 0); // 1-indexed -> 0-indexed
    }

    #[tokio::test]
    async fn theme_not_found() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme nonexistent_theme_xyz").await);
        let status = app.status_message.as_deref().unwrap();
        assert!(status.contains("not found"));
    }

    // ── /alias ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn alias_bare_shows_list() {
        let mut app = App::new();
        assert!(handle(&mut app, "/alias").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("No aliases set"));
    }

    #[tokio::test]
    async fn alias_creates_mapping() {
        let mut app = App::new();
        assert!(handle(&mut app, "/alias t /run cargo test").await);
        assert_eq!(app.aliases.get("t").unwrap(), "/run cargo test");
    }

    #[tokio::test]
    async fn alias_name_equals_value_parse() {
        let mut app = App::new();
        // The alias parser uses splitn(2, whitespace), so "name=value" is the name
        assert!(handle(&mut app, "/alias r /run cargo run").await);
        assert_eq!(app.aliases.get("r").unwrap(), "/run cargo run");
    }

    #[tokio::test]
    async fn alias_without_command_shows_usage() {
        let mut app = App::new();
        assert!(handle(&mut app, "/alias justname").await);
        let status = app.status_message.as_deref().unwrap();
        assert!(status.contains("Usage"));
    }

    #[tokio::test]
    async fn alias_list_after_creation() {
        let mut app = App::new();
        app.aliases.insert("t".into(), "/run cargo test".into());
        assert!(handle(&mut app, "/alias").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Aliases"));
        assert!(last.1.contains("/t"));
    }

    // ── /usage and /cost ────────────────────────────────────────────────

    #[tokio::test]
    async fn usage_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/usage").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Session Usage"));
    }

    #[tokio::test]
    async fn cost_alias_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/cost").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Session Usage"));
    }

    // ── /limit ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn limit_bare_shows_info() {
        let mut app = App::new();
        assert!(handle(&mut app, "/limit").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Spending Limits"));
    }

    #[tokio::test]
    async fn limit_on_enables() {
        let mut app = App::new();
        assert!(handle(&mut app, "/limit on").await);
        assert!(app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_off_disables() {
        let mut app = App::new();
        app.spending_limit.enabled = true;
        assert!(handle(&mut app, "/limit off").await);
        assert!(!app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_set_extracts_amount() {
        let mut app = App::new();
        assert!(handle(&mut app, "/limit set 10").await);
        assert!((app.spending_limit.max_cost_usd - 10.0).abs() < f64::EPSILON);
        assert!(app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_set_invalid_shows_usage() {
        let mut app = App::new();
        assert!(handle(&mut app, "/limit set abc").await);
        let status = app.status_message.as_deref().unwrap();
        assert!(status.contains("Usage"));
    }

    // ── /plugin ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn plugin_list_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugin list").await);
    }

    #[tokio::test]
    async fn plugin_bare_defaults_to_list() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugin").await);
    }

    #[tokio::test]
    async fn plugin_init_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugin init").await);
    }

    #[tokio::test]
    async fn plugin_reload_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugin reload").await);
    }

    #[tokio::test]
    async fn plugins_plural_is_handled() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugins").await);
    }

    #[tokio::test]
    async fn plugin_unknown_subcommand() {
        let mut app = App::new();
        assert!(handle(&mut app, "/plugin foobar").await);
        let last = app.current_conversation().messages.last().unwrap();
        assert!(last.1.contains("Usage"));
    }

    // ── Unrecognised commands ───────────────────────────────────────────

    #[tokio::test]
    async fn unrecognised_returns_false() {
        let mut app = App::new();
        assert!(!handle(&mut app, "/themes").await);
        assert!(!handle(&mut app, "/aliasing").await);
        assert!(!handle(&mut app, "/limiter").await);
    }

    // ── Spending limit edge cases ───────────────────────────────────

    #[tokio::test]
    async fn limit_set_valid_amount() {
        let mut app = App::new();
        handle(&mut app, "/limit set 5.00").await;
        assert!(app.spending_limit.enabled);
        assert!((app.spending_limit.max_cost_usd - 5.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn limit_set_negative_rejected() {
        let mut app = App::new();
        app.spending_limit.enabled = false;
        handle(&mut app, "/limit set -10").await;
        // Should NOT enable the limit
        assert!(!app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_set_zero_rejected() {
        let mut app = App::new();
        app.spending_limit.enabled = false;
        handle(&mut app, "/limit set 0").await;
        assert!(!app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_set_non_numeric_shows_usage() {
        let mut app = App::new();
        handle(&mut app, "/limit set abc").await;
        let status = app.status_message.clone().unwrap_or_default();
        assert!(status.contains("Usage"));
    }

    #[tokio::test]
    async fn limit_on_enables_from_disabled() {
        let mut app = App::new();
        app.spending_limit.enabled = false;
        handle(&mut app, "/limit on").await;
        assert!(app.spending_limit.enabled);
    }

    #[tokio::test]
    async fn limit_off_disables_from_enabled() {
        let mut app = App::new();
        app.spending_limit.enabled = true;
        handle(&mut app, "/limit off").await;
        assert!(!app.spending_limit.enabled);
    }

    // ── Theme edge cases ────────────────────────────────────────────

    #[tokio::test]
    async fn theme_list_is_recognized() {
        let mut app = App::new();
        assert!(handle(&mut app, "/theme list").await);
    }

    #[tokio::test]
    async fn theme_out_of_bounds_number() {
        let mut app = App::new();
        let original_index = app.theme_index;
        handle(&mut app, "/theme 999").await;
        // Should not change the theme index for out-of-bounds
        assert_eq!(app.theme_index, original_index);
    }

    #[tokio::test]
    async fn theme_zero_not_valid() {
        let mut app = App::new();
        let original_index = app.theme_index;
        handle(&mut app, "/theme 0").await;
        // 0 is out of bounds (1-indexed)
        assert_eq!(app.theme_index, original_index);
    }

    // ── Alias edge cases ────────────────────────────────────────────

    #[tokio::test]
    async fn alias_create_and_lookup() {
        let mut app = App::new();
        handle(&mut app, "/alias mytest /run echo hello").await;
        assert!(app.aliases.contains_key("mytest"));
        assert_eq!(app.aliases["mytest"], "/run echo hello");
    }

    #[tokio::test]
    async fn alias_no_args_shows_list() {
        let mut app = App::new();
        assert!(handle(&mut app, "/alias").await);
    }
}
