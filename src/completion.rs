use crate::app::App;

/// Find the longest common prefix among a set of string slices.
///
/// Works in `char` units throughout (not bytes), so a common prefix that ends
/// inside a multi-byte codepoint can never produce a panicking byte slice.
pub(crate) fn common_prefix(strings: &[&str]) -> String {
    let Some((first, rest)) = strings.split_first() else {
        return String::new();
    };
    let mut prefix_chars = first.chars().count();
    for s in rest {
        let common = first
            .chars()
            .zip(s.chars())
            .take_while(|(a, b)| a == b)
            .count();
        prefix_chars = prefix_chars.min(common);
    }
    first.chars().take(prefix_chars).collect()
}

// ─── Inline autocomplete ────────────────────────────────────────────────────

/// All slash commands with descriptions, used for autocomplete.
/// Each entry is `(command_name, description)`.
///
/// Descriptions must match those in `ui::command_bar::command_prompts()`.
pub(crate) fn get_all_commands() -> &'static [(&'static str, &'static str)] {
    &[
        // Chat
        ("help", "Show all available commands"),
        ("clear", "Clear current conversation"),
        ("new", "Start new conversation"),
        ("delete", "Delete current conversation"),
        ("rename", "Rename current conversation"),
        ("export", "Export conversation to markdown"),
        ("copy", "Copy last AI response to clipboard"),
        ("copy code", "Copy last code block from AI response"),
        ("copy all", "Copy entire conversation"),
        ("system", "Show or set system prompt"),
        // AI Provider
        ("provider", "Switch AI provider"),
        ("providers", "List available providers"),
        ("model", "Switch AI model"),
        ("models", "List available models"),
        ("ollama", "Manage Ollama models (list/pull/remove)"),
        (
            "mode",
            "Switch mode (efficient/thorough/agent/learning/auto/code/review)",
        ),
        ("agent on", "Enable agent mode (AI tool loop)"),
        ("agent off", "Disable agent mode"),
        ("agent status", "Show agent mode status"),
        ("agent undo", "Roll back to pre-agent git checkpoint"),
        ("agent diff", "Show what the agent changed (git diff)"),
        ("agent commit", "Commit agent changes"),
        ("autocontext", "Auto-gather project context (alias: /ac)"),
        ("ac", "Auto-gather project context"),
        ("code on", "Enable code mode (Claude Code only)"),
        ("code off", "Disable code mode"),
        ("cwd", "Set working directory"),
        ("cd", "Change working directory"),
        // Knowledge & Context
        ("file", "Read file as context"),
        ("files", "Read multiple files as context"),
        ("summary", "Summarize current conversation"),
        ("compact", "Compact conversation (save tokens)"),
        ("context", "Show current AI context window"),
        ("tokens", "Show token usage breakdown"),
        ("kb add", "Add directory to knowledge base"),
        ("kb search", "Search knowledge base"),
        ("kb list", "List knowledge bases"),
        ("kb status", "Show KB statistics"),
        ("kb clear", "Clear knowledge base"),
        ("url", "Scrape URL for context"),
        // Shell & Git
        ("run", "Run shell command and show output"),
        ("pipe", "Run command and add output as context"),
        ("diff", "Show git diff (adds as context)"),
        ("test", "Auto-detect and run project tests"),
        ("build", "Auto-detect and run project build"),
        ("git", "Quick git operations (status/log/diff/branch)"),
        ("commit", "Stage all and commit (AI message if omitted)"),
        ("stage", "Stage files (all if no args)"),
        ("unstage", "Unstage files"),
        ("gitbranch", "Create/switch/delete git branches"),
        ("gitbranch switch", "Switch to existing branch"),
        ("gitbranch delete", "Delete a git branch"),
        ("stash", "Stash changes"),
        ("stash pop", "Pop latest stash"),
        ("stash list", "List stashes"),
        ("stash drop", "Drop a stash entry"),
        ("stash show", "Show stash contents"),
        ("stash apply", "Apply stash without removing"),
        ("log", "Show git log (default 10)"),
        ("gitstatus", "Show full git status"),
        // Project Scaffolding
        ("template list", "List available project templates"),
        ("template", "Create project from template"),
        ("scaffold", "AI-generate a project from description"),
        ("map", "Show project map (file tree + symbols)"),
        ("tree", "Show project file tree (alias)"),
        // Automation
        ("auto list", "List automations"),
        ("auto run", "Run automation"),
        ("auto info", "Show automation details"),
        ("auto create", "Create custom automation"),
        ("auto delete", "Delete custom automation"),
        // Sessions
        ("session", "Show session info"),
        ("session save", "Save current session"),
        ("session list", "List saved sessions"),
        ("session restore", "Restore last session"),
        // Branching
        ("branch save", "Save conversation branch point"),
        ("branch list", "List saved branches"),
        ("branch restore", "Restore a saved branch"),
        ("branch diff", "Compare current with a branch"),
        ("branch delete", "Delete a branch"),
        // Workspace
        ("workspace", "Show detected project info"),
        // Usage & Cost
        ("usage", "Show session usage stats (estimated)"),
        ("cost", "Alias for /usage"),
        ("limit", "Show spending limit info"),
        ("limit on", "Enable spending limit"),
        ("limit off", "Disable spending limit"),
        ("limit set", "Set spending limit amount"),
        // System
        ("status", "Show system status"),
        ("theme", "Switch UI theme"),
        // Power User
        ("alias", "List or create aliases"),
        ("!!", "Recall last input"),
        ("repeat", "Recall last input (same as /!!)"),
        // Plugins
        ("plugin list", "List installed plugins"),
        ("plugin init", "Create example plugin"),
        ("plugin reload", "Reload all plugins"),
    ]
}

/// Update the inline autocomplete popup based on current input.
///
/// Shows matching slash commands with descriptions when input starts with `/`,
/// or matching file paths when the input contains an `@` mention.
pub(crate) fn update_autocomplete(app: &mut App) {
    let input = &app.input;

    if let Some(partial) = input.strip_prefix('/') {
        // Autocomplete slash commands — supports subcommands (e.g. "/agent on").
        let commands = get_all_commands();
        let max_items = 10;

        let mut scored: Vec<(bool, &str, &str)> = if partial.is_empty() {
            // Show popular commands when the user just typed '/'.
            commands
                .iter()
                .take(max_items)
                .map(|(cmd, desc)| (true, *cmd, *desc))
                .collect()
        } else {
            commands
                .iter()
                .filter(|(cmd, desc)| {
                    cmd.starts_with(partial)
                        || cmd.contains(partial)
                        || desc.to_lowercase().contains(&partial.to_lowercase())
                })
                .take(max_items)
                .map(|(cmd, desc)| (cmd.starts_with(partial), *cmd, *desc))
                .collect()
        };
        // Prefix matches first.
        scored.sort_by_key(|x| std::cmp::Reverse(x.0));

        app.autocomplete_items = scored
            .iter()
            .map(|(_, cmd, desc)| format!("/{cmd}  \u{2500}\u{2500} {desc}"))
            .collect();
        app.autocomplete_visible = !app.autocomplete_items.is_empty();
        app.autocomplete_index = 0;
    } else if let Some(at_pos) = input.rfind('@') {
        // Autocomplete file paths after `@`.
        let partial = &input[at_pos + 1..];
        if !partial.contains(' ') {
            app.autocomplete_items = autocomplete_file_paths(partial);
            app.autocomplete_visible = !app.autocomplete_items.is_empty();
            app.autocomplete_index = 0;
        } else {
            app.autocomplete_visible = false;
            app.autocomplete_items.clear();
            app.autocomplete_index = 0;
        }
    } else {
        app.autocomplete_visible = false;
        app.autocomplete_items.clear();
        app.autocomplete_index = 0;
    }
}

/// Return up to 10 file path matches for the given partial path, suitable for
/// displaying in the autocomplete popup.
///
/// When `partial` is empty, lists files in the current directory. Directories
/// are sorted before files and shown with a trailing `/`. Each entry includes
/// a description suffix (e.g. "directory" or a human-readable file size).
pub(crate) fn autocomplete_file_paths(partial: &str) -> Vec<String> {
    use std::path::Path;

    let path = if partial.is_empty() {
        match std::env::current_dir() {
            Ok(cwd) => cwd,
            Err(_) => return Vec::new(),
        }
    } else if let Some(stripped) = partial.strip_prefix("~/") {
        match dirs::home_dir() {
            Some(h) => h.join(stripped),
            None => return Vec::new(),
        }
    } else if partial.starts_with('/') {
        std::path::PathBuf::from(partial)
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(partial),
            Err(_) => return Vec::new(),
        }
    };

    let (dir, prefix) = if path.is_dir() {
        (path, String::new())
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_prefix = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent.to_path_buf(), file_prefix)
    };

    if !dir.exists() {
        return Vec::new();
    }

    // Collect (display_path, is_dir, size_bytes) tuples.
    let mut entries_vec: Vec<(String, bool, u64)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue; // skip hidden files
            }
            if !prefix.is_empty() && !name.starts_with(&prefix) {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

            let completed = if partial.contains('/') {
                let dir_part = &partial[..=partial.rfind('/').unwrap_or(0)];
                if is_dir {
                    format!("{dir_part}{name}/")
                } else {
                    format!("{dir_part}{name}")
                }
            } else if is_dir {
                format!("{name}/")
            } else {
                name
            };

            entries_vec.push((completed, is_dir, size));
        }
    }

    // Sort: directories first, then alphabetically within each group.
    entries_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Build display strings with description suffixes.
    entries_vec
        .into_iter()
        .take(10)
        .map(|(path, is_dir, size)| {
            if is_dir {
                format!("{path}  \u{2500}\u{2500} directory")
            } else {
                format!("{}  \u{2500}\u{2500} {}", path, format_file_size(size))
            }
        })
        .collect()
}

/// Format a byte count into a human-readable size string.
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Strip the description suffix (e.g. `  -- directory` or `  -- 1.2 KB`) from
/// an autocomplete display item, returning just the path portion.
pub(crate) fn strip_autocomplete_description(item: &str) -> &str {
    if let Some(pos) = item.find("  \u{2500}\u{2500} ") {
        &item[..pos]
    } else {
        item
    }
}

/// Accept the currently selected autocomplete item and insert it into the
/// input buffer.
pub(crate) fn accept_autocomplete(app: &mut App) {
    if let Some(selected) = app.autocomplete_items.get(app.autocomplete_index).cloned() {
        // Strip description suffix ("  ── ...") if present.
        // Format is always "/command  ── description", so first match is correct.
        let clean = if let Some(sep) = selected.find("  \u{2500}\u{2500} ") {
            selected[..sep].to_string()
        } else {
            selected
        };

        if app.input.starts_with('/') {
            // Replace the entire slash command prefix.
            app.input = format!("{clean} ");
            app.cursor_position = app.input.len();
        } else if let Some(at_pos) = app.input.rfind('@') {
            // Strip the description suffix to get the actual path.
            let path = strip_autocomplete_description(&clean).to_string();
            let is_directory = path.ends_with('/');

            // Replace the text after `@` with the selected path.
            let before = app.input[..=at_pos].to_string();
            let after_cursor = if app.cursor_position < app.input.len() {
                // Preserve any text after the current partial.
                let partial_end = app.input[at_pos + 1..]
                    .find(' ')
                    .map(|i| at_pos + 1 + i)
                    .unwrap_or(app.input.len());
                app.input[partial_end..].to_string()
            } else {
                String::new()
            };

            if is_directory {
                // Don't add a space — let the user browse into the directory.
                app.input = format!("{before}{path}{after_cursor}");
                app.cursor_position = before.len() + path.len();
                // Re-trigger autocomplete so the directory contents are shown.
                update_autocomplete(app);
                return;
            } else {
                // File selected — add a trailing space.
                app.input = format!("{before}{path} {after_cursor}");
                app.cursor_position = before.len() + path.len() + 1;
            }
        }
        app.autocomplete_visible = false;
    }
}

// ─── File path completion ──────────────────────────────────────────────────

/// Attempt to complete a partial file path. Returns `Some(completed)` if there
/// is exactly one match or a longer common prefix; `None` otherwise.
pub(crate) fn complete_file_path(partial: &str) -> Option<String> {
    use std::path::Path;

    let path = if let Some(stripped) = partial.strip_prefix("~/") {
        dirs::home_dir()?.join(stripped)
    } else if partial.starts_with('/') {
        std::path::PathBuf::from(partial)
    } else {
        std::env::current_dir().ok()?.join(partial)
    };

    // If the partial path points to an existing file, return it as-is
    if path.exists() && path.is_file() {
        return Some(partial.to_string());
    }

    // Get the parent directory and the prefix to match
    let (dir, prefix) = if path.is_dir() {
        (path, String::new())
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_prefix = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent.to_path_buf(), file_prefix)
    };

    if !dir.exists() {
        return None;
    }

    let mut matches: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            } // skip hidden
            if !prefix.is_empty() && !name.starts_with(&prefix) {
                continue;
            }

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            // Build the completed path relative to what the user typed
            let completed = if partial.contains('/') {
                let dir_part = &partial[..=partial.rfind('/').unwrap_or(0)];
                if is_dir {
                    format!("{dir_part}{name}/")
                } else {
                    format!("{dir_part}{name}")
                }
            } else if is_dir {
                format!("{name}/")
            } else {
                name.clone()
            };

            matches.push(completed);
        }
    }

    matches.sort();

    if matches.len() == 1 {
        Some(matches.into_iter().next().expect("len checked == 1"))
    } else if matches.len() > 1 {
        // Return common prefix if it extends beyond what was typed
        let common = find_common_prefix_strings(&matches);
        if common.len() > partial.len() {
            Some(common)
        } else {
            None // No further completion possible, but matches exist
        }
    } else {
        None
    }
}

/// List all file matches for a partial path (used for showing options in status).
pub(crate) fn list_file_matches(partial: &str) -> Vec<String> {
    use std::path::Path;

    let path = if let Some(stripped) = partial.strip_prefix("~/") {
        match dirs::home_dir() {
            Some(h) => h.join(stripped),
            None => return Vec::new(),
        }
    } else if partial.starts_with('/') {
        std::path::PathBuf::from(partial)
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(partial),
            Err(_) => return Vec::new(),
        }
    };

    let (dir, prefix) = if path.is_dir() {
        (path, String::new())
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        let file_prefix = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent.to_path_buf(), file_prefix)
    };

    if !dir.exists() {
        return Vec::new();
    }

    let mut matches: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if !prefix.is_empty() && !name.starts_with(&prefix) {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                matches.push(format!("{name}/"));
            } else {
                matches.push(name);
            }
        }
    }

    matches.sort();
    matches
}

/// Find the longest common prefix among a vec of owned strings.
pub(crate) fn find_common_prefix_strings(strings: &[String]) -> String {
    let Some((first, rest)) = strings.split_first() else {
        return String::new();
    };
    // Count in `char` units so a prefix ending mid-codepoint (e.g. two
    // filenames sharing a leading multi-byte char like "日記"/"日付") can't
    // produce a panicking byte slice.
    let mut prefix_chars = first.chars().count();
    for s in rest {
        let common = first
            .chars()
            .zip(s.chars())
            .take_while(|(a, b)| a == b)
            .count();
        prefix_chars = prefix_chars.min(common);
    }
    first.chars().take(prefix_chars).collect()
}
