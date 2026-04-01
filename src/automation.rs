use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Data types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub name: String,
    pub description: String,
    pub steps: Vec<AutomationStep>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStep {
    pub name: String,
    pub prompt_template: String, // Can contain {{input}} and {{prev_output}}
    pub model: Option<String>,   // Override model for this step
}

#[derive(Debug, Clone)]
pub struct AutomationResult {
    pub automation_name: String,
    pub step_results: Vec<StepResult>,
    pub final_output: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_name: String,
    pub output: String,
    pub duration_ms: u64,
}

// ─── Automation constructors ────────────────────────────────────────────────

impl Automation {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            steps: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn add_step(&mut self, step: AutomationStep) {
        self.steps.push(step);
    }
}

// ─── Storage helpers ────────────────────────────────────────────────────────

fn automations_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    base.join("nerve").join("automations")
}

/// Persist an automation to a TOML file in the automations directory.
pub fn save_automation(automation: &Automation) -> anyhow::Result<()> {
    let dir = automations_dir();
    fs::create_dir_all(&dir)?;

    let filename = sanitize_filename(&automation.name);
    let path = dir.join(format!("{filename}.toml"));

    let toml_str = toml::to_string_pretty(automation)?;
    fs::write(path, toml_str)?;
    Ok(())
}

/// Load a single automation by name from the automations directory.
pub fn load_automation(name: &str) -> anyhow::Result<Automation> {
    let dir = automations_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{filename}.toml"));

    if !path.exists() {
        anyhow::bail!("Automation '{}' not found", name);
    }

    let content = fs::read_to_string(path)?;
    let automation: Automation = toml::from_str(&content)?;
    Ok(automation)
}

/// List all custom automations saved on disk.
pub fn list_automations() -> anyhow::Result<Vec<Automation>> {
    let dir = automations_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut automations = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            let content = fs::read_to_string(&path)?;
            if let Ok(auto) = toml::from_str::<Automation>(&content) {
                automations.push(auto);
            }
        }
    }
    automations.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(automations)
}

/// Delete a custom automation by name.
pub fn delete_automation(name: &str) -> anyhow::Result<()> {
    let dir = automations_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{filename}.toml"));

    if !path.exists() {
        anyhow::bail!("Automation '{}' not found", name);
    }

    fs::remove_file(path)?;
    Ok(())
}

/// Turn an automation name into a safe filename (lowercase, spaces → dashes).
fn sanitize_filename(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

// ─── Built-in automations ───────────────────────────────────────────────────

/// Return the set of pre-built automations that ship with Nerve.
pub fn builtin_automations() -> Vec<Automation> {
    vec![
        // 1. Code Review Pipeline
        {
            let mut a = Automation::new(
                "Code Review Pipeline".into(),
                "Analyze code for bugs, suggest fixes, and generate corrected code.".into(),
            );
            a.add_step(AutomationStep {
                name: "Analyze Code".into(),
                prompt_template: "Analyze this code for bugs and issues:\n\n{{input}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Suggest Fixes".into(),
                prompt_template: "Now suggest specific fixes for each issue found:\n\n{{prev_output}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Generate Corrected Code".into(),
                prompt_template: "Generate the corrected code:\n\n{{prev_output}}".into(),
                model: None,
            });
            a
        },
        // 2. Content Optimizer
        {
            let mut a = Automation::new(
                "Content Optimizer".into(),
                "Analyze content for clarity, rewrite with improvements, and create a summary.".into(),
            );
            a.add_step(AutomationStep {
                name: "Analyze Content".into(),
                prompt_template: "Analyze the following content for clarity, grammar, and structure:\n\n{{input}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Rewrite Content".into(),
                prompt_template: "Rewrite the content incorporating all improvements:\n\n{{prev_output}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Summary & Headlines".into(),
                prompt_template: "Create a brief summary and 3 alternative headlines:\n\n{{prev_output}}".into(),
                model: None,
            });
            a
        },
        // 3. Research Assistant
        {
            let mut a = Automation::new(
                "Research Assistant".into(),
                "Break down a topic into research questions, analyze each, and synthesize findings.".into(),
            );
            a.add_step(AutomationStep {
                name: "Research Questions".into(),
                prompt_template: "Break down the following topic into 5 key research questions:\n\n{{input}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Detailed Analysis".into(),
                prompt_template: "For each question, provide a detailed analysis:\n\n{{prev_output}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Synthesize Findings".into(),
                prompt_template: "Synthesize the research into a comprehensive summary with key findings:\n\n{{prev_output}}".into(),
                model: None,
            });
            a
        },
        // 4. Email Drafter
        {
            let mut a = Automation::new(
                "Email Drafter".into(),
                "Analyze context for tone and key points, then draft a professional email.".into(),
            );
            a.add_step(AutomationStep {
                name: "Analyze Context".into(),
                prompt_template: "Analyze the context and determine the appropriate tone and key points:\n\n{{input}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Draft Email".into(),
                prompt_template: "Draft a professional email based on the analysis:\n\n{{prev_output}}".into(),
                model: None,
            });
            a
        },
        // 5. Translate & Localize
        {
            let mut a = Automation::new(
                "Translate & Localize".into(),
                "Translate text to a target language and review for cultural nuances.".into(),
            );
            a.add_step(AutomationStep {
                name: "Translate".into(),
                prompt_template: "Translate the following text to the target language, preserving meaning:\n\n{{input}}".into(),
                model: None,
            });
            a.add_step(AutomationStep {
                name: "Localize".into(),
                prompt_template: "Review the translation for cultural nuances and localization issues, then provide the final localized version:\n\n{{prev_output}}".into(),
                model: None,
            });
            a
        },
    ]
}

/// Find an automation by name across both built-in and custom automations.
/// Built-in automations are checked first, then custom ones from disk.
pub fn find_automation(name: &str) -> anyhow::Result<Automation> {
    let name_lower = name.to_lowercase();

    // Check built-ins first.
    for auto in builtin_automations() {
        if auto.name.to_lowercase() == name_lower {
            return Ok(auto);
        }
    }

    // Try loading from disk.
    load_automation(name)
}

/// List all automations (built-in + custom), with built-ins first.
pub fn all_automations() -> Vec<Automation> {
    let mut all = builtin_automations();
    if let Ok(custom) = list_automations() {
        all.extend(custom);
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_new_has_correct_fields() {
        let a = Automation::new("Test Auto".to_string(), "A test automation".to_string());
        assert_eq!(a.name, "Test Auto");
        assert_eq!(a.description, "A test automation");
        assert!(a.steps.is_empty());
    }

    #[test]
    fn automation_add_step_appends() {
        let mut a = Automation::new("Test".to_string(), "Desc".to_string());
        assert!(a.steps.is_empty());

        a.add_step(AutomationStep {
            name: "Step 1".to_string(),
            prompt_template: "Do {{input}}".to_string(),
            model: None,
        });
        assert_eq!(a.steps.len(), 1);
        assert_eq!(a.steps[0].name, "Step 1");

        a.add_step(AutomationStep {
            name: "Step 2".to_string(),
            prompt_template: "Then {{prev_output}}".to_string(),
            model: Some("gpt-4".to_string()),
        });
        assert_eq!(a.steps.len(), 2);
        assert_eq!(a.steps[1].name, "Step 2");
        assert_eq!(a.steps[1].model.as_deref(), Some("gpt-4"));
    }

    #[test]
    fn builtin_automations_returns_five() {
        let builtins = builtin_automations();
        assert_eq!(builtins.len(), 5, "Expected 5 built-in automations");
    }

    #[test]
    fn all_builtin_automations_have_required_fields() {
        for a in builtin_automations() {
            assert!(!a.name.is_empty(), "Automation has empty name");
            assert!(
                !a.description.is_empty(),
                "Automation '{}' has empty description",
                a.name
            );
            assert!(
                !a.steps.is_empty(),
                "Automation '{}' has no steps",
                a.name
            );
        }
    }

    #[test]
    fn all_builtin_steps_have_input_or_prev_output_placeholder() {
        for a in builtin_automations() {
            for step in &a.steps {
                let has_input = step.prompt_template.contains("{{input}}");
                let has_prev = step.prompt_template.contains("{{prev_output}}");
                assert!(
                    has_input || has_prev,
                    "Step '{}' in automation '{}' has no {{{{input}}}} or {{{{prev_output}}}} placeholder",
                    step.name,
                    a.name
                );
            }
        }
    }

    #[test]
    fn builtin_automation_names_are_unique() {
        let builtins = builtin_automations();
        let mut names = std::collections::HashSet::new();
        for a in &builtins {
            assert!(
                names.insert(&a.name),
                "Duplicate builtin automation name: {}",
                a.name
            );
        }
    }

    #[test]
    fn sanitize_filename_basic() {
        assert_eq!(sanitize_filename("Hello World"), "hello-world");
        assert_eq!(sanitize_filename("Code Review Pipeline"), "code-review-pipeline");
        assert_eq!(sanitize_filename("test_name"), "test_name");
        assert_eq!(sanitize_filename("special!@#chars"), "special---chars");
    }

    #[test]
    fn sanitize_filename_preserves_alphanumeric_and_dashes() {
        let result = sanitize_filename("my-automation-123");
        assert_eq!(result, "my-automation-123");
    }

    #[test]
    fn automation_serialization_roundtrip() {
        let mut a = Automation::new("Test Ser".to_string(), "Serialization test".to_string());
        a.add_step(AutomationStep {
            name: "Step 1".to_string(),
            prompt_template: "Do {{input}}".to_string(),
            model: None,
        });

        let toml_str = toml::to_string_pretty(&a).expect("serialize to TOML");
        let deserialized: Automation = toml::from_str(&toml_str).expect("deserialize from TOML");

        assert_eq!(deserialized.name, a.name);
        assert_eq!(deserialized.description, a.description);
        assert_eq!(deserialized.steps.len(), 1);
        assert_eq!(deserialized.steps[0].name, "Step 1");
    }

    #[test]
    fn all_automations_includes_builtins() {
        let all = all_automations();
        let builtin_count = builtin_automations().len();
        assert!(
            all.len() >= builtin_count,
            "all_automations() ({}) should include at least {} builtins",
            all.len(),
            builtin_count
        );

        // Verify each builtin name is present in all_automations
        let all_names: std::collections::HashSet<&str> =
            all.iter().map(|a| a.name.as_str()).collect();
        for b in builtin_automations() {
            assert!(
                all_names.contains(b.name.as_str()),
                "Builtin '{}' missing from all_automations()",
                b.name
            );
        }
    }

    #[test]
    fn find_automation_finds_builtin_case_insensitive() {
        let result = find_automation("code review pipeline");
        assert!(result.is_ok(), "Should find builtin by lowercase name");
        assert_eq!(result.unwrap().name, "Code Review Pipeline");
    }

    #[test]
    fn find_automation_missing_returns_error() {
        let result = find_automation("nonexistent automation xyz");
        assert!(result.is_err(), "Should return error for missing automation");
    }

    #[test]
    fn save_and_load_automation_roundtrip() {
        let mut a = Automation::new(
            format!("Test Save {}", uuid::Uuid::new_v4()),
            "Roundtrip test".to_string(),
        );
        a.add_step(AutomationStep {
            name: "Only Step".to_string(),
            prompt_template: "Process: {{input}}".to_string(),
            model: None,
        });

        save_automation(&a).expect("save");
        let loaded = load_automation(&a.name).expect("load");

        assert_eq!(loaded.name, a.name);
        assert_eq!(loaded.description, a.description);
        assert_eq!(loaded.steps.len(), 1);

        // Cleanup
        delete_automation(&a.name).expect("cleanup");
    }

    #[test]
    fn delete_automation_removes_file() {
        let name = format!("Test Delete {}", uuid::Uuid::new_v4());
        let a = Automation::new(name.clone(), "Delete test".to_string());
        save_automation(&a).expect("save");

        delete_automation(&name).expect("delete");

        let result = load_automation(&name);
        assert!(result.is_err(), "Loading deleted automation should fail");
    }

    #[test]
    fn delete_missing_automation_returns_error() {
        let result = delete_automation("nonexistent_automation_xyz_12345");
        assert!(result.is_err(), "Deleting nonexistent automation should error");
    }

    #[test]
    fn builtin_automations_all_have_steps() {
        for auto in builtin_automations() {
            assert!(!auto.steps.is_empty(),
                "Automation '{}' has no steps", auto.name);
        }
    }

    #[test]
    fn builtin_steps_have_templates() {
        for auto in builtin_automations() {
            for step in &auto.steps {
                assert!(!step.prompt_template.is_empty(),
                    "Step '{}' in '{}' has empty template", step.name, auto.name);
            }
        }
    }

    #[test]
    fn automation_toml_roundtrip() {
        let mut auto = Automation::new("Test".into(), "A test".into());
        auto.add_step(AutomationStep {
            name: "Step 1".into(),
            prompt_template: "Do {{input}}".into(),
            model: None,
        });

        let toml_str = toml::to_string(&auto).unwrap();
        let restored: Automation = toml::from_str(&toml_str).unwrap();
        assert_eq!(restored.name, "Test");
        assert_eq!(restored.steps.len(), 1);
    }

    #[test]
    fn all_automations_includes_custom_after_save() {
        // Save a custom automation with a unique name
        let name = format!("TestCustom_{}", uuid::Uuid::new_v4());
        let mut auto = Automation::new(name.clone(), "Test".into());
        auto.add_step(AutomationStep {
            name: "Step1".into(),
            prompt_template: "Do {{input}}".into(),
            model: None,
        });
        let _ = save_automation(&auto);

        // all_automations should include builtins + custom
        let all = all_automations();
        assert!(all.len() > builtin_automations().len()); // Has at least the custom one extra
        assert!(all.iter().any(|a| a.name == name));

        // Clean up
        let _ = delete_automation(&name);
    }

    #[test]
    fn find_automation_case_insensitive_lookup() {
        let result = find_automation("code review pipeline");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Code Review Pipeline");
    }

    #[test]
    fn builtin_automation_step_count() {
        let builtins = builtin_automations();
        // Verify specific automations have expected step counts
        let code_review = builtins.iter().find(|a| a.name == "Code Review Pipeline");
        assert!(code_review.is_some());
        assert!(code_review.unwrap().steps.len() >= 3);
    }

    #[test]
    fn exactly_five_builtin_automations() {
        assert_eq!(builtin_automations().len(), 5);
    }
}
