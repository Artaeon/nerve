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
fn config_default_command_timeout() {
    let cfg = Config::default();
    assert_eq!(cfg.command_timeout_secs, 30);
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

#[test]
fn config_default_has_retry() {
    let cfg = Config::default();
    assert_eq!(cfg.retry.max_retries, 3);
    assert_eq!(cfg.retry.initial_delay_ms, 1000);
    assert_eq!(cfg.retry.max_delay_ms, 30_000);
    assert!((cfg.retry.backoff_factor - 2.0).abs() < f64::EPSILON);
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
    assert_eq!(deserialized.retry.max_retries, original.retry.max_retries);
    assert_eq!(
        deserialized.retry.initial_delay_ms,
        original.retry.initial_delay_ms
    );
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
    assert!(output.contains("[retry]"));
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
    assert!(output.contains("Retry"));
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
    // Partial TOML with only top-level fields should deserialize
    // successfully, filling missing sections from Default.
    let partial = r#"
default_model = "gpt-4o"
default_provider = "openai"
"#;
    let cfg: Config = toml::from_str(partial).expect("partial TOML should parse with defaults");
    assert_eq!(cfg.default_model, "gpt-4o");
    assert_eq!(cfg.default_provider, "openai");
    // Missing sections should use their Default values
    assert_eq!(cfg.theme.user_color, ThemeConfig::default().user_color);
    assert_eq!(cfg.keybinds.quit, KeybindsConfig::default().quit);
}

// ── command_timeout_secs parsing ──────────────────────────────────

#[test]
fn config_parse_custom_timeout() {
    let toml_str = r#"
default_model = "sonnet"
default_provider = "claude_code"
command_timeout_secs = 120
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.command_timeout_secs, 120);
}

#[test]
fn config_parse_timeout_zero() {
    let toml_str = r"
command_timeout_secs = 0
";
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.command_timeout_secs, 0);
}

#[test]
fn config_missing_timeout_gets_default() {
    let toml_str = r#"
default_model = "sonnet"
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.command_timeout_secs, 30);
}

#[test]
fn config_timeout_roundtrip() {
    let cfg = Config {
        command_timeout_secs: 60,
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(restored.command_timeout_secs, 60);
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
    // Empty TOML should now succeed with all defaults thanks to #[serde(default)].
    let cfg: Config = toml::from_str("").expect("empty TOML should parse with defaults");
    assert_eq!(cfg.default_model, Config::default().default_model);
    assert_eq!(cfg.default_provider, Config::default().default_provider);
}

// ── Theme presets ──────────────────────────────────────────────────────

#[test]
fn theme_presets_not_empty() {
    let presets = theme_presets();
    assert!(presets.len() >= 10);
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
fn exactly_ten_theme_presets() {
    assert_eq!(theme_presets().len(), 10);
}

// ── Git config ──────────────────────────────────────────────────────

#[test]
fn config_default_git_fields_none() {
    let cfg = Config::default();
    assert!(cfg.git_user_name.is_none());
    assert!(cfg.git_user_email.is_none());
}

#[test]
fn config_parse_with_git_fields() {
    let toml_str = r#"
default_model = "sonnet"
default_provider = "claude_code"
git_user_name = "Jane Doe"
git_user_email = "jane@example.com"
"#;
    let cfg: Config = toml::from_str(toml_str).expect("should parse TOML with git fields");
    assert_eq!(cfg.git_user_name.as_deref(), Some("Jane Doe"));
    assert_eq!(cfg.git_user_email.as_deref(), Some("jane@example.com"));
}

#[test]
fn config_parse_without_git_fields() {
    let toml_str = r#"
default_model = "sonnet"
default_provider = "claude_code"
"#;
    let cfg: Config = toml::from_str(toml_str).expect("should parse TOML without git fields");
    assert!(cfg.git_user_name.is_none());
    assert!(cfg.git_user_email.is_none());
}

#[test]
fn config_roundtrip_with_git_fields() {
    let cfg = Config {
        git_user_name: Some("Test User".into()),
        git_user_email: Some("test@example.com".into()),
        ..Config::default()
    };

    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();

    assert_eq!(restored.git_user_name.as_deref(), Some("Test User"));
    assert_eq!(restored.git_user_email.as_deref(), Some("test@example.com"));
}

#[test]
fn config_roundtrip_git_fields_none() {
    let cfg = Config::default();
    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();
    assert!(restored.git_user_name.is_none());
    assert!(restored.git_user_email.is_none());
}

// ── New fields: temperature, top_p, context_limit ─────────────────

#[test]
fn config_default_new_fields_are_none() {
    let cfg = Config::default();
    assert!(cfg.temperature.is_none());
    assert!(cfg.top_p.is_none());
    assert!(cfg.context_limit.is_none());
}

#[test]
fn config_roundtrip_temperature() {
    let cfg = Config {
        temperature: Some(0.7),
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(restored.temperature, Some(0.7));
}

#[test]
fn config_roundtrip_top_p() {
    let cfg = Config {
        top_p: Some(0.9),
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(restored.top_p, Some(0.9));
}

#[test]
fn config_roundtrip_context_limit() {
    let cfg = Config {
        context_limit: Some(128_000),
        ..Config::default()
    };
    let toml_str = toml::to_string(&cfg).unwrap();
    let restored: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(restored.context_limit, Some(128_000));
}

#[test]
fn config_load_missing_new_fields_defaults_to_none() {
    // Simulate an old config without the new fields
    let toml_str = r#"
default_model = "gpt-4o"
default_provider = "openai"
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.temperature.is_none());
    assert!(cfg.top_p.is_none());
    assert!(cfg.context_limit.is_none());
}
