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
