use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Top-level application configuration for Nerve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default model identifier (e.g. "llama3.1", "gpt-4o").
    pub default_model: String,
    /// Default provider key: "openai", "ollama", "openrouter", or the name
    /// of a custom provider.
    pub default_provider: String,
    /// Theme colours.
    pub theme: ThemeConfig,
    /// Provider connection settings.
    pub providers: ProvidersConfig,
    /// Keyboard shortcuts.
    pub keybinds: KeybindsConfig,
}

/// Connections for each supported AI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct ThemeConfig {
    pub user_color: String,
    pub assistant_color: String,
    pub border_color: String,
    pub accent_color: String,
}

/// Configurable keyboard shortcuts (stored as human-readable strings such
/// as `"ctrl+k"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            theme: ThemeConfig::default(),
            providers: ProvidersConfig::default(),
            keybinds: KeybindsConfig::default(),
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

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            user_color: "#89b4fa".into(),
            assistant_color: "#a6e3a1".into(),
            border_color: "#585b70".into(),
            accent_color: "#cba6f7".into(),
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
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Persist the configuration to disk, creating parent directories as
    /// needed. The file is written with helpful comments so first-time
    /// users can edit it by hand.
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;

        let path = Self::config_file();
        let contents = self.to_commented_toml();
        fs::write(path, contents)?;
        Ok(())
    }

    /// Serialize the config to TOML with leading comments that serve as
    /// documentation for new users.
    fn to_commented_toml(&self) -> String {
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
# ───────────────────────────────────────────────────────────────────────

";
        format!("{header}{raw}")
    }
}
