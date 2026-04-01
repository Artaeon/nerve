/// Manages conversation context to stay within token limits
pub struct ContextManager {
    max_tokens: usize,
}

/// Truncate text intelligently at a sentence boundary when possible.
pub(crate) fn smart_truncate(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    let truncated = &text[..max_chars];

    // Try to break at the last sentence-ending punctuation within the range.
    if let Some(pos) = truncated.rfind(|c| c == '.' || c == '!' || c == '?') {
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

    /// Estimate token count for a message (rough: ~4 chars per token)
    pub fn estimate_tokens(text: &str) -> usize {
        text.len() / 4 + 1
    }

    /// Estimate total tokens for a conversation
    pub fn conversation_tokens(messages: &[(String, String)]) -> usize {
        messages
            .iter()
            .map(|(_, content)| Self::estimate_tokens(content))
            .sum()
    }

    /// Return a recommended context token limit for a given provider name.
    pub fn recommended_limit(provider: &str) -> usize {
        match provider {
            "claude_code" | "claude" => 200_000, // Claude has huge context
            "openai" => 60_000,                  // GPT-4o has 128K but we want headroom
            "openrouter" => 30_000,              // Be conservative — depends on model
            "ollama" => 8_000,                   // Local models often have small context
            _ => 30_000,
        }
    }

    /// Check whether the conversation exceeds the token budget.
    pub fn is_over_budget(&self, messages: &[(String, String)]) -> bool {
        Self::conversation_tokens(messages) > self.max_tokens
    }

    /// Calculate remaining tokens available in the context window.
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
                let brief = format!(
                    "[Previous tool execution: {} tool(s) ran successfully]",
                    tool_count
                );
                result.push((role.clone(), brief));
            } else if role == "system"
                && (content.starts_with("File:")
                    || content.starts_with("Command:")
                    || content.starts_with("Directory listing"))
            {
                // Compact old file/command context
                let first_line: String = content.lines().next().unwrap_or("").to_string();
                result.push((role.clone(), format!("[Context: {}]", first_line)));
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

        // Keep system messages, last 4 messages, and summarize the rest
        let mut result = Vec::new();

        // Collect system messages
        for (role, content) in messages {
            if role == "system" {
                result.push((role.clone(), content.clone()));
            }
        }

        // Non-system messages
        let non_system: Vec<&(String, String)> =
            messages.iter().filter(|(r, _)| r != "system").collect();

        if non_system.len() <= 4 {
            result.extend(non_system.iter().map(|(r, c)| (r.clone(), c.clone())));
            return result;
        }

        // Summarize older messages
        let keep_count = 4;
        let to_summarize = &non_system[..non_system.len() - keep_count];
        let to_keep = &non_system[non_system.len() - keep_count..];

        // Create summary
        let summary = Self::summarize_messages(to_summarize);
        result.push((
            "system".into(),
            format!("Previous conversation summary:\n{summary}"),
        ));

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
                    summary.push_str(&format!("- User: {brief}\n"));
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
                        summary.push_str(&format!(
                            "  AI: {first_line}... [{code_blocks} code block(s)]\n"
                        ));
                    } else {
                        let brief = smart_truncate(content, 150);
                        summary.push_str(&format!("  AI: {brief}\n"));
                    }
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
        // Should have system summary + last 4 messages
        assert!(compacted.len() <= 5);
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
    fn smart_truncate_no_spaces() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let result = smart_truncate(text, 10);
        assert_eq!(result, "abcdefghij...");
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
        assert_eq!(ContextManager::recommended_limit("ollama"), 8_000);
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
        let messages = vec![("user".into(), "this is definitely more than one token".into())];
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
        assert!(compacted[0].1.contains("[Previous tool execution: 2 tool(s)"));
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
        assert_eq!(tokens, 1001); // 4000/4 + 1
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
}
