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
