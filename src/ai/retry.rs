use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
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
                // Sanitise before logging: upstream error messages can
                // carry Bearer tokens / API keys from the request that
                // we forwarded, and tracing output may be persisted.
                let sanitized = redact_credentials(&err.to_string());
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %sanitized,
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

/// Whether an error is a provider *quota / session limit* exhaustion (as opposed
/// to a transient blip). These do NOT clear on immediate retry — they clear when
/// the provider's usage window resets — so the worker DEFERS the job to run later
/// rather than retrying it now or failing it outright. Kept narrow to avoid
/// deferring a genuine failure forever.
///
/// Matches the Claude CLI shape ("You've hit your session limit · resets …") and
/// common "usage limit"/"quota exceeded" wordings.
pub fn is_quota_error(err: &anyhow::Error) -> bool {
    let msg = format!("{err:#}").to_lowercase();
    msg.contains("session limit")
        || msg.contains("usage limit")
        || msg.contains("quota exceeded")
        || msg.contains("quota exhausted")
        || (msg.contains("hit your") && msg.contains("limit"))
}

/// Extract the reset wall-clock + IANA timezone from a quota/session-limit
/// error message (e.g. `"Claude: You've hit your session limit · resets
/// 12:30am (Europe/Berlin)"`) and resolve it to the next UTC instant that
/// clock time occurs at or after `now`.
///
/// Measured cost of NOT doing this: a limit hit at 00:26 with the provider
/// reporting `resets 12:30am` actually cleared at 00:30, but the old blind
/// `QUOTA_BACKOFF_SECS` (20min) deferred the job to ~00:46 — ~16 minutes of
/// idle worker per quota event. The reset time is right there in the string.
///
/// Returns `None` if the message doesn't carry a parseable
/// `"resets <time> (<tz>)"` clause (missing/garbage time, missing or unknown
/// timezone) — the caller falls back to a blind backoff in that case, so
/// it's safe to be strict here rather than guess.
///
/// Pure and deterministic (`now` is a parameter, never read from the clock
/// internally) so callers can test it without timezone-dependent flakiness
/// or depending on the machine's local timezone.
pub fn parse_quota_reset(msg: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    const MARKER: &str = "resets ";
    let after = {
        let idx = msg.find(MARKER)?;
        &msg[idx + MARKER.len()..]
    };

    // The timezone is the parenthesized IANA name right after the time,
    // e.g. "12:30am (Europe/Berlin)".
    let tz_open = after.find('(')?;
    let tz_close = tz_open + after[tz_open..].find(')')?;
    let time_part = after[..tz_open].trim();
    let tz_name = after[tz_open + 1..tz_close].trim();
    let tz: Tz = tz_name.parse().ok()?;

    let (hour, minute) = parse_wall_clock(time_part)?;

    // Resolve "today at <hour>:<minute> in <tz>" first; if that instant is
    // already behind `now`, the provider must mean tomorrow's occurrence.
    let now_local = now.with_timezone(&tz);
    let today = now_local.date_naive();
    let today_naive = today.and_hms_opt(hour, minute, 0)?;
    let today_local = tz.from_local_datetime(&today_naive).single()?;
    let today_utc = today_local.with_timezone(&Utc);

    if today_utc >= now {
        return Some(today_utc);
    }

    let tomorrow = today.succ_opt()?;
    let tomorrow_naive = tomorrow.and_hms_opt(hour, minute, 0)?;
    let tomorrow_local = tz.from_local_datetime(&tomorrow_naive).single()?;
    Some(tomorrow_local.with_timezone(&Utc))
}

/// Parse a 12-hour wall-clock time like `"12:30am"` or `"3pm"` into
/// `(hour, minute)` in 24-hour form. Minutes default to `0` when omitted
/// (the provider's message drops them for on-the-hour resets, e.g. `"3pm"`).
fn parse_wall_clock(s: &str) -> Option<(u32, u32)> {
    let lower = s.to_lowercase();
    let (digits, is_pm) = if let Some(d) = lower.strip_suffix("am") {
        (d, false)
    } else {
        let d = lower.strip_suffix("pm")?;
        (d, true)
    };
    let digits = digits.trim();
    let (hour_str, minute_str) = digits.split_once(':').unwrap_or((digits, "0"));
    let hour: u32 = hour_str.parse().ok()?;
    let minute: u32 = minute_str.parse().ok()?;
    if !(1..=12).contains(&hour) || minute > 59 {
        return None;
    }
    let hour24 = match (hour, is_pm) {
        (12, false) => 0, // 12am -> midnight
        (12, true) => 12, // 12pm -> noon
        (h, false) => h,
        (h, true) => h + 12,
    };
    Some((hour24, minute))
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

/// Redact credentials that might appear in an error string before we
/// write it to logs. Upstream HTTP errors can include request headers
/// (Bearer tokens, API keys) or URL query parameters, and trace output
/// may be persisted by operators — keys must not land there.
///
/// Covers common shapes: `Bearer <token>`, `api_key=...`, `api-key: ...`,
/// `Authorization: ...`, `sk-...`-style keys from OpenAI-family providers.
pub(crate) fn redact_credentials(msg: &str) -> String {
    let mut out = msg.to_string();

    // Values terminated by end-of-line only (HTTP header shape).
    for prefix in [
        "Authorization: ",
        "authorization: ",
        "X-Api-Key: ",
        "x-api-key: ",
    ] {
        redact_after(&mut out, prefix, false, |c| c == '\n' || c == '\r');
    }

    // `Bearer <token>` in freeform text.
    redact_after(&mut out, "Bearer ", false, |c| {
        c.is_whitespace() || matches!(c, '"' | '\'')
    });

    // Query-string / form-style key=value.
    for prefix in ["api_key=", "api-key=", "apikey=", "API_KEY=", "API-KEY="] {
        redact_after(&mut out, prefix, false, |c| {
            matches!(c, ' ' | '&' | '"' | '\'' | ',' | '\n' | '\r')
        });
    }

    // OpenAI/Anthropic-style secret prefixes in freeform text
    // (e.g. an error body that echoes back the provided key). Require a word
    // boundary before "sk-" so it isn't matched inside ordinary words like
    // "Disk-full", "task-123", or "risk-averse".
    redact_after(&mut out, "sk-", true, |c| {
        c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ')' | ']' | '}')
    });

    out
}

/// Redact everything between `needle` and the next char for which
/// `end_of_value` returns true. `needle` itself is kept (so the log
/// still carries context: "Bearer [REDACTED]"), but the secret portion
/// is replaced with `[REDACTED]`. When `boundary_before` is set, `needle`
/// only matches when preceded by start-of-string or a non-alphanumeric char,
/// so a short prefix (e.g. "sk-") isn't matched mid-word. Char-boundary safe.
fn redact_after(
    buf: &mut String,
    needle: &str,
    boundary_before: bool,
    end_of_value: impl Fn(char) -> bool,
) {
    let mut search_from = 0;
    while let Some(rel) = buf[search_from..].find(needle) {
        let needle_start = search_from + rel;
        if boundary_before
            && buf[..needle_start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
        {
            // Glued to a preceding word char — not a real credential prefix.
            search_from = needle_start + needle.len();
            continue;
        }
        let start = needle_start + needle.len();
        let end_idx = buf[start..]
            .char_indices()
            .find(|(_, c)| end_of_value(*c))
            .map(|(i, _)| start + i)
            .unwrap_or(buf.len());
        if end_idx > start {
            buf.replace_range(start..end_idx, "[REDACTED]");
            search_from = start + "[REDACTED]".len();
        } else {
            search_from = start;
        }
        if search_from >= buf.len() {
            break;
        }
    }
}

/// Inner helper that operates on the stringified error message.
fn is_retryable_message(msg: &str) -> bool {
    // HTTP status codes embedded in error messages (e.g. "API error (429)").
    // 529 is Anthropic's "overloaded" status — transient, worth retrying.
    let retryable_statuses = ["408", "429", "500", "502", "503", "504", "529"];
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
        "overloaded", // Anthropic transient overload
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
    fn retryable_408() {
        assert!(is_retryable_message("API error (408): request timeout"));
    }

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
    fn quota_error_matches_session_limit_but_not_transient() {
        let quota = anyhow::anyhow!(
            "Claude: You've hit your session limit · resets 11:20pm (Europe/Berlin)"
        );
        assert!(is_quota_error(&quota));
        assert!(is_quota_error(&anyhow::anyhow!("usage limit reached")));
        // A transient/server error is NOT a quota error (it should retry, not defer).
        assert!(!is_quota_error(&anyhow::anyhow!(
            "API error (503): unavailable"
        )));
        assert!(!is_quota_error(&anyhow::anyhow!("connection reset")));
    }

    // ── parse_quota_reset ────────────────────────────────────────────────
    // All expected values are constructed as explicit UTC instants (never
    // derived from the test machine's local timezone), matching the fixed
    // historical UTC offset for the chosen date — Europe/Berlin is CET
    // (UTC+1) and America/New_York is EST (UTC-5) in mid-January, no DST.

    #[test]
    fn parse_quota_reset_am_case_rolls_to_tomorrow_when_already_passed() {
        // now = 2026-01-14 22:00 UTC = 23:00 Berlin (CET, UTC+1). Today's
        // 00:30 local already passed (it was 23 hours ago), so the next
        // occurrence is tomorrow 2026-01-15 00:30 CET = 2026-01-14 23:30 UTC.
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 22, 0, 0).unwrap();
        let msg = "Claude: You've hit your session limit · resets 12:30am (Europe/Berlin)";
        let expected = Utc.with_ymd_and_hms(2026, 1, 14, 23, 30, 0).unwrap();
        assert_eq!(parse_quota_reset(msg, now), Some(expected));
    }

    #[test]
    fn parse_quota_reset_pm_case_same_day() {
        // now = 2026-01-14 14:00 UTC = 09:00 New York (EST, UTC-5). Today's
        // 15:00 local hasn't happened yet, so it resolves to today:
        // 2026-01-14 15:00 EST = 2026-01-14 20:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 14, 0, 0).unwrap();
        let msg = "resets 3pm (America/New_York)";
        let expected = Utc.with_ymd_and_hms(2026, 1, 14, 20, 0, 0).unwrap();
        assert_eq!(parse_quota_reset(msg, now), Some(expected));
    }

    #[test]
    fn parse_quota_reset_falls_back_to_tomorrow_when_today_passed() {
        // now = 2026-01-14 20:00 UTC = 21:00 Berlin (CET). Today's 15:00
        // local already passed, so it rolls to tomorrow:
        // 2026-01-15 15:00 CET = 2026-01-15 14:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 20, 0, 0).unwrap();
        let msg = "resets 3pm (Europe/Berlin)";
        let expected = Utc.with_ymd_and_hms(2026, 1, 15, 14, 0, 0).unwrap();
        assert_eq!(parse_quota_reset(msg, now), Some(expected));
    }

    #[test]
    fn parse_quota_reset_none_when_timezone_missing() {
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 20, 0, 0).unwrap();
        assert_eq!(parse_quota_reset("resets 3pm", now), None);
    }

    #[test]
    fn parse_quota_reset_none_when_timezone_unknown() {
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 20, 0, 0).unwrap();
        assert_eq!(parse_quota_reset("resets 3pm (Mars/Colony)", now), None);
    }

    #[test]
    fn parse_quota_reset_none_when_no_resets_clause() {
        let now = Utc.with_ymd_and_hms(2026, 1, 14, 20, 0, 0).unwrap();
        assert_eq!(parse_quota_reset("API error (503): unavailable", now), None);
    }

    #[test]
    fn retryable_anthropic_overloaded_529() {
        // Anthropic's overloaded status and its textual form are transient.
        assert!(is_retryable_message("API error (529): overloaded_error"));
        assert!(is_retryable_message(
            "claude exited: Overloaded, please retry"
        ));
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

    // ── is_retryable edge cases ─────────────────────────────────────

    #[test]
    fn non_retryable_wins_over_retryable_in_same_message() {
        // Message contains both 400 (non-retryable) and 429 (retryable).
        // Non-retryable is checked first, so should return false.
        assert!(!is_retryable_message(
            "error (400): bad request, then (429): rate limited"
        ));
    }

    #[test]
    fn retryable_found_when_no_non_retryable_present() {
        assert!(is_retryable_message("API error (429): rate limited"));
        assert!(is_retryable_message("API error (503): service unavailable"));
    }

    #[test]
    fn retryable_connection_refused_case_insensitive() {
        assert!(is_retryable_message("connection refused"));
        assert!(is_retryable_message("Connection refused by server"));
    }

    #[test]
    fn retryable_timeout_variations() {
        assert!(is_retryable_message("request timed out"));
        assert!(is_retryable_message("connection timeout"));
        assert!(is_retryable_message("operation timed out"));
    }

    #[test]
    fn non_retryable_auth_errors() {
        assert!(!is_retryable_message("API error (401): unauthorized"));
        assert!(!is_retryable_message("API error (403): forbidden"));
    }

    #[test]
    fn non_retryable_not_found() {
        assert!(!is_retryable_message("API error (404): not found"));
    }

    #[test]
    fn unknown_error_not_retryable() {
        assert!(!is_retryable_message("some random error with no status"));
    }

    // ── delay_with_jitter boundary ──────────────────────────────────

    #[test]
    fn delay_with_jitter_never_exceeds_max_at_boundary() {
        let cfg = RetryConfig {
            max_delay_ms: 1000,
            initial_delay_ms: 1000,
            ..RetryConfig::default()
        };
        // At max, jitter would try to add up to 250ms, but result is clamped.
        // Run multiple times to catch jitter variance.
        for _ in 0..20 {
            let d = cfg.delay_with_jitter(10);
            assert!(
                d.as_millis() as u64 <= cfg.max_delay_ms,
                "delay {} should not exceed max {}",
                d.as_millis(),
                cfg.max_delay_ms
            );
        }
    }

    #[test]
    fn delay_with_jitter_zero_initial() {
        let cfg = RetryConfig {
            initial_delay_ms: 0,
            ..RetryConfig::default()
        };
        let d = cfg.delay_with_jitter(0);
        assert_eq!(d.as_millis(), 0);
    }

    // ── redact_credentials ────────────────────────────────────────────

    #[test]
    fn redact_bearer_token() {
        let r = redact_credentials("auth failed: Bearer sk-abc123def456 expired");
        assert!(!r.contains("sk-abc123def456"));
        assert!(r.contains("Bearer [REDACTED]"));
        assert!(r.contains("expired"));
    }

    #[test]
    fn redact_authorization_header() {
        let r = redact_credentials("HTTP 401\nAuthorization: Bearer xyz789\nDate: today");
        assert!(!r.contains("xyz789"));
        assert!(r.contains("Authorization: [REDACTED]"));
        assert!(r.contains("Date: today"));
    }

    #[test]
    fn redact_x_api_key_header() {
        let r = redact_credentials("X-Api-Key: super-secret-value\nfoo");
        assert!(!r.contains("super-secret-value"));
        assert!(r.contains("X-Api-Key: [REDACTED]"));
    }

    #[test]
    fn redact_query_string_api_key() {
        let r = redact_credentials("https://api.example.com/v1?api_key=topsecret&model=gpt-4");
        assert!(!r.contains("topsecret"));
        assert!(r.contains("api_key=[REDACTED]"));
        assert!(r.contains("model=gpt-4"));
    }

    #[test]
    fn redact_openai_style_sk_key() {
        let r = redact_credentials("Provider error: invalid key sk-proj-abc123xyz789");
        assert!(!r.contains("sk-proj-abc123xyz789"));
        assert!(r.contains("sk-[REDACTED]"));
    }

    #[test]
    fn redact_no_credentials_is_noop() {
        let plain = "API error (500): internal server error";
        let r = redact_credentials(plain);
        assert_eq!(r, plain);
    }

    #[test]
    fn redact_does_not_corrupt_sk_substring_inside_words() {
        // "sk-" appears inside ordinary words — must NOT be redacted.
        for msg in [
            "Disk-full error while writing to /tmp",
            "task-123 failed and risk-averse retry disabled",
            "ask-me-later prompt was dismissed",
        ] {
            assert_eq!(redact_credentials(msg), msg, "corrupted: {msg}");
        }
    }

    #[test]
    fn redact_still_catches_boundary_sk_key() {
        // A real key at a boundary is still redacted...
        let r = redact_credentials("key: sk-abc123 and (sk-def456) plus \"sk-ghi789\"");
        assert!(!r.contains("abc123"));
        assert!(!r.contains("def456"));
        assert!(!r.contains("ghi789"));
        // ...even when it follows a non-alphanumeric boundary like '(' or '"'.
        assert!(r.contains("sk-[REDACTED]"));
        // But the word "Disk-" in the same string would be untouched.
        let r2 = redact_credentials("Disk-full then sk-secret999");
        assert!(r2.starts_with("Disk-full"));
        assert!(!r2.contains("secret999"));
    }

    #[test]
    fn redact_handles_multiple_secrets_in_one_message() {
        let r = redact_credentials("Bearer xyz123 then api_key=foobar and sk-ant-secretvalue");
        assert!(!r.contains("xyz123"));
        assert!(!r.contains("foobar"));
        assert!(!r.contains("secretvalue"));
        // All three placeholders should be present.
        assert_eq!(r.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn redact_empty_string() {
        assert_eq!(redact_credentials(""), "");
    }

    #[test]
    fn redact_handles_multibyte_chars_around_secret() {
        // Make sure the char-boundary scan in redact_after doesn't trip
        // on multi-byte chars surrounding a secret.
        let r = redact_credentials("\u{1F511} Bearer abc123 \u{1F600} done");
        assert!(!r.contains("abc123"));
        assert!(r.contains("[REDACTED]"));
        assert!(r.contains("\u{1F511}"));
        assert!(r.contains("\u{1F600}"));
    }

    #[test]
    fn redact_does_not_loop_forever_on_bare_prefix() {
        // `Bearer ` followed immediately by a terminator (no value to
        // hide) must not cause an infinite loop in redact_after.
        let r = redact_credentials("Bearer \nnext line");
        // The function returned, that's the assertion. Result content
        // here is whatever — what matters is termination.
        assert!(r.contains("next line"));
    }
}
