use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::ai::retry::RetryConfig;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Top-level application configuration for Nerve.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default model identifier (e.g. "llama3.1", "gpt-4o").
    pub default_model: String,
    /// Default provider key: "openai", "ollama", "openrouter", or the name
    /// of a custom provider.
    pub default_provider: String,
    /// Timeout in seconds for shell commands executed by `/run` and agent
    /// tools. Set to `0` to disable the timeout. Defaults to 30.
    #[serde(default = "default_command_timeout_secs")]
    pub command_timeout_secs: u64,
    /// Theme colours.
    pub theme: ThemeConfig,
    /// Provider connection settings.
    pub providers: ProvidersConfig,
    /// Keyboard shortcuts.
    pub keybinds: KeybindsConfig,
    /// API retry behaviour (exponential backoff).
    pub retry: RetryConfig,
    /// When `true`, automatically enable agent mode for messages that appear
    /// to need tool access (file I/O, shell commands, git, etc.).
    #[serde(default = "Config::default_auto_agent")]
    pub auto_agent: bool,
    /// When `true`, `/workflow` skips the plan-approval gate and executes
    /// the plan immediately (the pre-gate behaviour). Default `false`:
    /// nothing runs until the user types `/approve`.
    #[serde(default)]
    pub workflow_auto_approve: bool,
    /// When `true` (default), Nerve routes each turn to a model tier relative
    /// to `default_model`: the strong model for planning/review/hard problems,
    /// the small model for trivial edits. Set `false` to always use the
    /// selected model. Only Claude and OpenAI providers are routed.
    #[serde(default = "Config::default_auto_model_routing")]
    pub auto_model_routing: bool,
    /// When `true` (default), after an agent turn that edited files Nerve runs
    /// a verify command (type-check / build) and feeds any failure back so the
    /// agent fixes it. Set `false` to disable.
    #[serde(default = "Config::default_auto_verify")]
    pub auto_verify: bool,
    /// Explicit verify command (e.g. `"npm run lint"`). When unset, Nerve
    /// auto-detects one from the workspace (Cargo/npm). Only used when
    /// `auto_verify` is on.
    #[serde(default)]
    pub verify_command: Option<String>,
    /// Git commit author name (e.g. "Jane Doe").
    /// When set, passed as `--author` to `git commit`.
    #[serde(default)]
    pub git_user_name: Option<String>,
    /// Git commit author email (e.g. "jane@example.com").
    /// When set, passed as `--author` to `git commit`.
    #[serde(default)]
    pub git_user_email: Option<String>,
    /// Sampling temperature for AI responses (0.0 = deterministic, 2.0 = very
    /// creative). Leave unset to use the provider's default.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Top-p (nucleus sampling). 0.1 keeps only the top 10% probability mass.
    /// Leave unset to use the provider's default.
    #[serde(default)]
    pub top_p: Option<f32>,
    /// Override the maximum context window size (in tokens) for the active
    /// provider. Useful for local models with non-standard context lengths.
    #[serde(default)]
    pub context_limit: Option<usize>,
}

fn default_command_timeout_secs() -> u64 {
    crate::shell::DEFAULT_COMMAND_TIMEOUT_SECS
}

/// Connections for each supported AI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub claude_code: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
    pub openrouter: Option<ProviderConfig>,
    /// Zero or more additional OpenAI-compatible endpoints.
    #[serde(default)]
    pub custom: Vec<CustomProviderConfig>,
}

/// Configuration for a built-in provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: bool,
}

/// Configuration for an arbitrary OpenAI-compatible provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub name: String,
    pub api_key: String,
    pub base_url: String,
}

/// TUI colour theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub user_color: String,
    pub assistant_color: String,
    pub border_color: String,
    pub accent_color: String,
    #[serde(default = "ThemeConfig::default_success_color")]
    pub success_color: String,
    #[serde(default = "ThemeConfig::default_error_color")]
    pub error_color: String,
    #[serde(default = "ThemeConfig::default_warning_color")]
    pub warning_color: String,
    #[serde(default = "ThemeConfig::default_dim_color")]
    pub dim_color: String,
}

/// Configurable keyboard shortcuts (stored as human-readable strings such
/// as `"ctrl+k"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindsConfig {
    pub command_bar: String,
    pub new_conversation: String,
    pub prompt_picker: String,
    pub model_select: String,
    pub help: String,
    pub copy_last: String,
    pub quit: String,
}

// ---------------------------------------------------------------------------
// Default implementations
// ---------------------------------------------------------------------------

impl Default for Config {
    fn default() -> Self {
        Self {
            default_model: "sonnet".into(),
            default_provider: "claude_code".into(),
            command_timeout_secs: default_command_timeout_secs(),
            theme: ThemeConfig::default(),
            providers: ProvidersConfig::default(),
            keybinds: KeybindsConfig::default(),
            retry: RetryConfig::default(),
            auto_agent: true,
            workflow_auto_approve: false,
            auto_model_routing: true,
            auto_verify: true,
            verify_command: None,
            git_user_name: None,
            git_user_email: None,
            temperature: None,
            top_p: None,
            context_limit: None,
        }
    }
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            claude_code: Some(ProviderConfig {
                api_key: None,
                base_url: None,
                enabled: true,
            }),
            openai: Some(ProviderConfig {
                api_key: None,
                base_url: Some("https://api.openai.com/v1".into()),
                enabled: false,
            }),
            ollama: Some(ProviderConfig {
                api_key: None,
                base_url: Some("http://localhost:11434/v1".into()),
                enabled: true,
            }),
            openrouter: Some(ProviderConfig {
                api_key: None,
                base_url: Some("https://openrouter.ai/api/v1".into()),
                enabled: false,
            }),
            custom: Vec::new(),
        }
    }
}

impl ThemeConfig {
    fn default_success_color() -> String {
        "#a6e3a1".into()
    }
    fn default_error_color() -> String {
        "#f38ba8".into()
    }
    fn default_warning_color() -> String {
        "#f9e2af".into()
    }
    fn default_dim_color() -> String {
        "#6c7086".into()
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            user_color: "#89b4fa".into(),
            assistant_color: "#a6e3a1".into(),
            border_color: "#585b70".into(),
            accent_color: "#cba6f7".into(),
            success_color: "#a6e3a1".into(),
            error_color: "#f38ba8".into(),
            warning_color: "#f9e2af".into(),
            dim_color: "#6c7086".into(),
        }
    }
}

impl Default for KeybindsConfig {
    fn default() -> Self {
        Self {
            command_bar: "ctrl+k".into(),
            new_conversation: "ctrl+n".into(),
            prompt_picker: "ctrl+p".into(),
            model_select: "ctrl+m".into(),
            help: "f1".into(),
            copy_last: "ctrl+shift+c".into(),
            quit: "ctrl+q".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Load / save / paths
// ---------------------------------------------------------------------------

impl Config {
    /// Default value for the `auto_agent` field (used by serde).
    fn default_auto_agent() -> bool {
        true
    }

    fn default_auto_model_routing() -> bool {
        true
    }

    fn default_auto_verify() -> bool {
        true
    }

    /// Return the Nerve configuration directory (`~/.config/nerve/`).
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nerve")
    }

    /// Full path to the configuration file.
    fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load the configuration from disk.
    ///
    /// If the file does not exist a default configuration is written and
    /// returned. If the file exists but cannot be parsed, an error is
    /// returned so the user can fix the syntax.
    pub fn load() -> anyhow::Result<Config> {
        let path = Self::config_file();

        if !path.exists() {
            let config = Config::default();
            config.save().context("failed to write default config")?;
            return Ok(config);
        }

        // A read failure (non-UTF8 bytes, permissions, …) must NOT abort
        // startup — fall back to defaults like the parse-error path does,
        // backing up the raw bytes first so nothing is silently lost.
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                if let Ok(bytes) = fs::read(&path) {
                    let _ = fs::write(path.with_extension("toml.bak"), &bytes);
                }
                tracing::warn!(
                    "failed to read config ({}): {e}. Backed up and using defaults.",
                    path.display()
                );
                return Ok(Config::default());
            }
        };
        match toml::from_str::<Config>(&contents) {
            Ok(config) => Ok(config),
            Err(e) => {
                // Old config format or corrupt file — fall back to defaults
                // so the app can still start. Back up the original first so a
                // later save() can't silently overwrite (and lose) the user's
                // real settings, including API keys.
                let backup = path.with_extension("toml.bak");
                if let Err(be) = fs::write(&backup, &contents) {
                    tracing::warn!("failed to back up unparseable config: {be}");
                }
                tracing::warn!(
                    "Config parse error ({}): {e}. Backed up to {} and using defaults.",
                    path.display(),
                    backup.display()
                );
                Ok(Config::default())
            }
        }
    }

    /// Persist the configuration to disk, creating parent directories as
    /// needed. The file is written with helpful comments so first-time
    /// users can edit it by hand.
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create config directory: {}", dir.display()))?;

        let path = Self::config_file();
        let contents = self.to_commented_toml();
        crate::files::atomic_write(&path, &contents)
            .with_context(|| format!("failed to write config: {}", path.display()))?;

        // Restrict file permissions on Unix (config contains API keys)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)
                .with_context(|| format!("failed to read config metadata: {}", path.display()))?
                .permissions();
            perms.set_mode(0o600); // Owner read/write only
            fs::set_permissions(&path, perms)
                .with_context(|| format!("failed to set config permissions: {}", path.display()))?;
        }

        Ok(())
    }

    /// Serialize the config to TOML with leading comments that serve as
    /// documentation for new users.
    pub(crate) fn to_commented_toml(&self) -> String {
        let raw = toml::to_string_pretty(self).unwrap_or_default();

        let header = "\
# ───────────────────────────────────────────────────────────────────────
# Nerve - configuration file
# ───────────────────────────────────────────────────────────────────────
#
# This file is auto-generated on first run. Edit it freely; Nerve will
# reload it on next launch.
#
# Paths:
#   Config  : ~/.config/nerve/config.toml
#   Prompts : ~/.config/nerve/prompts/   (custom .toml prompt files)
#   History : ~/.local/share/nerve/history/
#
# ── General ──────────────────────────────────────────────────────────
# default_model    : Model identifier sent to the provider (e.g.
#                    \"claude-sonnet-4-20250514\", \"gpt-4o\", \"llama3.1\").
# default_provider : Which provider to use by default.
#                    Built-in options: \"claude_code\", \"openai\", \"ollama\",
#                    \"openrouter\". Or the name of a [[providers.custom]] entry.
# auto_agent       : When true, automatically enable agent mode (tool access)
#                    for messages that appear to need it.  Set to false to
#                    require manual \"/agent on\".
# command_timeout_secs  : Max seconds for /run and agent tool commands
#                          before they are killed. Set to 0 to disable.
#                          Default: 30.
#
# ── Theme ────────────────────────────────────────────────────────────
# Colours can be any CSS-style hex code (\"#rrggbb\").
#
# ── Providers ────────────────────────────────────────────────────────
# Set enabled = true and fill in api_key for each provider you use.
# Claude Code is the default — it uses your Claude Code subscription
# (no API key needed). Just make sure `claude` is in your PATH.
# Ollama runs locally and needs no API key.
#
# To add a custom OpenAI-compatible provider, append a section:
#
#   [[providers.custom]]
#   name = \"My Provider\"
#   api_key = \"sk-...\"
#   base_url = \"https://api.example.com/v1\"
#
# ── Keybinds ─────────────────────────────────────────────────────────
# Format: \"modifier+key\" (e.g. \"ctrl+k\", \"ctrl+shift+c\", \"f1\").
# Supported modifiers: ctrl, shift, alt.  Combinable with '+'.
# Special keys: f1-f12, esc, enter, tab, backspace, delete, up, down,
#               left, right, home, end, pageup, pagedown.
#
# ── Retry ────────────────────────────────────────────────────────────
# Controls exponential backoff for transient API errors (429, 5xx,
# connection failures).  Delay formula:
#   delay = min(initial_delay_ms * backoff_factor^attempt + jitter,
#               max_delay_ms)
#
# ───────────────────────────────────────────────────────────────────────

";
        format!("{header}{raw}")
    }
}

// ---------------------------------------------------------------------------
// Theme presets
// ---------------------------------------------------------------------------

/// Built-in theme presets. Returns `(name, theme)` pairs.
pub fn theme_presets() -> Vec<(&'static str, ThemeConfig)> {
    vec![
        (
            "Catppuccin Mocha",
            ThemeConfig {
                user_color: "#89b4fa".into(),
                assistant_color: "#a6e3a1".into(),
                border_color: "#585b70".into(),
                accent_color: "#cba6f7".into(),
                success_color: "#a6e3a1".into(),
                error_color: "#f38ba8".into(),
                warning_color: "#f9e2af".into(),
                dim_color: "#6c7086".into(),
            },
        ),
        (
            "Tokyo Night",
            ThemeConfig {
                user_color: "#7aa2f7".into(),
                assistant_color: "#9ece6a".into(),
                border_color: "#3b4261".into(),
                accent_color: "#bb9af7".into(),
                success_color: "#9ece6a".into(),
                error_color: "#f7768e".into(),
                warning_color: "#e0af68".into(),
                dim_color: "#565f89".into(),
            },
        ),
        (
            "Gruvbox Dark",
            ThemeConfig {
                user_color: "#83a598".into(),
                assistant_color: "#b8bb26".into(),
                border_color: "#504945".into(),
                accent_color: "#d3869b".into(),
                success_color: "#b8bb26".into(),
                error_color: "#fb4934".into(),
                warning_color: "#fabd2f".into(),
                dim_color: "#665c54".into(),
            },
        ),
        (
            "Nord",
            ThemeConfig {
                user_color: "#88c0d0".into(),
                assistant_color: "#a3be8c".into(),
                border_color: "#4c566a".into(),
                accent_color: "#b48ead".into(),
                success_color: "#a3be8c".into(),
                error_color: "#bf616a".into(),
                warning_color: "#ebcb8b".into(),
                dim_color: "#616e88".into(),
            },
        ),
        (
            "Solarized Dark",
            ThemeConfig {
                user_color: "#268bd2".into(),
                assistant_color: "#859900".into(),
                border_color: "#586e75".into(),
                accent_color: "#6c71c4".into(),
                success_color: "#859900".into(),
                error_color: "#dc322f".into(),
                warning_color: "#b58900".into(),
                dim_color: "#657b83".into(),
            },
        ),
        (
            "Dracula",
            ThemeConfig {
                user_color: "#8be9fd".into(),
                assistant_color: "#50fa7b".into(),
                border_color: "#6272a4".into(),
                accent_color: "#ff79c6".into(),
                success_color: "#50fa7b".into(),
                error_color: "#ff5555".into(),
                warning_color: "#f1fa8c".into(),
                dim_color: "#6272a4".into(),
            },
        ),
        (
            "One Dark",
            ThemeConfig {
                user_color: "#61afef".into(),
                assistant_color: "#98c379".into(),
                border_color: "#5c6370".into(),
                accent_color: "#c678dd".into(),
                success_color: "#98c379".into(),
                error_color: "#e06c75".into(),
                warning_color: "#e5c07b".into(),
                dim_color: "#5c6370".into(),
            },
        ),
        (
            "Rose Pine",
            ThemeConfig {
                user_color: "#9ccfd8".into(),
                assistant_color: "#31748f".into(),
                border_color: "#524f67".into(),
                accent_color: "#c4a7e7".into(),
                success_color: "#31748f".into(),
                error_color: "#eb6f92".into(),
                warning_color: "#f6c177".into(),
                dim_color: "#6e6a86".into(),
            },
        ),
        (
            "High Contrast",
            ThemeConfig {
                user_color: "#00ffff".into(),      // bright cyan
                assistant_color: "#ffff00".into(), // bright yellow
                border_color: "#ffffff".into(),    // white borders
                accent_color: "#ff00ff".into(),    // bright magenta
                success_color: "#00ff00".into(),   // bright green
                error_color: "#ff6600".into(),     // orange (avoids red/green ambiguity)
                warning_color: "#ff00ff".into(), // bright magenta (distinct from assistant yellow)
                dim_color: "#aaaaaa".into(),     // light gray
            },
        ),
        (
            "Monochrome",
            ThemeConfig {
                user_color: "#ffffff".into(),      // bright white
                assistant_color: "#cccccc".into(), // light gray
                border_color: "#666666".into(),    // medium gray
                accent_color: "#ffffff".into(),    // white
                success_color: "#cccccc".into(),   // light gray
                error_color: "#ffffff".into(),     // white (relies on context, not color)
                warning_color: "#dddddd".into(),   // near-white
                dim_color: "#888888".into(),       // gray
            },
        ),
    ]
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
