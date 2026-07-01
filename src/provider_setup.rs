use anyhow::Context;

use crate::ai::provider::AiProvider;
use crate::ai::{ClaudeCodeProvider, CopilotProvider, OpenAiProvider};
use crate::app::App;
use crate::config::Config;

// ─── Provider creation ──────────────────────────────────────────────────────

/// Apply config-level sampling parameters to an OpenAI-compatible provider.
fn apply_sampling(mut provider: OpenAiProvider, config: &Config) -> OpenAiProvider {
    if let Some(t) = config.temperature {
        provider = provider.with_temperature(t);
    }
    if let Some(p) = config.top_p {
        provider = provider.with_top_p(p);
    }
    provider
}

pub(crate) fn create_provider(
    config: &Config,
    provider_override: Option<&str>,
) -> anyhow::Result<Box<dyn AiProvider>> {
    let provider_name = provider_override.unwrap_or(&config.default_provider);
    match provider_name {
        "claude_code" | "claude" => Ok(Box::new(ClaudeCodeProvider::new())),
        "openai" => {
            let pc = config.providers.openai.as_ref();
            let key = resolve_api_key(pc.and_then(|p| p.api_key.as_deref()), "OPENAI_API_KEY")?;
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            let p = OpenAiProvider::new(key, base_url, "OpenAI".into())
                .with_retry_config(config.retry.clone());
            Ok(Box::new(apply_sampling(p, config)))
        }
        "ollama" => {
            let pc = config.providers.ollama.as_ref();
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".into());
            let p = OpenAiProvider::new("ollama".into(), base_url, "Ollama".into())
                .with_retry_config(config.retry.clone());
            Ok(Box::new(apply_sampling(p, config)))
        }
        "openrouter" => {
            let pc = config.providers.openrouter.as_ref();
            let key = resolve_api_key(pc.and_then(|p| p.api_key.as_deref()), "OPENROUTER_API_KEY")?;
            let base_url = pc
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".into());
            let p = OpenAiProvider::new(key, base_url, "OpenRouter".into())
                .with_retry_config(config.retry.clone());
            Ok(Box::new(apply_sampling(p, config)))
        }
        "copilot" | "gh" => Ok(Box::new(CopilotProvider::new())),
        other => {
            // Check custom providers.
            if let Some(custom) = config.providers.custom.iter().find(|c| c.name == other) {
                let p = OpenAiProvider::new(
                    custom.api_key.clone(),
                    custom.base_url.clone(),
                    custom.name.clone(),
                )
                .with_retry_config(config.retry.clone());
                return Ok(Box::new(apply_sampling(p, config)));
            }
            let help = provider_help_message(other);
            anyhow::bail!("{help}")
        }
    }
}

/// Create a provider using app state (respects code_mode for claude_code).
pub(crate) fn create_provider_from_app(
    config: &Config,
    app: &App,
) -> anyhow::Result<Box<dyn AiProvider>> {
    let provider_name = &app.selected_provider;
    match provider_name.as_str() {
        "claude_code" | "claude" => {
            if app.code_mode {
                let mut p = ClaudeCodeProvider::with_tools();
                if let Some(ref dir) = app.working_dir {
                    p = p.with_working_dir(dir.clone());
                }
                Ok(Box::new(p))
            } else {
                Ok(Box::new(ClaudeCodeProvider::new()))
            }
        }
        _ => create_provider(config, Some(provider_name)),
    }
}

/// Detect locally-installed Ollama models by shelling out to `ollama list`.
/// Falls back to a single "llama3" entry if Ollama is unavailable.
pub(crate) fn detect_ollama_models() -> Vec<String> {
    match std::process::Command::new("ollama").args(["list"]).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let models: Vec<String> = stdout
                .lines()
                .skip(1) // Skip header line "NAME  ID  SIZE  MODIFIED"
                .filter_map(|line| {
                    let name = line.split_whitespace().next()?;
                    if name.is_empty() {
                        None
                    } else {
                        Some(name.to_string())
                    }
                })
                .collect();
            if models.is_empty() {
                vec!["llama3".into()]
            } else {
                models
            }
        }
        _ => vec!["llama3".into()], // Fallback if ollama not running
    }
}

/// Return the default model for a given provider.
pub(crate) fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "claude_code" | "claude" => "sonnet",
        "openai" => "gpt-4o",
        "openrouter" => "anthropic/claude-sonnet-4-20250514",
        "ollama" => "llama3",
        "copilot" | "gh" => "copilot",
        _ => "default",
    }
}

/// Return the list of available models for a given provider.
pub(crate) fn models_for_provider(provider: &str) -> Vec<String> {
    match provider {
        "claude_code" | "claude" => vec!["opus".into(), "sonnet".into(), "haiku".into()],
        "openai" => vec!["gpt-4o".into(), "gpt-4o-mini".into()],
        "openrouter" => vec![
            "anthropic/claude-sonnet-4-20250514".into(),
            "openai/gpt-4o".into(),
            "meta-llama/llama-3-70b".into(),
            "google/gemini-pro".into(),
        ],
        "copilot" | "gh" => vec!["copilot".into()],
        "ollama" => detect_ollama_models(),
        _ => vec!["default".into()],
    }
}

/// Resolve an API key: prefer the config value, fall back to an environment
/// variable. Returns an error if neither is set.
fn resolve_api_key(config_value: Option<&str>, env_var: &str) -> anyhow::Result<String> {
    if let Some(val) = config_value
        && !val.is_empty()
    {
        return Ok(val.to_string());
    }
    std::env::var(env_var)
        .with_context(|| format!("API key not found: set it in config or via ${env_var}"))
}

// ─── List models ────────────────────────────────────────────────────────────

pub(crate) async fn list_models(provider: &dyn AiProvider) -> anyhow::Result<()> {
    let models = provider.list_models().await?;
    if models.is_empty() {
        println!("No models found for provider '{}'.", provider.name());
        return Ok(());
    }
    println!("Available models ({}):", provider.name());
    for m in &models {
        let ctx = m
            .context_length
            .map(|c| format!("  (ctx: {c})"))
            .unwrap_or_default();
        println!("  {}{ctx}", m.id);
    }
    Ok(())
}

/// Save the last used provider+model so new sessions remember the choice.
pub(crate) fn save_last_provider(provider: &str, model: &str) {
    let dir = dirs::config_dir().unwrap_or_default().join("nerve");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("failed to create config directory: {e}");
        return;
    }
    if let Err(e) = std::fs::write(dir.join("last_provider"), format!("{provider}\n{model}")) {
        tracing::warn!("failed to save last provider: {e}");
    }
}

/// Load the last used provider+model (if saved).
pub(crate) fn load_last_provider() -> Option<(String, String)> {
    let path = dirs::config_dir()?.join("nerve").join("last_provider");
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let provider = lines.next()?.to_string();
    let model = lines.next()?.to_string();
    if provider.is_empty() {
        return None;
    }
    Some((provider, model))
}

pub(crate) fn provider_help_message(provider: &str) -> String {
    match provider {
        "openai" => "OpenAI requires an API key.\n\n\
            Set it via:\n\
            \x20 1. Config: ~/.config/nerve/config.toml -> providers.openai.api_key\n\
            \x20 2. Environment: export OPENAI_API_KEY=\"sk-...\"\n\n\
            Get a key at: https://platform.openai.com/api-keys"
            .into(),
        "openrouter" => "OpenRouter requires an API key.\n\n\
            Set it via:\n\
            \x20 1. Config: ~/.config/nerve/config.toml -> providers.openrouter.api_key\n\
            \x20 2. Environment: export OPENROUTER_API_KEY=\"sk-or-...\"\n\n\
            Get a key at: https://openrouter.ai/keys"
            .into(),
        "claude_code" | "claude" => "Claude Code requires the `claude` CLI.\n\n\
            Install it from: https://claude.ai/code\n\
            Verify: claude --version"
            .into(),
        "ollama" => "Ollama needs to be running locally.\n\n\
            Install: https://ollama.ai\n\
            Start: ollama serve\n\
            Pull a model: ollama pull llama3"
            .into(),
        "copilot" | "gh" => "GitHub Copilot requires the `gh` CLI with Copilot extension.\n\n\
            Install gh: https://cli.github.com\n\
            Add Copilot: gh extension install github/gh-copilot"
            .into(),
        _ => format!(
            "Unknown provider: {provider}\n\nAvailable: claude_code, openai, openrouter, ollama, copilot"
        ),
    }
}
