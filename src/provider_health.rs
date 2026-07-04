//! Startup health checks for AI providers.
//!
//! Nerve's default provider shells out to the `claude` CLI; other providers
//! need API keys or a local server. Rather than letting the *first prompt*
//! fail with a raw error, we cheaply probe the configured provider at startup
//! and — when it can't work — fall back to the best available alternative so
//! nerve works out of the box on as many machines as possible.
//!
//! Checks are deliberately cheap (PATH scan, TCP connect with a short
//! timeout, env/config presence). We never spawn provider binaries or make
//! HTTP requests here: startup must stay fast.

use std::time::Duration;

use crate::config::Config;

/// Result of probing a single provider.
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub healthy: bool,
    /// Human-readable reason (why it's healthy / what's missing).
    pub detail: String,
}

/// Probe whether `provider` can plausibly serve requests right now.
pub fn check_provider(provider: &str, config: &Config) -> HealthReport {
    let (healthy, detail) = match provider {
        "claude_code" | "claude" => {
            if binary_on_path("claude") {
                (true, "claude CLI found".into())
            } else {
                (
                    false,
                    "`claude` CLI not found on PATH (install: https://claude.ai/code)".into(),
                )
            }
        }
        "ollama" => {
            let base_url = config
                .providers
                .ollama
                .as_ref()
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "http://localhost:11434/v1".into());
            if tcp_reachable(&base_url, Duration::from_millis(300)) {
                (true, "Ollama server reachable".into())
            } else {
                (
                    false,
                    format!("Ollama not reachable at {base_url} (start: ollama serve)"),
                )
            }
        }
        "openai" => key_health(
            config
                .providers
                .openai
                .as_ref()
                .and_then(|p| p.api_key.as_deref()),
            "OPENAI_API_KEY",
        ),
        "openrouter" => key_health(
            config
                .providers
                .openrouter
                .as_ref()
                .and_then(|p| p.api_key.as_deref()),
            "OPENROUTER_API_KEY",
        ),
        "copilot" | "gh" => {
            if binary_on_path("gh") {
                (true, "gh CLI found".into())
            } else {
                (
                    false,
                    "`gh` CLI not found on PATH (install: https://cli.github.com)".into(),
                )
            }
        }
        other => {
            // Custom providers: healthy if configured (we can't cheaply probe).
            if config.providers.custom.iter().any(|c| c.name == other) {
                (true, "custom provider configured".into())
            } else {
                (false, format!("unknown provider '{other}'"))
            }
        }
    };
    HealthReport { healthy, detail }
}

/// Pick the best available provider when `preferred` is unhealthy.
///
/// Returns `Some((provider, reason))` for the first healthy candidate in
/// preference order, or `None` when nothing on this machine can serve
/// requests. Never returns `preferred` itself.
pub fn pick_fallback(preferred: &str, config: &Config) -> Option<(String, String)> {
    const ORDER: &[&str] = &["claude_code", "ollama", "openai", "openrouter", "copilot"];
    for candidate in ORDER {
        if *candidate == preferred
            // "claude" is an alias for "claude_code" — don't fall back to the
            // same provider under its other name.
            || (preferred == "claude" && *candidate == "claude_code")
        {
            continue;
        }
        let report = check_provider(candidate, config);
        if report.healthy {
            return Some((candidate.to_string(), report.detail));
        }
    }
    None
}

/// Multi-provider setup guidance shown when *no* provider is available.
pub fn no_provider_guidance() -> String {
    "No AI provider is available on this machine. Set up any ONE of:\n\n\
     \x20 1. Claude (recommended) — install the claude CLI from https://claude.ai/code,\n\
     \x20    then run `claude` once to log in.\n\
     \x20 2. Ollama (free, local) — install from https://ollama.ai, then:\n\
     \x20    ollama serve && ollama pull llama3\n\
     \x20 3. OpenAI — export OPENAI_API_KEY=\"sk-...\"\n\
     \x20 4. OpenRouter — export OPENROUTER_API_KEY=\"sk-or-...\"\n\n\
     Then re-run nerve. Keys can also go in ~/.config/nerve/config.toml."
        .into()
}

// ── Probes ──────────────────────────────────────────────────────────────────

fn key_health(config_key: Option<&str>, env_var: &str) -> (bool, String) {
    let has_config = config_key.is_some_and(|k| !k.is_empty());
    let has_env = std::env::var(env_var).is_ok_and(|v| !v.is_empty());
    if has_config || has_env {
        (true, "API key configured".into())
    } else {
        (false, format!("no API key (set ${env_var} or config)"))
    }
}

/// Is an executable named `name` on the current PATH?
fn binary_on_path(name: &str) -> bool {
    binary_on_path_in(name, &std::env::var_os("PATH").unwrap_or_default())
}

fn binary_on_path_in(name: &str, path_var: &std::ffi::OsStr) -> bool {
    std::env::split_paths(path_var).any(|dir| {
        let candidate = dir.join(name);
        is_executable(&candidate)
    })
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Can we open a TCP connection to the host:port of `url` within `timeout`?
fn tcp_reachable(url: &str, timeout: Duration) -> bool {
    let Some(rest) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    else {
        return false;
    };
    let host_port = rest.split('/').next().unwrap_or("");
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
        None => (host_port, if url.starts_with("https") { 443 } else { 80 }),
    };
    if host.is_empty() {
        return false;
    }
    use std::net::ToSocketAddrs;
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if std::net::TcpStream::connect_timeout(&addr, timeout).is_ok() {
            return true;
        }
    }
    false
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig};

    fn config_with_openai_key(key: &str) -> Config {
        let mut config = Config::default();
        config.providers.openai = Some(ProviderConfig {
            api_key: Some(key.into()),
            base_url: None,
            enabled: true,
        });
        config
    }

    #[test]
    fn binary_on_path_finds_executable() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("fakebin");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path_var = std::ffi::OsString::from(dir.path());
        assert!(binary_on_path_in("fakebin", &path_var));
        assert!(!binary_on_path_in("missing-binary", &path_var));
    }

    #[cfg(unix)]
    #[test]
    fn binary_on_path_ignores_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notexec");
        std::fs::write(&file, "data").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let path_var = std::ffi::OsString::from(dir.path());
        assert!(!binary_on_path_in("notexec", &path_var));
    }

    #[test]
    fn openai_healthy_with_config_key() {
        let config = config_with_openai_key("sk-test");
        let report = check_provider("openai", &config);
        assert!(report.healthy);
    }

    #[test]
    fn openai_unhealthy_with_empty_key_and_no_env() {
        // Empty config key; the env var may exist on dev machines, so only
        // assert when it's absent.
        if std::env::var("OPENAI_API_KEY").is_ok() {
            return;
        }
        let config = config_with_openai_key("");
        let report = check_provider("openai", &config);
        assert!(!report.healthy);
        assert!(report.detail.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn unknown_provider_unhealthy() {
        let config = Config::default();
        let report = check_provider("nonsense", &config);
        assert!(!report.healthy);
    }

    #[test]
    fn ollama_unreachable_reports_unhealthy() {
        let mut config = Config::default();
        // Reserved TEST-NET-1 address: guaranteed unroutable, fails fast.
        config.providers.ollama = Some(ProviderConfig {
            api_key: None,
            base_url: Some("http://192.0.2.1:1".into()),
            enabled: true,
        });
        let report = check_provider("ollama", &config);
        assert!(!report.healthy);
    }

    #[test]
    fn fallback_never_returns_preferred() {
        let config = config_with_openai_key("sk-test");
        // openai is healthy but is also the preferred provider — must not
        // be suggested as its own fallback.
        if let Some((provider, _)) = pick_fallback("openai", &config) {
            assert_ne!(provider, "openai");
        }
    }

    #[test]
    fn fallback_finds_openai_when_key_set() {
        let config = config_with_openai_key("sk-test");
        // Preferred = something unhealthy and un-aliased. Whether claude/
        // ollama are healthy depends on the machine; assert only that SOME
        // fallback is found (openai is guaranteed healthy here).
        let fb = pick_fallback("nonsense-provider", &config);
        assert!(fb.is_some());
    }

    #[test]
    fn tcp_reachable_rejects_bad_urls() {
        assert!(!tcp_reachable("not-a-url", Duration::from_millis(50)));
        assert!(!tcp_reachable("ftp://x", Duration::from_millis(50)));
        assert!(!tcp_reachable("http://", Duration::from_millis(50)));
    }

    #[test]
    fn guidance_mentions_all_setup_paths() {
        let g = no_provider_guidance();
        assert!(g.contains("claude"));
        assert!(g.contains("ollama"));
        assert!(g.contains("OPENAI_API_KEY"));
        assert!(g.contains("OPENROUTER_API_KEY"));
    }
}
