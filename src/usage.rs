use serde::{Deserialize, Serialize};

/// Token pricing per 1M tokens (approximate, input/output averaged).
///
/// These are rough estimates — actual costs vary by input vs output pricing
/// and are subject to change. Free/subscription providers always return 0.0.
pub fn cost_per_million_tokens(provider: &str, model: &str) -> f64 {
    match (provider, model) {
        // Claude via Claude Code — included in subscription, no per-token cost
        ("claude_code" | "claude", _) => 0.0,
        // OpenAI pricing (approximate averages of input+output)
        ("openai", "gpt-4o") => 7.50,      // ~$2.50 in + $10 out averaged
        ("openai", "gpt-4o-mini") => 0.30,  // ~$0.15 in + $0.60 out averaged
        ("openai", _) => 5.00,              // Default estimate
        // OpenRouter — varies wildly, use conservative estimate
        ("openrouter", m) if m.contains("claude") => 6.00,
        ("openrouter", m) if m.contains("gpt-4o") => 7.50,
        ("openrouter", m) if m.contains("llama") => 0.20,
        ("openrouter", _) => 3.00, // Conservative default
        // Ollama — free (local)
        ("ollama", _) => 0.0,
        // Copilot — included in subscription
        ("copilot", _) => 0.0,
        _ => 0.0,
    }
}

/// Running usage stats for the current session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    pub total_tokens_sent: usize,
    pub total_tokens_received: usize,
    pub total_requests: usize,
    pub estimated_cost_usd: f64,
    pub session_start: Option<String>, // ISO timestamp
}

impl UsageStats {
    pub fn new() -> Self {
        Self {
            session_start: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }

    /// Record a request and update costs.
    pub fn record_request(
        &mut self,
        tokens_sent: usize,
        tokens_received: usize,
        provider: &str,
        model: &str,
    ) {
        self.total_tokens_sent += tokens_sent;
        self.total_tokens_received += tokens_received;
        self.total_requests += 1;

        let total_tokens = tokens_sent + tokens_received;
        let cost =
            cost_per_million_tokens(provider, model) * total_tokens as f64 / 1_000_000.0;
        self.estimated_cost_usd += cost;
    }

    /// Format cost as human-readable string.
    pub fn format_cost(&self) -> String {
        if self.estimated_cost_usd == 0.0 {
            "Free (subscription/local)".into()
        } else if self.estimated_cost_usd < 0.01 {
            "<$0.01".into()
        } else {
            format!("${:.2}", self.estimated_cost_usd)
        }
    }
}

/// Session spending limit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingLimit {
    pub max_cost_usd: f64,
    pub max_requests: Option<usize>,
    pub max_tokens: Option<usize>,
    pub enabled: bool,
}

impl Default for SpendingLimit {
    fn default() -> Self {
        Self {
            max_cost_usd: 5.0, // $5 default limit per session
            max_requests: None,
            max_tokens: None,
            enabled: false, // Disabled by default
        }
    }
}

impl SpendingLimit {
    /// Check if a request would exceed the limit. Returns a warning message if so.
    pub fn would_exceed(
        &self,
        stats: &UsageStats,
        estimated_new_tokens: usize,
        provider: &str,
        model: &str,
    ) -> Option<String> {
        if !self.enabled {
            return None;
        }

        // Cost check
        let new_cost = cost_per_million_tokens(provider, model) * estimated_new_tokens as f64
            / 1_000_000.0;
        if stats.estimated_cost_usd + new_cost > self.max_cost_usd {
            return Some(format!(
                "Spending limit reached: ${:.2} / ${:.2}. Use /limit set <amount> or /limit off",
                stats.estimated_cost_usd, self.max_cost_usd
            ));
        }

        // Request count check
        if let Some(max) = self.max_requests {
            if stats.total_requests >= max {
                return Some(format!(
                    "Request limit reached: {}/{}",
                    stats.total_requests, max
                ));
            }
        }

        // Token check
        if let Some(max) = self.max_tokens {
            let total = stats.total_tokens_sent + stats.total_tokens_received;
            if total >= max {
                return Some(format!("Token limit reached: {}/{}", total, max));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_free_providers() {
        assert_eq!(cost_per_million_tokens("claude_code", "sonnet"), 0.0);
        assert_eq!(cost_per_million_tokens("ollama", "llama3"), 0.0);
        assert_eq!(cost_per_million_tokens("copilot", "copilot"), 0.0);
    }

    #[test]
    fn cost_paid_providers() {
        assert!(cost_per_million_tokens("openai", "gpt-4o") > 0.0);
        assert!(cost_per_million_tokens("openrouter", "gpt-4o") > 0.0);
    }

    #[test]
    fn record_request_updates_stats() {
        let mut stats = UsageStats::new();
        stats.record_request(1000, 500, "openai", "gpt-4o");
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_tokens_sent, 1000);
        assert_eq!(stats.total_tokens_received, 500);
        assert!(stats.estimated_cost_usd > 0.0);
    }

    #[test]
    fn spending_limit_disabled_allows_all() {
        let limit = SpendingLimit::default(); // disabled
        let stats = UsageStats::new();
        assert!(limit
            .would_exceed(&stats, 1_000_000, "openai", "gpt-4o")
            .is_none());
    }

    #[test]
    fn spending_limit_blocks_when_exceeded() {
        let limit = SpendingLimit {
            max_cost_usd: 0.01,
            enabled: true,
            ..Default::default()
        };
        let mut stats = UsageStats::new();
        stats.record_request(100_000, 50_000, "openai", "gpt-4o");
        assert!(limit
            .would_exceed(&stats, 100_000, "openai", "gpt-4o")
            .is_some());
    }

    #[test]
    fn format_cost_free() {
        let stats = UsageStats::new();
        assert_eq!(stats.format_cost(), "Free (subscription/local)");
    }

    #[test]
    fn format_cost_small() {
        let mut stats = UsageStats::new();
        stats.estimated_cost_usd = 0.005;
        assert_eq!(stats.format_cost(), "<$0.01");
    }

    #[test]
    fn format_cost_normal() {
        let mut stats = UsageStats::new();
        stats.estimated_cost_usd = 1.23;
        assert_eq!(stats.format_cost(), "$1.23");
    }

    #[test]
    fn record_multiple_requests_accumulates() {
        let mut stats = UsageStats::new();
        stats.record_request(1000, 500, "openai", "gpt-4o");
        stats.record_request(2000, 1000, "openai", "gpt-4o");
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_tokens_sent, 3000);
        assert_eq!(stats.total_tokens_received, 1500);
    }

    #[test]
    fn spending_limit_request_count() {
        let limit = SpendingLimit {
            max_cost_usd: 100.0,
            max_requests: Some(5),
            max_tokens: None,
            enabled: true,
        };
        let mut stats = UsageStats::new();
        for _ in 0..5 {
            stats.record_request(100, 50, "ollama", "llama3");
        }
        assert!(limit.would_exceed(&stats, 100, "ollama", "llama3").is_some());
    }

    #[test]
    fn spending_limit_token_count() {
        let limit = SpendingLimit {
            max_cost_usd: 100.0,
            max_requests: None,
            max_tokens: Some(1000),
            enabled: true,
        };
        let mut stats = UsageStats::new();
        stats.record_request(600, 500, "ollama", "llama3");
        assert!(limit.would_exceed(&stats, 100, "ollama", "llama3").is_some());
    }

    #[test]
    fn cost_openrouter_claude_model() {
        let cost = cost_per_million_tokens("openrouter", "anthropic/claude-3.5-sonnet");
        assert!(cost > 0.0);
    }

    #[test]
    fn cost_openrouter_llama_model() {
        let cost = cost_per_million_tokens("openrouter", "meta-llama/llama-3-70b");
        assert!(cost > 0.0);
        assert!(cost < cost_per_million_tokens("openrouter", "gpt-4o"));
    }
}
