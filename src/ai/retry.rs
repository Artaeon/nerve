use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for API request retry behavior with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Initial delay in milliseconds before the first retry.
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds between retries.
    pub max_delay_ms: u64,
    /// Multiplier applied to the delay after each retry attempt.
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30_000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate the delay for a given attempt (0-indexed) without jitter.
    pub fn base_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay_ms as f64 * self.backoff_factor.powi(attempt as i32);
        let clamped_ms = delay_ms.min(self.max_delay_ms as f64) as u64;
        Duration::from_millis(clamped_ms)
    }

    /// Calculate the delay for a given attempt (0-indexed) with random jitter.
    ///
    /// Jitter adds up to 25% of the base delay to avoid thundering-herd
    /// problems when multiple clients retry simultaneously.
    pub fn delay_with_jitter(&self, attempt: u32) -> Duration {
        let base = self.base_delay(attempt);
        let jitter_range_ms = base.as_millis() as u64 / 4; // up to 25%
        let jitter_ms = if jitter_range_ms > 0 {
            fastrand::u64(0..=jitter_range_ms)
        } else {
            0
        };
        let total_ms = base.as_millis() as u64 + jitter_ms;
        let clamped = total_ms.min(self.max_delay_ms);
        Duration::from_millis(clamped)
    }
}

/// Retry an async operation using exponential backoff.
///
/// The closure `f` is called up to `config.max_retries + 1` times (the
/// initial attempt plus retries).  Between each retry the task sleeps for
/// an exponentially-increasing duration with jitter.
///
/// The caller provides `should_retry` to decide whether a given error is
/// transient and worth retrying.  If `should_retry` returns `false` the
/// error is returned immediately.
pub async fn retry_async<F, Fut, T, E>(
    config: &RetryConfig,
    should_retry: fn(&E) -> bool,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_err: Option<E> = None;

    for attempt in 0..=config.max_retries {
        match f().await {
            Ok(val) => return Ok(val),
            Err(err) => {
                if attempt == config.max_retries || !should_retry(&err) {
                    return Err(err);
                }

                let delay = config.delay_with_jitter(attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "Retrying after transient error"
                );

                tokio::time::sleep(delay).await;
                last_err = Some(err);
            }
        }
    }

    // Should be unreachable, but satisfy the compiler.
    Err(last_err.expect("retry loop should have run at least once"))
}

/// Check whether an `anyhow::Error` represents a transient failure that is
/// worth retrying.
///
/// Retryable conditions:
/// - HTTP 429 (Too Many Requests)
/// - HTTP 500, 502, 503, 504 (server errors)
/// - Connection refused / reset / timeout strings
///
/// Non-retryable:
/// - HTTP 400, 401, 403, 404 (client errors)
/// - Parse / deserialization errors
/// - Any other unknown error
pub fn is_retryable_error(err: &anyhow::Error) -> bool {
    let msg = format!("{err:#}");
    is_retryable_message(&msg)
}

/// Inner helper that operates on the stringified error message.
fn is_retryable_message(msg: &str) -> bool {
    // HTTP status codes embedded in error messages (e.g. "API error (429)")
    let retryable_statuses = ["429", "500", "502", "503", "504"];
    let non_retryable_statuses = ["400", "401", "403", "404"];

    // Check for non-retryable status codes first (more specific match)
    for status in &non_retryable_statuses {
        if msg.contains(&format!("({status})"))
            || msg.contains(&format!("status: {status}"))
            || msg.contains(&format!("status code: {status}"))
        {
            return false;
        }
    }

    // Check for retryable status codes
    for status in &retryable_statuses {
        if msg.contains(&format!("({status})"))
            || msg.contains(&format!("status: {status}"))
            || msg.contains(&format!("status code: {status}"))
        {
            return true;
        }
    }

    // Connection-level errors
    let connection_errors = [
        "connection refused",
        "connection reset",
        "connection closed",
        "connection timed out",
        "timed out",
        "timeout",
        "broken pipe",
        "network unreachable",
        "dns error",
        "resolve error",
        "no route to host",
    ];

    let lower = msg.to_lowercase();
    for pattern in &connection_errors {
        if lower.contains(pattern) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RetryConfig::default() ──────────────────────────────────────────

    #[test]
    fn default_max_retries() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
    }

    #[test]
    fn default_initial_delay_ms() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.initial_delay_ms, 1000);
    }

    #[test]
    fn default_max_delay_ms() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_delay_ms, 30_000);
    }

    #[test]
    fn default_backoff_factor() {
        let cfg = RetryConfig::default();
        assert!((cfg.backoff_factor - 2.0).abs() < f64::EPSILON);
    }

    // ── Backoff delay calculation ───────────────────────────────────────

    #[test]
    fn base_delay_attempt_0() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.base_delay(0), Duration::from_millis(1000));
    }

    #[test]
    fn base_delay_attempt_1() {
        let cfg = RetryConfig::default();
        // 1000 * 2^1 = 2000
        assert_eq!(cfg.base_delay(1), Duration::from_millis(2000));
    }

    #[test]
    fn base_delay_attempt_2() {
        let cfg = RetryConfig::default();
        // 1000 * 2^2 = 4000
        assert_eq!(cfg.base_delay(2), Duration::from_millis(4000));
    }

    #[test]
    fn base_delay_attempt_3() {
        let cfg = RetryConfig::default();
        // 1000 * 2^3 = 8000
        assert_eq!(cfg.base_delay(3), Duration::from_millis(8000));
    }

    #[test]
    fn base_delay_clamped_to_max() {
        let cfg = RetryConfig {
            max_delay_ms: 5000,
            ..RetryConfig::default()
        };
        // 1000 * 2^3 = 8000, clamped to 5000
        assert_eq!(cfg.base_delay(3), Duration::from_millis(5000));
    }

    #[test]
    fn base_delay_exponential_growth() {
        let cfg = RetryConfig::default();
        let d0 = cfg.base_delay(0).as_millis();
        let d1 = cfg.base_delay(1).as_millis();
        let d2 = cfg.base_delay(2).as_millis();
        assert_eq!(d1, d0 * 2);
        assert_eq!(d2, d1 * 2);
    }

    #[test]
    fn delay_with_jitter_at_least_base() {
        let cfg = RetryConfig::default();
        for attempt in 0..4 {
            let base = cfg.base_delay(attempt);
            let jittered = cfg.delay_with_jitter(attempt);
            assert!(jittered >= base, "jittered delay should be >= base delay");
        }
    }

    #[test]
    fn delay_with_jitter_at_most_125_percent_of_base() {
        let cfg = RetryConfig::default();
        for attempt in 0..4 {
            let base = cfg.base_delay(attempt).as_millis() as u64;
            let jittered = cfg.delay_with_jitter(attempt).as_millis() as u64;
            let max_expected = base + base / 4; // 125%
            let max_allowed = max_expected.min(cfg.max_delay_ms);
            assert!(
                jittered <= max_allowed,
                "attempt {attempt}: jittered {jittered}ms > max allowed {max_allowed}ms"
            );
        }
    }

    #[test]
    fn delay_with_jitter_clamped_to_max() {
        let cfg = RetryConfig {
            max_delay_ms: 2000,
            ..RetryConfig::default()
        };
        // Attempt 3 base = 8000, jittered should still be <= 2000
        let delay = cfg.delay_with_jitter(3);
        assert!(delay.as_millis() <= 2000);
    }

    // ── is_retryable_error / is_retryable_message ───────────────────────

    #[test]
    fn retryable_429() {
        assert!(is_retryable_message("API error (429): rate limited"));
    }

    #[test]
    fn retryable_500() {
        assert!(is_retryable_message(
            "API error (500): internal server error"
        ));
    }

    #[test]
    fn retryable_502() {
        assert!(is_retryable_message("API error (502): bad gateway"));
    }

    #[test]
    fn retryable_503() {
        assert!(is_retryable_message("API error (503): service unavailable"));
    }

    #[test]
    fn retryable_504() {
        assert!(is_retryable_message("API error (504): gateway timeout"));
    }

    #[test]
    fn not_retryable_400() {
        assert!(!is_retryable_message("API error (400): bad request"));
    }

    #[test]
    fn not_retryable_401() {
        assert!(!is_retryable_message("API error (401): unauthorized"));
    }

    #[test]
    fn not_retryable_403() {
        assert!(!is_retryable_message("API error (403): forbidden"));
    }

    #[test]
    fn not_retryable_404() {
        assert!(!is_retryable_message("API error (404): not found"));
    }

    #[test]
    fn retryable_connection_refused() {
        assert!(is_retryable_message("connection refused"));
    }

    #[test]
    fn retryable_connection_reset() {
        assert!(is_retryable_message("Connection reset by peer"));
    }

    #[test]
    fn retryable_timeout() {
        assert!(is_retryable_message("request timed out"));
    }

    #[test]
    fn retryable_dns_error() {
        assert!(is_retryable_message("DNS error: name resolution failed"));
    }

    #[test]
    fn not_retryable_unknown_error() {
        assert!(!is_retryable_message("failed to parse JSON response"));
    }

    #[test]
    fn retryable_status_code_format() {
        // reqwest-style status messages
        assert!(is_retryable_message("status: 429"));
        assert!(is_retryable_message("status code: 503"));
    }

    #[test]
    fn is_retryable_error_with_anyhow() {
        let err = anyhow::anyhow!("API error (429): too many requests");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("API error (401): unauthorized");
        assert!(!is_retryable_error(&err));
    }

    // ── retry_async ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn retry_immediate_success() {
        let cfg = RetryConfig::default();
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        let result: Result<&str, String> = retry_async(
            &cfg,
            |_: &String| true,
            || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok("success")
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_exhausts_all_attempts() {
        let cfg = RetryConfig {
            max_retries: 2,
            initial_delay_ms: 1, // tiny delays for fast tests
            max_delay_ms: 10,
            backoff_factor: 2.0,
        };
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        let result: Result<(), String> = retry_async(
            &cfg,
            |_: &String| true,
            || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err("always fails".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "always fails");
        // 1 initial + 2 retries = 3 total
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let cfg = RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1,
            max_delay_ms: 10,
            backoff_factor: 2.0,
        };
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        let result: Result<&str, String> = retry_async(
            &cfg,
            |_: &String| true,
            || {
                let counter = counter.clone();
                async move {
                    let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if n == 0 {
                        Err("transient failure".to_string())
                    } else {
                        Ok("recovered")
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_stops_on_non_retryable_error() {
        let cfg = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 1,
            max_delay_ms: 10,
            backoff_factor: 2.0,
        };
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        let result: Result<(), String> = retry_async(
            &cfg,
            |err: &String| err.contains("transient"),
            || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err("permanent failure".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        // Should only call once — not retryable
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_zero_retries_means_single_attempt() {
        let cfg = RetryConfig {
            max_retries: 0,
            initial_delay_ms: 1,
            max_delay_ms: 10,
            backoff_factor: 2.0,
        };
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        let result: Result<(), String> = retry_async(
            &cfg,
            |_: &String| true,
            || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err("fail".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    // ── RetryConfig serde ───────────────────────────────────────────────

    #[test]
    fn retry_config_toml_roundtrip() {
        let cfg = RetryConfig::default();
        let toml_str = toml::to_string(&cfg).unwrap();
        let restored: RetryConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.max_retries, cfg.max_retries);
        assert_eq!(restored.initial_delay_ms, cfg.initial_delay_ms);
        assert_eq!(restored.max_delay_ms, cfg.max_delay_ms);
        assert!((restored.backoff_factor - cfg.backoff_factor).abs() < f64::EPSILON);
    }

    #[test]
    fn retry_config_deserializes_from_empty_toml() {
        let cfg: RetryConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.initial_delay_ms, 1000);
    }

    #[test]
    fn retry_config_partial_toml() {
        let cfg: RetryConfig = toml::from_str("max_retries = 5").unwrap();
        assert_eq!(cfg.max_retries, 5);
        // Rest should be defaults
        assert_eq!(cfg.initial_delay_ms, 1000);
        assert_eq!(cfg.max_delay_ms, 30_000);
    }
}
