use std::fmt::Write as _;

/// Manages conversation context to stay within token limits
pub struct ContextManager {
    max_tokens: usize,
}

/// Truncate text intelligently at a sentence boundary when possible.
///
/// `max_chars` is measured in characters, not bytes, and truncation always
/// happens on a character boundary so multi-byte UTF-8 (CJK, emoji, accents)
/// never panics.
pub fn smart_truncate(text: &str, max_chars: usize) -> String {
    // Fast path: short strings (by char count) are returned verbatim.
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    // Take up to `max_chars` characters; the result is always valid UTF-8.
    let truncated: String = text.chars().take(max_chars).collect();

    // Try to break at the last sentence-ending punctuation within the range —
    // but only if doing so retains a reasonable fraction of the window.
    // Otherwise a '.' inside a filename / version / abbreviation near the start
    // (e.g. "see config.rs line 5 ...") would throw away most of the message.
    if let Some(pos) = truncated.rfind(['.', '!', '?'])
        && pos >= truncated.len() / 2
    {
        // Include the punctuation character itself.
        return truncated[..=pos].to_string();
    }

    // Fall back to a word boundary.
    if let Some(pos) = truncated.rfind(' ') {
        return format!("{}...", &truncated[..pos]);
    }

    // Last resort: hard cut.
    format!("{truncated}...")
}

impl ContextManager {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    /// Estimate token count for a message.
    ///
    /// Uses ~3 *characters* per token, which is conservative — BPE tokenization
    /// averages 3-4 chars/token for English, less for code/CJK. Counting
    /// characters (not bytes) keeps the estimate honest for multi-byte scripts:
    /// a 3-byte CJK glyph is one character, not three, so byte-based counting
    /// used to overestimate CJK/emoji by ~3x. Erring slightly high still
    /// prevents context-limit overflows.
    pub fn estimate_tokens(text: &str) -> usize {
        text.chars().count() / 3 + 1
    }

    /// Estimate total tokens for a conversation
    pub fn conversation_tokens(messages: &[(String, String)]) -> usize {
        messages
            .iter()
            .map(|(_, content)| Self::estimate_tokens(content))
            .sum()
    }

    /// Return a recommended context token limit for a given provider name.
    /// If the user has set `context_limit` in config, that takes precedence.
    pub fn recommended_limit(provider: &str) -> usize {
        match provider {
            "claude_code" | "claude" => 200_000, // Claude has huge context
            "openai" => 60_000,                  // GPT-4o has 128K but we want headroom
            "openrouter" => 30_000,              // Be conservative — depends on model
            "ollama" => 32_000,                  // Many local models support 32K+ now
            _ => 30_000,
        }
    }

    /// Return the effective context limit, respecting a user override.
    pub fn effective_limit(provider: &str, user_override: Option<usize>) -> usize {
        user_override.unwrap_or_else(|| Self::recommended_limit(provider))
    }

    /// Check whether the conversation exceeds the token budget.
    #[allow(dead_code)]
    pub fn is_over_budget(&self, messages: &[(String, String)]) -> bool {
        Self::conversation_tokens(messages) > self.max_tokens
    }

    /// Calculate remaining tokens available in the context window.
    #[allow(dead_code)]
    pub fn remaining_tokens(&self, messages: &[(String, String)]) -> usize {
        let used = Self::conversation_tokens(messages);
        self.max_tokens.saturating_sub(used)
    }

    /// Compact tool results in a conversation: keep recent results intact,
    /// but shorten older tool outputs to save context space.
    pub fn compact_tool_results(&self, messages: &[(String, String)]) -> Vec<(String, String)> {
        let mut result = Vec::new();
        let len = messages.len();

        for (i, (role, content)) in messages.iter().enumerate() {
            // Keep the last 6 messages as-is (recent context matters)
            if i >= len.saturating_sub(6) {
                result.push((role.clone(), content.clone()));
                continue;
            }

            // Compact old tool results
            if role == "user" && content.starts_with("Tool execution results:") {
                let tool_count = content.matches("<tool_result>").count();
                let brief =
                    format!("[Previous tool execution: {tool_count} tool(s) ran successfully]");
                result.push((role.clone(), brief));
            } else if role == "system"
                && (content.starts_with("File:")
                    || content.starts_with("Command:")
                    || content.starts_with("Directory listing"))
            {
                // Compact old file/command context
                let first_line: String = content.lines().next().unwrap_or("").to_string();
                result.push((role.clone(), format!("[Context: {first_line}]")));
            } else {
                result.push((role.clone(), content.clone()));
            }
        }

        result
    }

    /// Compact a conversation by summarizing older messages if over token limit.
    /// Returns a new message list with older messages summarized.
    pub fn compact_messages(&self, messages: &[(String, String)]) -> Vec<(String, String)> {
        let total = Self::conversation_tokens(messages);
        if total <= self.max_tokens || messages.len() <= 4 {
            return messages.to_vec();
        }

        let mut result = Vec::new();

        // Preserve only the LEADING system prompt(s) verbatim at the front —
        // the base instructions that shape the whole conversation. System
        // messages injected mid-conversation (File:/Command:/directory-listing
        // context) must NOT be hoisted ahead of earlier turns, and must NOT
        // escape compaction; they stay in chronological position and are
        // summarized like any other old message.
        let lead_system = messages.iter().take_while(|(r, _)| r == "system").count();
        for m in &messages[..lead_system] {
            result.push(m.clone());
        }

        // Remaining messages in chronological order (may include mid-stream
        // system context, which is now treated like any other turn).
        let rest: Vec<&(String, String)> = messages[lead_system..].iter().collect();

        if rest.len() <= 4 {
            result.extend(rest.iter().map(|(r, c)| (r.clone(), c.clone())));
            return result;
        }

        // Summarize older messages, but ALWAYS preserve the original request
        // (the first user turn) verbatim. The interactive loop must never
        // summarize the task away — this mirrors the headless loop's HEAD_KEEP,
        // which pins the task message so a long conversation cannot forget what
        // it was asked to do.
        let keep_count = 4;
        let summarize_region = &rest[..rest.len() - keep_count];
        let to_keep = &rest[rest.len() - keep_count..];

        // Locate the first user turn in the region we are about to summarize —
        // that is the original request.
        let pinned = summarize_region.iter().position(|(r, _)| r == "user");

        if let Some(p) = pinned {
            // Original request, kept verbatim and first (it is the earliest
            // user turn, so this preserves chronological order).
            result.push((rest[p].0.clone(), rest[p].1.clone()));

            // Summarize everything else in the region (excluding the pinned
            // request) so no other detail is silently dropped.
            let others: Vec<&(String, String)> = summarize_region
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != p)
                .map(|(_, m)| *m)
                .collect();
            if !others.is_empty() {
                let summary = Self::summarize_messages(&others);
                result.push((
                    "system".into(),
                    format!("Previous conversation summary:\n{summary}"),
                ));
            }
        } else {
            // No user turn in the region (unusual) — summarize it all.
            let summary = Self::summarize_messages(summarize_region);
            result.push((
                "system".into(),
                format!("Previous conversation summary:\n{summary}"),
            ));
        }

        // Add recent messages
        for (role, content) in to_keep {
            result.push((role.to_string(), content.to_string()));
        }

        result
    }

    /// Create a brief, informative summary of messages.
    fn summarize_messages(messages: &[&(String, String)]) -> String {
        let mut summary = String::new();
        let mut exchange_count = 0;

        for (role, content) in messages {
            match role.as_str() {
                "user" => {
                    exchange_count += 1;
                    // Extract the core question/request
                    let brief = smart_truncate(content, 150);
                    let _ = writeln!(summary, "- User: {brief}");
                }
                "assistant" => {
                    // For code responses, just note what was done
                    if content.contains("```") {
                        let code_blocks = content.matches("```").count() / 2;
                        let first_line: String = content
                            .lines()
                            .find(|l| !l.trim().is_empty() && !l.starts_with("```"))
                            .unwrap_or("")
                            .chars()
                            .take(100)
                            .collect();
                        let _ = writeln!(
                            summary,
                            "  AI: {first_line}... [{code_blocks} code block(s)]"
                        );
                    } else {
                        let brief = smart_truncate(content, 150);
                        let _ = writeln!(summary, "  AI: {brief}");
                    }
                }
                "system" => {
                    // Mid-stream injected context (File:/Command:/directory
                    // listings). Summarize it too so it isn't silently lost.
                    let brief = smart_truncate(content, 150);
                    let _ = writeln!(summary, "- Context: {brief}");
                }
                _ => {}
            }
        }

        if exchange_count == 0 {
            return "No prior exchanges.".into();
        }

        format!("[Summary of {exchange_count} previous exchange(s)]\n{summary}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(ContextManager::estimate_tokens(""), 1);
    }

    #[test]
    fn estimate_tokens_short() {
        // "hello" = 5 chars / 4 + 1 = 2
        assert_eq!(ContextManager::estimate_tokens("hello"), 2);
    }

    #[test]
    fn compact_short_conversation_unchanged() {
        let cm = ContextManager::new(10000);
        let messages = vec![
            ("user".into(), "hello".into()),
            ("assistant".into(), "hi".into()),
        ];
        let compacted = cm.compact_messages(&messages);
        assert_eq!(compacted.len(), 2);
    }

    #[test]
    fn compact_long_conversation_summarizes() {
        let cm = ContextManager::new(100); // Very low limit
        let mut messages = Vec::new();
        for i in 0..20 {
            messages.push((
                "user".into(),
                format!("Question number {i} with some extra text to use tokens"),
            ));
            messages.push((
                "assistant".into(),
                format!("Answer number {i} with detailed response text here"),
            ));
        }
        let compacted = cm.compact_messages(&messages);
        assert!(compacted.len() < messages.len());
        // Should have the pinned original request + system summary + last 4.
        assert!(compacted.len() <= 6);
    }

    #[test]
    fn compact_preserves_system_messages() {
        let cm = ContextManager::new(100);
        let mut messages = vec![("system".into(), "You are helpful.".into())];
        for i in 0..20 {
            messages.push(("user".into(), format!("Q{i} long text")));
            messages.push(("assistant".into(), format!("A{i} long text")));
        }
        let compacted = cm.compact_messages(&messages);
        let system_count = compacted.iter().filter(|(r, _)| r == "system").count();
        assert!(system_count >= 1); // Original system + summary
    }

    #[test]
    fn compact_pins_original_request_verbatim() {
        // The interactive loop must never summarize the task away: the first
        // user turn (the original request) must survive compaction byte-for-byte
        // no matter how long the conversation grows.
        let cm = ContextManager::new(50); // low limit forces summarization
        let original = "Refactor the auth module to use the new SessionStore and \
             keep the /login redirect behavior exactly as it is today.";
        let mut messages = vec![("system".into(), "BASE PROMPT".into())];
        messages.push(("user".into(), original.into()));
        for i in 0..12 {
            messages.push(("assistant".into(), format!("working on step {i} now")));
            messages.push(("user".into(), format!("follow-up question {i} here")));
        }
        let compacted = cm.compact_messages(&messages);
        assert!(compacted.len() < messages.len(), "should have compacted");
        assert!(
            compacted.iter().any(|(r, c)| r == "user" && c == original),
            "original request was summarized away: {compacted:?}"
        );
    }

    #[test]
    fn smart_truncate_short_text_unchanged() {
        assert_eq!(smart_truncate("hello", 100), "hello");
    }

    #[test]
    fn smart_truncate_at_sentence_boundary() {
        let text = "First sentence. Second sentence that goes on and on and on.";
        let result = smart_truncate(text, 20);
        // Should break at the period after "First sentence."
        assert_eq!(result, "First sentence.");
    }

    #[test]
    fn smart_truncate_at_word_boundary() {
        let text = "no period here just words going on and on and on and on";
        let result = smart_truncate(text, 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn smart_truncate_ignores_early_period_in_filename() {
        // A '.' inside a filename near the START must not throw away most of
        // the message — it should keep going to a word boundary instead.
        let text = "config.rs contains the bug we should fix immediately today please";
        let result = smart_truncate(text, 44);
        assert_ne!(result, "config.", "must not truncate at the filename dot");
        assert!(result.starts_with("config.rs contains"));
        assert!(
            result.len() > 20,
            "should retain a reasonable fraction, got {result:?}"
        );
    }

    #[test]
    fn compact_does_not_hoist_or_drop_midstream_system() {
        let cm = ContextManager::new(50); // low limit forces summarization
        let mut messages = vec![("system".into(), "BASE PROMPT".into())];
        for i in 0..8 {
            messages.push((
                "user".into(),
                format!("question {i} with enough text to add up"),
            ));
            messages.push((
                "assistant".into(),
                format!("answer {i} with enough text to add up"),
            ));
        }
        // Inject mid-stream system context early so it lands in the summarized
        // (old) region, after at least one non-system message.
        messages.insert(3, ("system".into(), "File: MIDMARKER.rs context".into()));

        let compacted = cm.compact_messages(&messages);

        // Leading base prompt stays first, verbatim.
        assert_eq!(compacted[0].0, "system");
        assert!(compacted[0].1.contains("BASE PROMPT"));
        // The mid-stream marker must NOT survive as its own verbatim system
        // message (the old behavior hoisted every system message to the front,
        // exempt from compaction). It was in the old region, so it's folded
        // into the summary instead.
        assert!(
            !compacted
                .iter()
                .any(|(r, c)| r == "system" && c == "File: MIDMARKER.rs context"),
            "mid-stream system context survived compaction verbatim (hoisted): {compacted:?}"
        );
        // ...but it must not be dropped either — the summary references it.
        let summarized = compacted
            .iter()
            .any(|(_, c)| c.contains("Previous conversation summary") && c.contains("MIDMARKER"));
        assert!(
            summarized,
            "mid-stream system context was lost during compaction"
        );
    }

    #[test]
    fn compact_keeps_recent_midstream_system_in_chronological_tail() {
        let cm = ContextManager::new(50);
        let mut messages = vec![("system".into(), "BASE PROMPT".into())];
        for i in 0..8 {
            messages.push((
                "user".into(),
                format!("question {i} with enough text to add up"),
            ));
            messages.push((
                "assistant".into(),
                format!("answer {i} with enough text to add up"),
            ));
        }
        // Recent mid-stream system context (within the kept tail).
        messages.push(("system".into(), "File: RECENTMARKER.rs context".into()));
        messages.push(("user".into(), "the latest question here".into()));

        let compacted = cm.compact_messages(&messages);
        assert!(compacted[0].1.contains("BASE PROMPT"));
        let pos = compacted
            .iter()
            .position(|(_, c)| c.contains("RECENTMARKER"))
            .expect("recent mid-stream context must be preserved");
        // Preserved in the recent tail, NOT hoisted to the front (old behavior
        // would have moved it to index 1 right after the base prompt).
        assert!(
            pos > 1,
            "recent mid-stream system context was hoisted to front"
        );
        assert!(
            pos >= compacted.len().saturating_sub(4),
            "recent mid-stream context should stay in the chronological tail"
        );
    }

    #[test]
    fn smart_truncate_no_spaces() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let result = smart_truncate(text, 10);
        assert_eq!(result, "abcdefghij...");
    }

    #[test]
    fn smart_truncate_multibyte_does_not_panic() {
        // Regression: byte-index slicing used to panic when the cut point
        // landed inside a multi-byte codepoint. These must not panic and
        // must always return valid UTF-8.
        let cjk = "あいうえお".repeat(50); // 250 multi-byte chars
        let r = smart_truncate(&cjk, 150);
        assert!(r.chars().count() <= 153); // up to 150 chars + "..."
        assert!(cjk.starts_with(r.trim_end_matches('.')));

        let emoji = "🎉🎊✨".repeat(80);
        let r2 = smart_truncate(&emoji, 100);
        assert!(!r2.is_empty()); // valid, no panic

        // Mixed ASCII + multibyte with the boundary mid-codepoint.
        let mixed = format!("hello {}", "λ".repeat(200));
        let _ = smart_truncate(&mixed, 7); // boundary inside a λ — must not panic
    }

    #[test]
    fn summarize_messages_with_code_blocks() {
        let messages: Vec<(String, String)> = vec![
            ("user".into(), "Write a hello world function".into()),
            (
                "assistant".into(),
                "Here is the function:\n```rust\nfn hello() { println!(\"Hello!\"); }\n```\nDone."
                    .into(),
            ),
        ];
        let refs: Vec<&(String, String)> = messages.iter().collect();
        let summary = ContextManager::summarize_messages(&refs);
        assert!(summary.contains("[Summary of 1 previous exchange(s)]"));
        assert!(summary.contains("1 code block(s)"));
    }

    #[test]
    fn summarize_messages_without_code() {
        let messages: Vec<(String, String)> = vec![
            ("user".into(), "What is Rust?".into()),
            (
                "assistant".into(),
                "Rust is a systems programming language.".into(),
            ),
        ];
        let refs: Vec<&(String, String)> = messages.iter().collect();
        let summary = ContextManager::summarize_messages(&refs);
        assert!(summary.contains("User: What is Rust?"));
        assert!(summary.contains("AI: Rust is a systems programming language."));
    }

    #[test]
    fn summarize_empty_messages() {
        let messages: Vec<&(String, String)> = vec![];
        let summary = ContextManager::summarize_messages(&messages);
        assert_eq!(summary, "No prior exchanges.");
    }

    #[test]
    fn recommended_limit_values() {
        assert_eq!(ContextManager::recommended_limit("claude_code"), 200_000);
        assert_eq!(ContextManager::recommended_limit("claude"), 200_000);
        assert_eq!(ContextManager::recommended_limit("openai"), 60_000);
        assert_eq!(ContextManager::recommended_limit("openrouter"), 30_000);
        assert_eq!(ContextManager::recommended_limit("ollama"), 32_000);
        assert_eq!(ContextManager::recommended_limit("unknown"), 30_000);
    }

    #[test]
    fn remaining_tokens_calculation() {
        let cm = ContextManager::new(1000);
        let messages = vec![("user".into(), "hello".into())]; // ~2 tokens
        let remaining = cm.remaining_tokens(&messages);
        assert_eq!(remaining, 998);
    }

    #[test]
    fn is_over_budget_false_when_under() {
        let cm = ContextManager::new(1000);
        let messages = vec![("user".into(), "hello".into())];
        assert!(!cm.is_over_budget(&messages));
    }

    #[test]
    fn is_over_budget_true_when_over() {
        let cm = ContextManager::new(1);
        let messages = vec![(
            "user".into(),
            "this is definitely more than one token".into(),
        )];
        assert!(cm.is_over_budget(&messages));
    }

    #[test]
    fn compact_tool_results_recent_preserved() {
        let cm = ContextManager::new(100_000);
        let messages = vec![
            ("user".into(), "Tool execution results:\n\n<tool_result>\ntool: read_file\nstatus: SUCCESS\noutput:\nfn main() {}\n</tool_result>\n\n".into()),
            ("assistant".into(), "I see the code.".into()),
            ("user".into(), "Now fix it".into()),
            ("assistant".into(), "Sure, here is the fix.".into()),
            ("user".into(), "Thanks".into()),
            ("assistant".into(), "You're welcome!".into()),
        ];
        let compacted = cm.compact_tool_results(&messages);
        // All 6 messages are "last 6" so all preserved as-is
        assert_eq!(compacted.len(), 6);
        assert!(compacted[0].1.starts_with("Tool execution results:"));
    }

    #[test]
    fn compact_tool_results_old_compacted() {
        let cm = ContextManager::new(100_000);
        let mut messages = vec![
            ("user".into(), "Tool execution results:\n\n<tool_result>\ntool: read_file\nstatus: SUCCESS\noutput:\nlong file content here\n</tool_result>\n\n<tool_result>\ntool: ls\nstatus: SUCCESS\noutput:\ndir listing\n</tool_result>\n\n".into()),
            ("assistant".into(), "I read the files.".into()),
        ];
        // Add 6 more messages so the tool result is "old"
        for i in 0..6 {
            messages.push(("user".into(), format!("msg {i}")));
        }
        let compacted = cm.compact_tool_results(&messages);
        // The old tool result should be compacted
        assert!(
            compacted[0]
                .1
                .contains("[Previous tool execution: 2 tool(s)")
        );
    }

    #[test]
    fn compact_tool_results_old_system_context() {
        let cm = ContextManager::new(100_000);
        let mut messages = vec![
            ("system".into(), "File: /tmp/foo.rs\nfn main() {}\n".into()),
            ("user".into(), "Read this file".into()),
        ];
        for i in 0..6 {
            messages.push(("user".into(), format!("msg {i}")));
        }
        let compacted = cm.compact_tool_results(&messages);
        assert_eq!(compacted[0].1, "[Context: File: /tmp/foo.rs]");
    }

    // === Edge case tests ===

    #[test]
    fn compact_messages_with_only_system() {
        let cm = ContextManager::new(10);
        let messages = vec![("system".into(), "You are helpful.".into())];
        let compacted = cm.compact_messages(&messages);
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].0, "system");
    }

    #[test]
    fn compact_messages_exactly_at_limit() {
        let cm = ContextManager::new(1000);
        let messages = vec![
            ("user".into(), "x".repeat(2000)),
            ("assistant".into(), "y".repeat(2000)),
        ];
        // Total is ~1001 tokens — at the limit, should still compact
        let compacted = cm.compact_messages(&messages);
        // With only 2 non-system messages (<=4), it won't compact
        assert_eq!(compacted.len(), 2);
    }

    #[test]
    fn compact_tool_results_preserves_non_tool_messages() {
        let cm = ContextManager::new(10000);
        let messages = vec![
            ("user".into(), "hello".into()),
            ("assistant".into(), "hi".into()),
            ("user".into(), "how are you".into()),
        ];
        let compacted = cm.compact_tool_results(&messages);
        assert_eq!(compacted.len(), 3);
        assert_eq!(compacted[0].1, "hello"); // Unchanged
    }

    #[test]
    fn estimate_tokens_long_text() {
        let text = "x".repeat(4000);
        let tokens = ContextManager::estimate_tokens(&text);
        assert_eq!(tokens, 1334); // 4000/3 + 1
    }

    #[test]
    fn recommended_limit_unknown_provider() {
        let limit = ContextManager::recommended_limit("unknown_provider");
        assert_eq!(limit, 30_000); // Conservative default
    }

    #[test]
    fn smart_truncate_empty() {
        assert_eq!(smart_truncate("", 100), "");
    }

    #[test]
    fn smart_truncate_exactly_at_limit() {
        let text = "Hello world."; // 12 chars
        assert_eq!(smart_truncate(text, 12), "Hello world.");
    }

    #[test]
    fn smart_truncate_one_over() {
        let text = "Hello world.X"; // 13 chars, limit 12
        let result = smart_truncate(text, 12);
        assert_eq!(result, "Hello world."); // Breaks at period
    }

    // === Stress tests ===

    #[test]
    fn compact_very_large_conversation() {
        let cm = ContextManager::new(1000);
        let mut messages = Vec::new();
        for i in 0..500 {
            messages.push(("user".into(), format!("Question {i} with lots of context about programming and software development and best practices")));
            messages.push(("assistant".into(), format!("Detailed answer {i} with code examples and explanations covering multiple aspects of the topic at hand")));
        }

        let compacted = cm.compact_messages(&messages);
        assert!(
            compacted.len() < 50,
            "Should be heavily compacted, got {} messages",
            compacted.len()
        );
    }

    #[test]
    fn compact_tool_results_many_tools() {
        let cm = ContextManager::new(100_000);
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(("user".into(), format!("Tool execution results:\n<tool_result>\ntool: read_file\nstatus: SUCCESS\noutput:\n{}\n</tool_result>", "x".repeat(500))));
            messages.push(("assistant".into(), format!("Analysis of tool result {i}")));
        }

        let compacted = cm.compact_tool_results(&messages);
        // Old tool results should be compacted (brief summaries)
        let old_result = &compacted[0].1;
        assert!(
            old_result.len() < 200,
            "Old tool result should be compacted, got {} chars",
            old_result.len()
        );

        // Recent ones should be preserved
        let last = &compacted[compacted.len() - 2].1;
        assert!(
            last.contains("tool_result") || last.len() > 200,
            "Recent tool result should be preserved"
        );
    }

    #[test]
    fn conversation_tokens_large_conversation() {
        let mut messages = Vec::new();
        for _ in 0..100 {
            messages.push(("user".into(), "x".repeat(4000)));
        }
        let tokens = ContextManager::conversation_tokens(&messages);
        assert_eq!(tokens, 100 * 1334); // (4000/3 + 1) * 100
    }

    // ── effective_limit ───────────────────────────────────────────────

    #[test]
    fn effective_limit_uses_default_when_no_override() {
        let limit = ContextManager::effective_limit("openai", None);
        assert_eq!(limit, 60_000);
    }

    #[test]
    fn effective_limit_uses_override_when_set() {
        let limit = ContextManager::effective_limit("openai", Some(100_000));
        assert_eq!(limit, 100_000);
    }

    #[test]
    fn effective_limit_override_zero_is_literal() {
        let limit = ContextManager::effective_limit("openai", Some(0));
        assert_eq!(limit, 0);
    }

    #[test]
    fn effective_limit_ollama_default() {
        let limit = ContextManager::effective_limit("ollama", None);
        assert_eq!(limit, 32_000);
    }

    #[test]
    fn effective_limit_ollama_override() {
        let limit = ContextManager::effective_limit("ollama", Some(128_000));
        assert_eq!(limit, 128_000);
    }

    #[test]
    fn effective_limit_unknown_provider() {
        let limit = ContextManager::effective_limit("custom_llm", None);
        assert_eq!(limit, 30_000);
    }

    // ── compact edge cases ────────────────────────────────────────────

    #[test]
    fn compact_messages_empty() {
        let cm = ContextManager::new(100);
        let messages: Vec<(String, String)> = vec![];
        let compacted = cm.compact_messages(&messages);
        assert!(compacted.is_empty());
    }

    #[test]
    fn compact_messages_single_system() {
        let cm = ContextManager::new(100_000);
        let messages = vec![("system".into(), "You are helpful.".into())];
        let compacted = cm.compact_messages(&messages);
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].0, "system");
    }

    #[test]
    fn compact_messages_under_budget_unchanged() {
        let cm = ContextManager::new(100_000);
        let messages = vec![
            ("user".into(), "Hello".into()),
            ("assistant".into(), "Hi there!".into()),
        ];
        let compacted = cm.compact_messages(&messages);
        assert_eq!(compacted.len(), 2);
        assert_eq!(compacted[0].1, "Hello");
    }

    #[test]
    fn estimate_tokens_empty_is_one() {
        assert_eq!(ContextManager::estimate_tokens(""), 1);
    }

    #[test]
    fn estimate_tokens_two_chars() {
        // "hi" = 2 chars / 3 + 1 = 1
        assert_eq!(ContextManager::estimate_tokens("hi"), 1);
    }

    #[test]
    fn estimate_tokens_cjk_multibyte() {
        // CJK chars are 3 bytes each in UTF-8 but count as ONE character each.
        // The estimate is char-based, so 3 glyphs => 3/3 + 1 = 2 tokens — not
        // the 4 that byte-based counting used to (over)report.
        let cjk = "日本語"; // 3 chars, 9 bytes
        let est = ContextManager::estimate_tokens(cjk);
        assert_eq!(est, 3 / 3 + 1); // 2 tokens
    }
}
