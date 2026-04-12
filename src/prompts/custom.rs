use std::fs;
use std::path::PathBuf;

use super::SmartPrompt;

/// Return the directory where custom user prompts are stored.
///
/// Defaults to `~/.config/nerve/prompts/`.
fn custom_prompts_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nerve")
        .join("prompts")
}

/// Derive a filesystem-safe filename from a prompt name.
///
/// Lowercases, replaces whitespace runs with `_`, strips non-alphanumeric
/// characters (except `_` and `-`), and appends `.toml`.
#[allow(dead_code)]
pub fn prompt_filename(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if slug.is_empty() {
        return "unnamed.toml".to_string();
    }
    format!("{slug}.toml")
}

/// Scan `~/.config/nerve/prompts/` and load every `.toml` file as a
/// [`SmartPrompt`]. Files that fail to parse are silently skipped.
pub fn load_custom_prompts() -> Vec<SmartPrompt> {
    let dir = custom_prompts_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut prompts = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml")
            && let Ok(contents) = fs::read_to_string(&path)
            && let Ok(prompt) = toml::from_str::<SmartPrompt>(&contents)
        {
            prompts.push(prompt);
        }
    }

    prompts.sort_by(|a, b| a.name.cmp(&b.name));
    prompts
}

/// Save a custom prompt as a `.toml` file in the custom prompts directory.
///
/// Creates the directory if it does not exist. Overwrites any existing file
/// with the same derived filename.
#[allow(dead_code)]
pub fn save_custom_prompt(prompt: &SmartPrompt) -> anyhow::Result<()> {
    let dir = custom_prompts_dir();
    fs::create_dir_all(&dir)?;

    let filename = prompt_filename(&prompt.name);
    let path = dir.join(filename);
    let contents = toml::to_string_pretty(prompt)?;
    fs::write(path, contents)?;
    Ok(())
}

/// Delete a custom prompt by name.
///
/// Returns `Ok(())` if the file was removed or did not exist.
#[allow(dead_code)]
pub fn delete_custom_prompt(name: &str) -> anyhow::Result<()> {
    let dir = custom_prompts_dir();
    let filename = prompt_filename(name);
    let path = dir.join(filename);

    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::SmartPrompt;

    #[test]
    fn smartprompt_toml_roundtrip() {
        let prompt = SmartPrompt {
            name: "Test Prompt".into(),
            description: "A test".into(),
            template: "Do something with {{input}}".into(),
            category: "Custom".into(),
            tags: vec!["test".into(), "example".into()],
        };
        let toml_str = toml::to_string(&prompt).unwrap();
        let restored: SmartPrompt = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.name, prompt.name);
        assert_eq!(restored.template, prompt.template);
        assert_eq!(restored.tags, prompt.tags);
    }

    #[test]
    fn prompt_filename_sanitizes() {
        let name = prompt_filename("Hello World!");
        assert!(!name.contains(' '));
        assert!(!name.contains('!'));
        assert!(name.ends_with(".toml"));
    }

    #[test]
    fn prompt_filename_lowercases() {
        let name = prompt_filename("My PROMPT");
        assert_eq!(name, "my_prompt.toml");
    }

    #[test]
    fn prompt_filename_strips_special_chars() {
        let name = prompt_filename("Test@#$%^&*()Prompt");
        assert_eq!(name, "testprompt.toml");
    }

    #[test]
    fn prompt_filename_preserves_hyphens() {
        let name = prompt_filename("code-review");
        assert_eq!(name, "code-review.toml");
    }

    #[test]
    fn prompt_filename_collapses_whitespace() {
        let name = prompt_filename("hello   world   test");
        assert_eq!(name, "hello_world_test.toml");
    }

    #[test]
    fn prompt_filename_empty_name() {
        let name = prompt_filename("");
        assert_eq!(name, ".toml");
    }

    #[test]
    fn smartprompt_empty_tags_roundtrip() {
        let prompt = SmartPrompt {
            name: "No Tags".into(),
            description: "Desc".into(),
            template: "template".into(),
            category: "Cat".into(),
            tags: vec![],
        };
        let toml_str = toml::to_string(&prompt).unwrap();
        let restored: SmartPrompt = toml::from_str(&toml_str).unwrap();
        assert!(restored.tags.is_empty());
    }

    #[test]
    fn smartprompt_with_multiline_template() {
        let prompt = SmartPrompt {
            name: "Multi".into(),
            description: "Desc".into(),
            template: "Line 1\nLine 2\nLine 3".into(),
            category: "Cat".into(),
            tags: vec![],
        };
        let toml_str = toml::to_string(&prompt).unwrap();
        let restored: SmartPrompt = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.template, "Line 1\nLine 2\nLine 3");
    }
}
