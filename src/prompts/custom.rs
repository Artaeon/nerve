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
fn prompt_filename(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
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
        if path.extension().is_some_and(|ext| ext == "toml") {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(prompt) = toml::from_str::<SmartPrompt>(&contents) {
                    prompts.push(prompt);
                }
            }
        }
    }

    prompts.sort_by(|a, b| a.name.cmp(&b.name));
    prompts
}

/// Save a custom prompt as a `.toml` file in the custom prompts directory.
///
/// Creates the directory if it does not exist. Overwrites any existing file
/// with the same derived filename.
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
pub fn delete_custom_prompt(name: &str) -> anyhow::Result<()> {
    let dir = custom_prompts_dir();
    let filename = prompt_filename(name);
    let path = dir.join(filename);

    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
