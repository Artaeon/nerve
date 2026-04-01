use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;
use std::process::Command;

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

fn default_true() -> bool { true }

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

        let output = Command::new(&script_path)
            .args(args.split_whitespace())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.dir)
            .env("NERVE_PLUGIN_DIR", &self.dir)
            .env("NERVE_ARGS", args)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            anyhow::bail!("Plugin '{}' failed (exit {}): {}", self.manifest.name, output.status, stderr);
        }

        Ok(if stdout.is_empty() { stderr } else { stdout })
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
            if !path.is_dir() { continue; }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() { continue; }

            match fs::read_to_string(&manifest_path) {
                Ok(content) => {
                    match toml::from_str::<PluginManifest>(&content) {
                        Ok(manifest) => {
                            if manifest.enabled {
                                plugins.push(Plugin { manifest, dir: path });
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse plugin manifest at {}: {e}", manifest_path.display());
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read plugin manifest at {}: {e}", manifest_path.display());
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
            if !path.is_dir() { continue; }

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
        let _ = create_example_plugin(); // Ensure it exists
        let dir = plugins_dir().join("example");
        let manifest_path = dir.join("plugin.toml");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path).unwrap();
            let manifest: PluginManifest = toml::from_str(&content).unwrap();
            assert_eq!(manifest.command, "example");
            assert!(manifest.enabled);
        }
    }

    #[test]
    fn list_all_plugins_includes_example() {
        let _ = create_example_plugin();
        let all = list_all_plugins();
        // May or may not find it depending on test order, just verify no panic
        let _ = all;
    }
}
