//! Single source of truth for the slash-command catalog.
//!
//! Both the inline autocomplete ([`crate::completion`]) and the command palette
//! ([`crate::ui::command_bar`]) derive their command lists from here. They used
//! to keep separate hand-maintained copies that had already drifted apart — the
//! palette listed `/lint`, `/format`, `/search` and `/web` that autocomplete
//! never offered, while autocomplete knew `/tree` and the `/stash drop|show|apply`
//! subcommands the palette omitted. Keeping one list makes that class of bug
//! impossible.

/// One slash command as surfaced in autocomplete and the command palette.
pub struct CommandInfo {
    /// Command name without the leading slash, e.g. `"agent on"`.
    pub name: &'static str,
    /// One-line description shown next to the command.
    pub description: &'static str,
    /// Palette grouping, e.g. `"Shell & Git"`.
    pub category: &'static str,
}

/// The full command catalog, grouped by category. Order within a category is
/// the display order; the first entries double as the "popular" list shown when
/// the user has typed just `/`.
pub const COMMANDS: &[CommandInfo] = &[
    // Chat
    c("help", "Show all available commands", "Chat"),
    c("clear", "Clear current conversation", "Chat"),
    c("new", "Start new conversation", "Chat"),
    c("delete", "Delete current conversation", "Chat"),
    c("rename", "Rename current conversation", "Chat"),
    c("export", "Export conversation to markdown", "Chat"),
    c("copy", "Copy last AI response to clipboard", "Chat"),
    c("copy code", "Copy last code block from AI response", "Chat"),
    c("copy all", "Copy entire conversation", "Chat"),
    c("system", "Show or set system prompt", "Chat"),
    // AI Provider
    c("provider", "Switch AI provider", "AI Provider"),
    c("providers", "List available providers", "AI Provider"),
    c("model", "Switch AI model", "AI Provider"),
    c("models", "List available models", "AI Provider"),
    c(
        "ollama",
        "Manage Ollama models (list/pull/remove)",
        "AI Provider",
    ),
    c(
        "mode",
        "Switch mode (efficient/thorough/agent/learning/auto/code/review)",
        "AI Provider",
    ),
    c(
        "agent on",
        "Enable agent mode (AI tool loop)",
        "AI Provider",
    ),
    c("agent off", "Disable agent mode", "AI Provider"),
    c("agent status", "Show agent mode status", "AI Provider"),
    c(
        "agent undo",
        "Roll back to pre-agent git checkpoint",
        "AI Provider",
    ),
    c(
        "agent diff",
        "Show what the agent changed (git diff)",
        "AI Provider",
    ),
    c("agent commit", "Commit agent changes", "AI Provider"),
    c(
        "autocontext",
        "Auto-gather project context (alias: /ac)",
        "AI Provider",
    ),
    c("ac", "Auto-gather project context", "AI Provider"),
    c(
        "code on",
        "Enable code mode (Claude Code only)",
        "AI Provider",
    ),
    c("code off", "Disable code mode", "AI Provider"),
    c("cwd", "Set working directory", "AI Provider"),
    c("cd", "Change working directory", "AI Provider"),
    // Knowledge & Context
    c("file", "Read file as context", "Knowledge"),
    c("files", "Read multiple files as context", "Knowledge"),
    c("summary", "Summarize current conversation", "Knowledge"),
    c("compact", "Compact conversation (save tokens)", "Knowledge"),
    c("context", "Show current AI context window", "Knowledge"),
    c("tokens", "Show token usage breakdown", "Knowledge"),
    c("kb add", "Add directory to knowledge base", "Knowledge"),
    c("kb search", "Search knowledge base", "Knowledge"),
    c("kb list", "List knowledge bases", "Knowledge"),
    c("kb status", "Show KB statistics", "Knowledge"),
    c("kb clear", "Clear knowledge base", "Knowledge"),
    c("url", "Scrape URL for context", "Knowledge"),
    // Shell & Git
    c("run", "Run shell command and show output", "Shell & Git"),
    c(
        "pipe",
        "Run command and add output as context",
        "Shell & Git",
    ),
    c("diff", "Show git diff (adds as context)", "Shell & Git"),
    c("test", "Auto-detect and run project tests", "Shell & Git"),
    c("build", "Auto-detect and run project build", "Shell & Git"),
    c(
        "lint",
        "Auto-detect and run linter (clippy/eslint/ruff)",
        "Shell & Git",
    ),
    c(
        "format",
        "Auto-detect and run formatter (fmt/prettier/ruff)",
        "Shell & Git",
    ),
    c(
        "search",
        "Search codebase with ripgrep (adds results as context)",
        "Shell & Git",
    ),
    c(
        "web",
        "Search the web (DuckDuckGo, adds results as context)",
        "Shell & Git",
    ),
    c(
        "git",
        "Quick git operations (status/log/diff/branch)",
        "Shell & Git",
    ),
    c(
        "commit",
        "Stage all and commit (AI message if omitted)",
        "Shell & Git",
    ),
    c("stage", "Stage files (all if no args)", "Shell & Git"),
    c("unstage", "Unstage files (all if no args)", "Shell & Git"),
    c(
        "gitbranch",
        "Create/switch/delete git branches",
        "Shell & Git",
    ),
    c(
        "gitbranch switch",
        "Switch to existing branch",
        "Shell & Git",
    ),
    c("gitbranch delete", "Delete a git branch", "Shell & Git"),
    c("stash", "Stash changes", "Shell & Git"),
    c("stash pop", "Pop latest stash", "Shell & Git"),
    c("stash list", "List stashes", "Shell & Git"),
    c("stash drop", "Drop a stash entry", "Shell & Git"),
    c("stash show", "Show stash contents", "Shell & Git"),
    c("stash apply", "Apply stash without removing", "Shell & Git"),
    c("log", "Show git log (default 10)", "Shell & Git"),
    c("gitstatus", "Show full git status", "Shell & Git"),
    // Project Scaffolding
    c(
        "template list",
        "List available project templates",
        "Scaffolding",
    ),
    c("template", "Create project from template", "Scaffolding"),
    c(
        "scaffold",
        "AI-generate a project from description",
        "Scaffolding",
    ),
    c(
        "map",
        "Show project map (file tree + symbols)",
        "Scaffolding",
    ),
    c("tree", "Show project file tree (alias)", "Scaffolding"),
    // Automation
    c("auto list", "List automations", "Automation"),
    c("auto run", "Run automation", "Automation"),
    c("auto info", "Show automation details", "Automation"),
    c("auto create", "Create custom automation", "Automation"),
    c("auto delete", "Delete custom automation", "Automation"),
    // Sessions
    c("session", "Show session info", "Sessions"),
    c("session save", "Save current session", "Sessions"),
    c("session list", "List saved sessions", "Sessions"),
    c("session restore", "Restore last session", "Sessions"),
    // Branching
    c("branch save", "Save conversation branch point", "Branching"),
    c("branch list", "List saved branches", "Branching"),
    c("branch restore", "Restore a saved branch", "Branching"),
    c("branch diff", "Compare current with a branch", "Branching"),
    c("branch delete", "Delete a branch", "Branching"),
    // Workspace
    c("workspace", "Show detected project info", "Workspace"),
    // Usage & Cost
    c(
        "usage",
        "Show session usage stats (estimated)",
        "Usage & Cost",
    ),
    c("cost", "Alias for /usage", "Usage & Cost"),
    c("limit", "Show spending limit info", "Usage & Cost"),
    c("limit on", "Enable spending limit", "Usage & Cost"),
    c("limit off", "Disable spending limit", "Usage & Cost"),
    c("limit set", "Set spending limit amount", "Usage & Cost"),
    // System
    c("status", "Show system status", "System"),
    c("theme", "Switch UI theme", "System"),
    // Power User
    c("alias", "List or create aliases", "Power User"),
    c("!!", "Recall last input", "Power User"),
    c("repeat", "Recall last input (same as /!!)", "Power User"),
    // Plugins
    c("plugin list", "List installed plugins", "Plugins"),
    c("plugin init", "Create example plugin", "Plugins"),
    c("plugin reload", "Reload all plugins", "Plugins"),
];

/// Terse const constructor so the table above stays readable.
const fn c(name: &'static str, description: &'static str, category: &'static str) -> CommandInfo {
    CommandInfo {
        name,
        description,
        category,
    }
}

/// The full command catalog.
pub fn all() -> &'static [CommandInfo] {
    COMMANDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn no_duplicate_command_names() {
        let mut seen = HashSet::new();
        for cmd in COMMANDS {
            assert!(
                seen.insert(cmd.name),
                "duplicate command in catalog: /{}",
                cmd.name
            );
        }
    }

    #[test]
    fn every_command_has_a_description_and_category() {
        for cmd in COMMANDS {
            assert!(!cmd.name.is_empty());
            assert!(
                !cmd.description.is_empty(),
                "/{} has no description",
                cmd.name
            );
            assert!(!cmd.category.is_empty(), "/{} has no category", cmd.name);
        }
    }

    #[test]
    fn contains_the_previously_drifted_commands() {
        // These four lived only in the palette and these four only in
        // autocomplete before unification — both surfaces must now have all.
        for name in [
            "lint",
            "format",
            "search",
            "web",
            "tree",
            "stash drop",
            "stash show",
            "stash apply",
        ] {
            assert!(
                COMMANDS.iter().any(|c| c.name == name),
                "catalog missing /{name}"
            );
        }
    }

    #[test]
    fn catalog_is_reasonably_complete() {
        assert!(
            COMMANDS.len() >= 90,
            "expected >= 90 commands, got {}",
            COMMANDS.len()
        );
    }
}
