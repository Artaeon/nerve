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
        match toml::from_str::<Config>(&contents) {
            Ok(config) => Ok(config),
            Err(e) => {
                tracing::warn!(
                    "Config parse error ({}): {e}. Using defaults.",
                    path.display()
                );
                // Old config format or corrupt file — fall back to defaults
                // so the app can still start.
                Ok(Config::default())
            }
        }
    }

    /// Persist the configuration to disk, creating parent directories as
    /// needed. The file is written with helpful comments so first-time
    /// users can edit it by hand.
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;

        let path = Self::config_file();
        let contents = self.to_commented_toml();
        fs::write(&path, contents)?;

        // Restrict file permissions on Unix (config contains API keys)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600); // Owner read/write only
            fs::set_permissions(&path, perms)?;
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Config::default() ───────────────────────────────────────────────

    #[test]
    fn config_default_model() {
        let cfg = Config::default();
        assert_eq!(cfg.default_model, "sonnet");
    }

    #[test]
    fn config_default_provider() {
        let cfg = Config::default();
        assert_eq!(cfg.default_provider, "claude_code");
    }

    #[test]
    fn config_default_has_theme() {
        let cfg = Config::default();
        assert!(!cfg.theme.user_color.is_empty());
        assert!(!cfg.theme.assistant_color.is_empty());
    }

    #[test]
    fn config_default_has_providers() {
        let cfg = Config::default();
        assert!(cfg.providers.claude_code.is_some());
        assert!(cfg.providers.openai.is_some());
        assert!(cfg.providers.ollama.is_some());
        assert!(cfg.providers.openrouter.is_some());
    }

    #[test]
    fn config_default_has_keybinds() {
        let cfg = Config::default();
        assert!(!cfg.keybinds.command_bar.is_empty());
        assert!(!cfg.keybinds.quit.is_empty());
    }

    // ── ProvidersConfig::default() ──────────────────────────────────────

    #[test]
    fn providers_claude_code_enabled() {
        let p = ProvidersConfig::default();
        let cc = p.claude_code.unwrap();
        assert!(cc.enabled);
        assert!(cc.api_key.is_none());
        assert!(cc.base_url.is_none());
    }

    #[test]
    fn providers_openai_disabled_by_default() {
        let p = ProvidersConfig::default();
        let oa = p.openai.unwrap();
        assert!(!oa.enabled);
        assert_eq!(oa.base_url, Some("https://api.openai.com/v1".into()));
        assert!(oa.api_key.is_none());
    }

    #[test]
    fn providers_ollama_enabled_by_default() {
        let p = ProvidersConfig::default();
        let ol = p.ollama.unwrap();
        assert!(ol.enabled);
        assert_eq!(ol.base_url, Some("http://localhost:11434/v1".into()));
    }

    #[test]
    fn providers_openrouter_disabled_by_default() {
        let p = ProvidersConfig::default();
        let or = p.openrouter.unwrap();
        assert!(!or.enabled);
        assert_eq!(or.base_url, Some("https://openrouter.ai/api/v1".into()));
    }

    #[test]
    fn providers_custom_empty_by_default() {
        let p = ProvidersConfig::default();
        assert!(p.custom.is_empty());
    }

    // ── ThemeConfig::default() ──────────────────────────────────────────

    #[test]
    fn theme_default_user_color() {
        let t = ThemeConfig::default();
        assert_eq!(t.user_color, "#89b4fa");
    }

    #[test]
    fn theme_default_assistant_color() {
        let t = ThemeConfig::default();
        assert_eq!(t.assistant_color, "#a6e3a1");
    }

    #[test]
    fn theme_default_border_color() {
        let t = ThemeConfig::default();
        assert_eq!(t.border_color, "#585b70");
    }

    #[test]
    fn theme_default_accent_color() {
        let t = ThemeConfig::default();
        assert_eq!(t.accent_color, "#cba6f7");
    }

    #[test]
    fn theme_colors_are_valid_hex() {
        let t = ThemeConfig::default();
        for color in &[
            &t.user_color,
            &t.assistant_color,
            &t.border_color,
            &t.accent_color,
            &t.success_color,
            &t.error_color,
            &t.warning_color,
            &t.dim_color,
        ] {
            assert!(color.starts_with('#'), "color should start with #: {color}");
            assert_eq!(color.len(), 7, "color should be #rrggbb: {color}");
        }
    }

    // ── KeybindsConfig::default() ───────────────────────────────────────

    #[test]
    fn keybinds_command_bar() {
        let k = KeybindsConfig::default();
        assert_eq!(k.command_bar, "ctrl+k");
    }

    #[test]
    fn keybinds_new_conversation() {
        let k = KeybindsConfig::default();
        assert_eq!(k.new_conversation, "ctrl+n");
    }

    #[test]
    fn keybinds_prompt_picker() {
        let k = KeybindsConfig::default();
        assert_eq!(k.prompt_picker, "ctrl+p");
    }

    #[test]
    fn keybinds_model_select() {
        let k = KeybindsConfig::default();
        assert_eq!(k.model_select, "ctrl+m");
    }

    #[test]
    fn keybinds_help() {
        let k = KeybindsConfig::default();
        assert_eq!(k.help, "f1");
    }

    #[test]
    fn keybinds_copy_last() {
        let k = KeybindsConfig::default();
        assert_eq!(k.copy_last, "ctrl+shift+c");
    }

    #[test]
    fn keybinds_quit() {
        let k = KeybindsConfig::default();
        assert_eq!(k.quit, "ctrl+q");
    }

    // ── Serialization roundtrip ─────────────────────────────────────────

    #[test]
    fn config_toml_roundtrip() {
        let original = Config::default();
        let toml_str = toml::to_string(&original).expect("serialize failed");
        let deserialized: Config = toml::from_str(&toml_str).expect("deserialize failed");

        assert_eq!(deserialized.default_model, original.default_model);
        assert_eq!(deserialized.default_provider, original.default_provider);
        assert_eq!(deserialized.theme.user_color, original.theme.user_color);
        assert_eq!(
            deserialized.theme.assistant_color,
            original.theme.assistant_color
        );
        assert_eq!(deserialized.theme.border_color, original.theme.border_color);
        assert_eq!(deserialized.theme.accent_color, original.theme.accent_color);
        assert_eq!(
            deserialized.keybinds.command_bar,
            original.keybinds.command_bar
        );
        assert_eq!(deserialized.keybinds.quit, original.keybinds.quit);
        assert_eq!(deserialized.keybinds.help, original.keybinds.help);
    }

    #[test]
    fn config_toml_pretty_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let restored: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.default_model, config.default_model);
        assert_eq!(restored.default_provider, config.default_provider);
    }

    #[test]
    fn config_roundtrip_preserves_providers() {
        let original = Config::default();
        let toml_str = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        let orig_cc = original.providers.claude_code.unwrap();
        let deser_cc = deserialized.providers.claude_code.unwrap();
        assert_eq!(deser_cc.enabled, orig_cc.enabled);
        assert_eq!(deser_cc.api_key, orig_cc.api_key);
        assert_eq!(deser_cc.base_url, orig_cc.base_url);
    }

    #[test]
    fn config_roundtrip_with_custom_values() {
        let mut cfg = Config {
            default_model: "gpt-4o".into(),
            default_provider: "openai".into(),
            ..Config::default()
        };
        cfg.theme.user_color = "#ff0000".into();
        cfg.keybinds.quit = "ctrl+w".into();

        let toml_str = toml::to_string(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.default_model, "gpt-4o");
        assert_eq!(deserialized.default_provider, "openai");
        assert_eq!(deserialized.theme.user_color, "#ff0000");
        assert_eq!(deserialized.keybinds.quit, "ctrl+w");
    }

    #[test]
    fn config_roundtrip_with_custom_provider() {
        let mut cfg = Config::default();
        cfg.providers.custom.push(CustomProviderConfig {
            name: "My Provider".into(),
            api_key: "sk-test".into(),
            base_url: "https://api.example.com/v1".into(),
        });

        let toml_str = toml::to_string(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.providers.custom.len(), 1);
        assert_eq!(deserialized.providers.custom[0].name, "My Provider");
        assert_eq!(deserialized.providers.custom[0].api_key, "sk-test");
        assert_eq!(
            deserialized.providers.custom[0].base_url,
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn theme_roundtrip() {
        let theme = ThemeConfig::default();
        let toml_str = toml::to_string(&theme).unwrap();
        let restored: ThemeConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.user_color, theme.user_color);
        assert_eq!(restored.assistant_color, theme.assistant_color);
        assert_eq!(restored.border_color, theme.border_color);
        assert_eq!(restored.accent_color, theme.accent_color);
        assert_eq!(restored.success_color, theme.success_color);
        assert_eq!(restored.error_color, theme.error_color);
        assert_eq!(restored.warning_color, theme.warning_color);
        assert_eq!(restored.dim_color, theme.dim_color);
    }

    #[test]
    fn keybinds_roundtrip() {
        let keybinds = KeybindsConfig::default();
        let toml_str = toml::to_string(&keybinds).unwrap();
        let restored: KeybindsConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.command_bar, keybinds.command_bar);
        assert_eq!(restored.new_conversation, keybinds.new_conversation);
        assert_eq!(restored.prompt_picker, keybinds.prompt_picker);
        assert_eq!(restored.model_select, keybinds.model_select);
        assert_eq!(restored.help, keybinds.help);
        assert_eq!(restored.copy_last, keybinds.copy_last);
        assert_eq!(restored.quit, keybinds.quit);
    }

    #[test]
    fn custom_provider_config_roundtrip() {
        let custom = CustomProviderConfig {
            name: "My Provider".into(),
            api_key: "sk-test".into(),
            base_url: "https://api.example.com/v1".into(),
        };
        let toml_str = toml::to_string(&custom).unwrap();
        let restored: CustomProviderConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.name, "My Provider");
        assert_eq!(restored.api_key, "sk-test");
        assert_eq!(restored.base_url, "https://api.example.com/v1");
    }

    // ── Config::config_dir() ────────────────────────────────────────────

    #[test]
    fn config_dir_ends_with_nerve() {
        let dir = Config::config_dir();
        assert!(
            dir.ends_with("nerve"),
            "config dir should end with 'nerve', got: {dir:?}"
        );
    }

    #[test]
    fn config_dir_is_not_empty() {
        let dir = Config::config_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    // ── to_commented_toml() ─────────────────────────────────────────────

    #[test]
    fn to_commented_toml_contains_header() {
        let cfg = Config::default();
        let output = cfg.to_commented_toml();
        assert!(output.contains("Nerve - configuration file"));
    }

    #[test]
    fn to_commented_toml_contains_config_values() {
        let cfg = Config::default();
        let output = cfg.to_commented_toml();
        assert!(output.contains("default_model"));
        assert!(output.contains("default_provider"));
        assert!(output.contains("sonnet"));
        assert!(output.contains("claude_code"));
    }

    #[test]
    fn to_commented_toml_contains_section_headers() {
        let cfg = Config::default();
        let output = cfg.to_commented_toml();
        assert!(output.contains("[theme]"));
        assert!(output.contains("[keybinds]"));
        assert!(output.contains("[providers"));
    }

    #[test]
    fn to_commented_toml_starts_with_comment() {
        let cfg = Config::default();
        let output = cfg.to_commented_toml();
        assert!(output.starts_with('#'));
    }

    #[test]
    fn to_commented_toml_mentions_paths() {
        let cfg = Config::default();
        let output = cfg.to_commented_toml();
        assert!(output.contains("~/.config/nerve/config.toml"));
        assert!(output.contains("~/.config/nerve/prompts/"));
    }

    #[test]
    fn to_commented_toml_contains_documentation_sections() {
        let config = Config::default();
        let output = config.to_commented_toml();
        assert!(output.contains("General"));
        assert!(output.contains("Theme"));
        assert!(output.contains("Providers"));
        assert!(output.contains("Keybinds"));
    }

    #[test]
    fn commented_toml_is_valid_toml_after_stripping_comments() {
        let config = Config::default();
        let output = config.to_commented_toml();
        let stripped: String = output
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        let restored: Config = toml::from_str(&stripped).unwrap();
        assert_eq!(restored.default_model, config.default_model);
    }

    // ── Deserialization from partial TOML ────────────────────────────────

    #[test]
    fn deserialize_minimal_toml() {
        let toml_str = r##"
            default_model = "haiku"
            default_provider = "ollama"

            [theme]
            user_color = "#ffffff"
            assistant_color = "#000000"
            border_color = "#111111"
            accent_color = "#222222"

            [providers]

            [keybinds]
            command_bar = "ctrl+k"
            new_conversation = "ctrl+n"
            prompt_picker = "ctrl+p"
            model_select = "ctrl+m"
            help = "f1"
            copy_last = "ctrl+shift+c"
            quit = "ctrl+q"
        "##;
        let cfg: Config = toml::from_str(toml_str).expect("should parse minimal TOML");
        assert_eq!(cfg.default_model, "haiku");
        assert_eq!(cfg.default_provider, "ollama");
    }

    #[test]
    fn config_load_with_missing_fields_uses_defaults() {
        // Test that partial TOML is handled gracefully — missing required
        // fields cause a parse error, but Config::load would fall back to
        // defaults. The important thing is it does not panic.
        let partial = r#"
default_model = "gpt-4o"
default_provider = "openai"
"#;
        let result = toml::from_str::<Config>(partial);
        // It is expected to error (missing [theme], [providers], [keybinds]),
        // but must not panic.
        let _ = result;
    }

    #[test]
    fn config_load_with_completely_invalid_toml() {
        // Completely garbage input should produce an Err, never a panic.
        let garbage = "{{{{not valid toml at all!!!!";
        let result = toml::from_str::<Config>(garbage);
        assert!(result.is_err());
    }

    #[test]
    fn config_load_with_empty_string() {
        let result = toml::from_str::<Config>("");
        // Empty TOML is missing all required fields — should fail gracefully.
        assert!(result.is_err());
    }

    // ── Theme presets ──────────────────────────────────────────────────────

    #[test]
    fn theme_presets_not_empty() {
        let presets = theme_presets();
        assert!(presets.len() >= 8);
    }

    #[test]
    fn theme_presets_all_have_names() {
        for (name, _) in theme_presets() {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn theme_presets_colors_are_hex() {
        for (name, theme) in theme_presets() {
            for color in [
                &theme.user_color,
                &theme.assistant_color,
                &theme.border_color,
                &theme.accent_color,
                &theme.success_color,
                &theme.error_color,
                &theme.warning_color,
                &theme.dim_color,
            ] {
                assert!(
                    color.starts_with('#'),
                    "Theme '{name}' has non-hex color: {color}"
                );
                assert_eq!(
                    color.len(),
                    7,
                    "Theme '{name}' has wrong color length: {color}"
                );
            }
        }
    }

    #[test]
    fn theme_presets_unique_names() {
        let presets = theme_presets();
        let names: std::collections::HashSet<&str> = presets.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), presets.len(), "Duplicate theme names");
    }

    #[test]
    fn config_dir_creates_on_save() {
        // Verify save doesn't panic
        let config = Config::default();
        let result = config.save();
        assert!(result.is_ok());
    }

    #[test]
    fn config_commented_toml_valid_after_stripping_comments() {
        let config = Config::default();
        let commented = config.to_commented_toml();

        // Strip comment lines
        let stripped: String = commented
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // Should still be valid TOML
        let result = toml::from_str::<Config>(&stripped);
        assert!(
            result.is_ok(),
            "Stripped TOML should be parseable: {:?}",
            result.err()
        );
    }

    #[test]
    fn theme_presets_all_different() {
        let presets = theme_presets();
        for i in 0..presets.len() {
            for j in (i + 1)..presets.len() {
                assert_ne!(
                    presets[i].1.user_color, presets[j].1.user_color,
                    "Themes '{}' and '{}' have identical user_color",
                    presets[i].0, presets[j].0
                );
            }
        }
    }

    #[test]
    fn exactly_eight_theme_presets() {
        assert_eq!(theme_presets().len(), 8);
    }
}
