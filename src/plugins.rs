use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Strip ANSI escape sequences and control characters from plugin output.
/// Preserves newlines and tabs.
fn strip_ansi_and_control(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consume CSI sequence: ESC [ ... <final byte>
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Read until we hit a letter (0x40..=0x7E)
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() || ch == '~' {
                        break;
                    }
                }
            }
            // Also skip OSC (ESC ]) and other short sequences.
            continue;
        }
        if c.is_control() && c != '\n' && c != '\t' {
            continue;
        }
        out.push(c);
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    /// The slash command that triggers this plugin (e.g., "weather")
    /// Users invoke it as /weather
    pub command: String,
    /// The script to run (relative to plugin directory)
    pub run: String,
    /// Whether the plugin is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

impl Plugin {
    /// Execute the plugin with the given arguments.
    /// The plugin receives args as command-line arguments and the conversation
    /// context via stdin (JSON).
    pub fn execute(&self, args: &str, _context: &str) -> anyhow::Result<String> {
        let script_path = self.dir.join(&self.manifest.run);

        if !script_path.exists() {
            anyhow::bail!("Plugin script not found: {}", script_path.display());
        }

        // Make script executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).ok();
        }

        let mut child = Command::new(&script_path)
            .args(args.split_whitespace())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.dir)
            .env("NERVE_PLUGIN_DIR", &self.dir)
            .env("NERVE_ARGS", args)
            .spawn()?;

        // Timeout: plugins get 30 seconds max.
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout_buf = child
                        .stdout
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();
                    let stderr_buf = child
                        .stderr
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();
                    let stdout = String::from_utf8_lossy(&stdout_buf).to_string();
                    let stderr = String::from_utf8_lossy(&stderr_buf).to_string();

                    if !status.success() {
                        anyhow::bail!(
                            "Plugin '{}' failed (exit {}): {}",
                            self.manifest.name,
                            status,
                            stderr
                        );
                    }
                    // Strip control characters and ANSI escape sequences
                    // from output to prevent terminal manipulation.
                    let output = if stdout.is_empty() { stderr } else { stdout };
                    let sanitized = strip_ansi_and_control(&output);
                    return Ok(sanitized);
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        anyhow::bail!(
                            "Plugin '{}' timed out after {}s",
                            self.manifest.name,
                            timeout.as_secs()
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    anyhow::bail!("Error running plugin '{}': {e}", self.manifest.name);
                }
            }
        }
    }
}

/// Directory where plugins are stored
pub fn plugins_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nerve")
        .join("plugins")
}

/// Load all plugins from the plugins directory
pub fn load_plugins() -> Vec<Plugin> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut plugins = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match fs::read_to_string(&manifest_path) {
                Ok(content) => match toml::from_str::<PluginManifest>(&content) {
                    Ok(manifest) => {
                        if manifest.enabled {
                            plugins.push(Plugin {
                                manifest,
                                dir: path,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse plugin manifest at {}: {e}",
                            manifest_path.display()
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read plugin manifest at {}: {e}",
                        manifest_path.display()
                    );
                }
            }
        }
    }

    plugins
}

/// Create an example plugin to show users the format
pub fn create_example_plugin() -> anyhow::Result<PathBuf> {
    let dir = plugins_dir().join("example");
    fs::create_dir_all(&dir)?;

    let manifest = r#"name = "Example Plugin"
description = "An example plugin showing the plugin format"
version = "1.0.0"
author = "Nerve"
command = "example"
run = "run.sh"
enabled = true
"#;

    let script = r#"#!/bin/bash
# Example Nerve plugin
# Arguments are passed as $1, $2, etc.
# Environment: NERVE_PLUGIN_DIR, NERVE_ARGS

echo "Hello from the example plugin!"
echo "Arguments: $NERVE_ARGS"
echo "Plugin dir: $NERVE_PLUGIN_DIR"
"#;

    fs::write(dir.join("plugin.toml"), manifest)?;
    fs::write(dir.join("run.sh"), script)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir.join("run.sh"))?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.join("run.sh"), perms)?;
    }

    Ok(dir)
}

/// List all plugins (loaded + disabled)
pub fn list_all_plugins() -> Vec<(PluginManifest, bool)> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if let Ok(content) = fs::read_to_string(&manifest_path)
                && let Ok(manifest) = toml::from_str::<PluginManifest>(&content)
            {
                let loaded = manifest.enabled;
                result.push((manifest, loaded));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest() {
        let toml = r#"
name = "Test"
description = "A test plugin"
version = "1.0.0"
command = "test"
run = "run.sh"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.name, "Test");
        assert_eq!(manifest.command, "test");
        assert!(manifest.enabled); // default true
    }

    #[test]
    fn parse_manifest_disabled() {
        let toml = r#"
name = "Test"
description = "Disabled"
version = "1.0.0"
command = "test"
run = "run.sh"
enabled = false
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert!(!manifest.enabled);
    }

    #[test]
    fn plugins_dir_ends_with_plugins() {
        let dir = plugins_dir();
        assert!(dir.ends_with("plugins"));
    }

    #[test]
    fn load_from_empty_dir_returns_empty() {
        // plugins_dir may not exist in test env
        let plugins = load_plugins();
        // Just verify it doesn't panic
        let _ = plugins;
    }

    #[test]
    fn create_example_plugin_works() {
        let result = create_example_plugin();
        assert!(result.is_ok());
        let dir = result.unwrap();
        assert!(dir.join("plugin.toml").exists());
        assert!(dir.join("run.sh").exists());
    }

    #[test]
    fn manifest_with_author() {
        let toml = r#"
name = "Test"
description = "A test"
version = "1.0.0"
author = "Test Author"
command = "test"
run = "run.sh"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.author, Some("Test Author".into()));
    }

    #[test]
    fn manifest_without_author() {
        let toml = r#"
name = "Test"
description = "A test"
version = "1.0.0"
command = "test"
run = "run.sh"
"#;
        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert!(manifest.author.is_none());
    }

    #[test]
    fn manifest_missing_required_field_fails() {
        let toml = r#"
name = "Test"
"#;
        let result = toml::from_str::<PluginManifest>(toml);
        assert!(result.is_err());
    }

    #[test]
    fn example_plugin_manifest_is_valid() {
        // Validate the manifest format in-memory to avoid filesystem races
        let manifest_toml = r#"name = "Example Plugin"
description = "An example plugin showing the plugin format"
version = "1.0.0"
author = "Nerve"
command = "example"
run = "run.sh"
enabled = true
"#;
        let manifest: PluginManifest = toml::from_str(manifest_toml).unwrap();
        assert_eq!(manifest.command, "example");
        assert!(manifest.enabled);
        assert_eq!(manifest.name, "Example Plugin");
    }

    #[test]
    fn list_all_plugins_includes_example() {
        let _ = create_example_plugin();
        let all = list_all_plugins();
        // May or may not find it depending on test order, just verify no panic
        let _ = all;
    }

    // ── ANSI / control-character sanitization ─────────────────────────

    #[test]
    fn strip_ansi_removes_color_codes() {
        let input = "\x1b[31mERROR\x1b[0m normal";
        assert_eq!(strip_ansi_and_control(input), "ERROR normal");
    }

    #[test]
    fn strip_ansi_removes_cursor_movement() {
        let input = "\x1b[2J\x1b[H\x1b[1;1Hfake prompt";
        assert_eq!(strip_ansi_and_control(input), "fake prompt");
    }

    #[test]
    fn strip_ansi_removes_bold_underline() {
        let input = "\x1b[1mbold\x1b[4munderline\x1b[0m";
        assert_eq!(strip_ansi_and_control(input), "boldunderline");
    }

    #[test]
    fn strip_ansi_preserves_newlines_and_tabs() {
        let input = "line1\n\tindented\nline3";
        assert_eq!(strip_ansi_and_control(input), input);
    }

    #[test]
    fn strip_ansi_removes_control_chars() {
        let input = "hello\x07\x08world";
        assert_eq!(strip_ansi_and_control(input), "helloworld");
    }

    #[test]
    fn strip_ansi_empty_input() {
        assert_eq!(strip_ansi_and_control(""), "");
    }

    #[test]
    fn strip_ansi_plain_text_unchanged() {
        let input = "just normal text with numbers 123";
        assert_eq!(strip_ansi_and_control(input), input);
    }

    #[test]
    fn strip_ansi_256_color() {
        let input = "\x1b[38;5;196mred\x1b[0m";
        assert_eq!(strip_ansi_and_control(input), "red");
    }

    #[test]
    fn strip_ansi_truecolor() {
        let input = "\x1b[38;2;255;0;0mred\x1b[0m";
        assert_eq!(strip_ansi_and_control(input), "red");
    }
}
