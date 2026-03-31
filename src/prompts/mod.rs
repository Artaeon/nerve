pub mod builtin;
pub mod custom;

use serde::{Deserialize, Serialize};

/// A reusable prompt template ("SmartPrompt") for the Nerve assistant.
///
/// Templates can contain `{{variable}}` placeholders that are substituted
/// at runtime. The most common placeholder is `{{input}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartPrompt {
    pub name: String,
    pub description: String,
    /// The prompt template body. May contain `{{variable}}` placeholders.
    pub template: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Return the combined set of built-in and user-defined custom prompts.
///
/// Custom prompts are appended after built-in prompts. If a custom prompt
/// shares the same name as a built-in, both are kept (the UI can decide
/// how to handle duplicates).
pub fn all_prompts() -> Vec<SmartPrompt> {
    let mut prompts = builtin::builtin_prompts();
    prompts.extend(custom::load_custom_prompts());
    prompts
}

/// Return a sorted, deduplicated list of every category present across
/// all built-in and custom prompts.
pub fn categories() -> Vec<String> {
    let mut cats: Vec<String> = all_prompts()
        .iter()
        .map(|p| p.category.clone())
        .collect();
    cats.sort();
    cats.dedup();
    cats
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_prompts_includes_at_least_builtins() {
        let builtin_count = builtin::builtin_prompts().len();
        let all_count = all_prompts().len();
        assert!(
            all_count >= builtin_count,
            "all_prompts() ({all_count}) should be >= builtin count ({builtin_count})"
        );
    }

    #[test]
    fn categories_returns_sorted_unique() {
        let cats = categories();
        // Check sorted
        let mut sorted = cats.clone();
        sorted.sort();
        assert_eq!(cats, sorted, "categories() should return sorted list");
        // Check unique (no duplicates)
        let unique: HashSet<&String> = cats.iter().collect();
        assert_eq!(
            cats.len(),
            unique.len(),
            "categories() should have no duplicates"
        );
    }

    #[test]
    fn categories_includes_all_expected() {
        let cats = categories();
        for expected in &[
            "Writing",
            "Coding",
            "Translation",
            "Analysis",
            "Creative",
            "Productivity",
            "Engineering",
            "Design",
            "Best Practices",
            "Git",
        ] {
            assert!(
                cats.iter().any(|c| c == expected),
                "Missing expected category: {expected}"
            );
        }
    }

    #[test]
    fn categories_has_no_duplicates() {
        let cats = categories();
        let set: HashSet<&str> = cats.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            cats.len(),
            set.len(),
            "categories() returned duplicates"
        );
    }
}
