/// Manages conversation context to stay within token limits
pub struct ContextManager {
    max_tokens: usize,
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

    /// Create a brief summary of messages
    fn summarize_messages(messages: &[&(String, String)]) -> String {
        let mut summary = String::new();
        let mut exchange_count = 0;

        for (role, content) in messages {
            match role.as_str() {
                "user" => {
                    exchange_count += 1;
                    let brief: String = content.chars().take(100).collect();
                    summary.push_str(&format!("- User asked: {brief}...\n"));
                }
                "assistant" => {
                    let brief: String = content.chars().take(100).collect();
                    summary.push_str(&format!("  AI responded: {brief}...\n"));
                }
                _ => {}
            }
        }

        if exchange_count == 0 {
            return "No prior exchanges.".into();
        }

        format!("[{exchange_count} previous exchange(s)]\n{summary}")
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
}
