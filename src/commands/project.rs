//! Project-memory commands: `/init`, `/remember`, `/memory`, `/decision`,
//! `/decisions`, `/improve`, `/improvements`.
//!
//! These are the user-facing surface of the per-project `.nerve/` store
//! (see `crate::project`). Everything a user records here is injected into
//! future prompts as "Project memory".

use std::sync::Arc;

use crate::ai::provider::{AiProvider, ChatMessage};
use crate::app::App;
use crate::project::ProjectStore;

/// Handle project-memory commands. Returns `true` when the input matched.
pub async fn handle(app: &mut App, text: &str, provider: &Arc<dyn AiProvider>) -> bool {
    let (cmd, args) = match text.find(char::is_whitespace) {
        Some(pos) => (&text[..pos], text[pos..].trim()),
        None => (text, ""),
    };

    match cmd {
        "/init" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let Some(ws) = app.cached_workspace.clone() else {
                return true;
            };
            app.set_status("Analyzing project to generate engineering brief...");

            let analysis_input = build_analysis_input(&ws);
            let messages = vec![
                ChatMessage::system(
                    "You are a senior engineer writing an engineering brief of a repository \
                     for a colleague who will work on it. Be factual and specific — no \
                     marketing language. Cover: purpose, tech stack, layout (key dirs/files), \
                     architecture, notable conventions, and how to build/test/lint/run. \
                     Plain text (markdown headers allowed), at most 40 lines.",
                ),
                ChatMessage::user(analysis_input),
            ];
            match provider.chat(&messages, &app.selected_model).await {
                Ok(brief) if !brief.trim().is_empty() => match store.save_brief(&brief) {
                    Ok(()) => {
                        app.add_assistant_message(format!(
                            "## Engineering brief (saved to {})\n\n{brief}",
                            store.brief_path().display()
                        ));
                        app.set_status(
                            "Brief saved — injected into every future prompt (/memory to view)",
                        );
                    }
                    Err(e) => app.set_status(format!("Could not save brief: {e}")),
                },
                Ok(_) => app.set_status("Model returned an empty brief — try again"),
                Err(e) => app.set_status(format!("Brief generation failed: {e}")),
            }
            true
        }
        "/remember" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            if args.is_empty() {
                app.set_status("Usage: /remember <fact about this project>");
                return true;
            }
            match store.remember(args) {
                Ok(()) => app.set_status(format!(
                    "Remembered. ({} — injected into every prompt)",
                    store.memory_path().display()
                )),
                Err(e) => app.set_status(format!("Could not save memory: {e}")),
            }
            true
        }
        "/memory" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let mut out = String::new();
            if let Some(brief) = store.load_brief() {
                out.push_str("## Engineering brief\n\n");
                out.push_str(&brief);
                out.push_str("\n\n");
            }
            match store.load_memory() {
                Some(memory) => {
                    out.push_str(&memory);
                    out.push('\n');
                }
                None if out.is_empty() => {
                    app.set_status(
                        "No project memory yet — /remember <fact> to add one, \
                         or /init to generate an engineering brief",
                    );
                    return true;
                }
                None => {}
            }
            let decisions = store.recent_decisions(5);
            if !decisions.is_empty() {
                out.push_str("\n## Recent decisions\n");
                for d in &decisions {
                    out.push_str(&format!("- {}\n", d.text));
                }
            }
            app.add_assistant_message(out);
            true
        }
        "/decision" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            if args.is_empty() {
                app.set_status("Usage: /decision <decision made in this project>");
                return true;
            }
            match store.record_decision(args) {
                Ok(()) => app.set_status("Decision recorded (last 5 are injected into prompts)"),
                Err(e) => app.set_status(format!("Could not record decision: {e}")),
            }
            true
        }
        "/decisions" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let decisions = store.recent_decisions(20);
            if decisions.is_empty() {
                app.set_status("No decisions recorded — /decision <text> to add one");
                return true;
            }
            let mut out = String::from("## Decision log (most recent 20)\n\n");
            for d in &decisions {
                let date = d.timestamp.split('T').next().unwrap_or("");
                out.push_str(&format!("- [{date}] {}\n", d.text));
            }
            app.add_assistant_message(out);
            true
        }
        "/improve" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            if args.is_empty() {
                app.set_status("Usage: /improve <improvement idea for the backlog>");
                return true;
            }
            match store.add_improvement(args) {
                Ok(id) => app.set_status(format!(
                    "Improvement #{id} added — /improvements to list, /improvements done {id} to close"
                )),
                Err(e) => app.set_status(format!("Could not add improvement: {e}")),
            }
            true
        }
        "/improvements" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            // "/improvements done <id>" closes an entry.
            if let Some(rest) = args.strip_prefix("done") {
                match rest.trim().parse::<u64>() {
                    Ok(id) => match store.complete_improvement(id) {
                        Ok(true) => app.set_status(format!("Improvement #{id} marked done")),
                        Ok(false) => app.set_status(format!("No improvement with id {id}")),
                        Err(e) => app.set_status(format!("Could not update: {e}")),
                    },
                    Err(_) => app.set_status("Usage: /improvements done <id>"),
                }
                return true;
            }
            let items = store.list_improvements();
            if items.is_empty() {
                app.set_status("Backlog is empty — /improve <idea> to add one");
                return true;
            }
            let open: Vec<_> = items.iter().filter(|i| i.status == "open").collect();
            let done: Vec<_> = items.iter().filter(|i| i.status != "open").collect();
            let mut out = String::from("## Improvement backlog\n\n");
            for i in &open {
                out.push_str(&format!("- [ ] #{} {}\n", i.id, i.text));
            }
            for i in &done {
                out.push_str(&format!("- [x] #{} {}\n", i.id, i.text));
            }
            app.add_assistant_message(out);
            true
        }
        _ => false,
    }
}

/// Assemble the read-only analysis input for `/init`: workspace profile,
/// project map, README head and the primary manifest, all capped so the
/// request stays small.
fn build_analysis_input(ws: &crate::workspace::WorkspaceInfo) -> String {
    let mut input = String::from("Write the engineering brief for this repository.\n\n");
    input.push_str(&ws.to_system_prompt());
    input.push_str("\n\nProject map:\n");
    let map = crate::workspace::generate_project_map(&ws.root, 3);
    input.push_str(&cap_chars(&map, 3000));

    if let Ok(readme) = std::fs::read_to_string(ws.root.join("README.md")) {
        input.push_str("\n\nREADME.md (head):\n");
        input.push_str(&cap_chars(&readme, 2500));
    }
    for manifest in ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"] {
        if let Ok(content) = std::fs::read_to_string(ws.root.join(manifest)) {
            input.push_str(&format!("\n\n{manifest}:\n"));
            input.push_str(&cap_chars(&content, 1500));
            break;
        }
    }
    input
}

fn cap_chars(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        let head: String = text.chars().take(max).collect();
        format!("{head}\n[truncated]")
    }
}

/// The store for the current workspace, or a status message when there is no
/// workspace to attach memory to.
fn open_store(app: &mut App) -> Option<ProjectStore> {
    match &app.cached_workspace {
        Some(ws) => Some(ProjectStore::for_workspace(&ws.root)),
        None => {
            app.set_status(
                "No project detected — project memory needs a workspace (git repo or manifest)",
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{ModelInfo, StreamEvent};
    use crate::workspace::{ProjectType, WorkspaceInfo};
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc;

    struct MockProvider;

    impl AiProvider for MockProvider {
        fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _model: &str,
            _tx: mpsc::UnboundedSender<StreamEvent>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }

        fn chat(
            &self,
            _messages: &[ChatMessage],
            _model: &str,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
            Box::pin(async { Ok("A test brief about the project.".to_string()) })
        }

        fn list_models(
            &self,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<ModelInfo>>> + Send + '_>> {
            Box::pin(async { Ok(vec![]) })
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    fn mock_provider() -> Arc<dyn AiProvider> {
        Arc::new(MockProvider)
    }

    fn app_with_workspace(root: &std::path::Path) -> App {
        let mut app = App::new();
        app.cached_workspace = Some(WorkspaceInfo {
            root: root.to_path_buf(),
            project_type: ProjectType::Rust,
            name: "test".into(),
            description: String::new(),
            key_files: vec![],
            tech_stack: vec![],
        });
        app
    }

    #[tokio::test]
    async fn remember_then_memory_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/remember uses tokio", &mock_provider()).await);
        assert!(handle(&mut app, "/memory", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("uses tokio"));
    }

    #[tokio::test]
    async fn remember_without_workspace_sets_status() {
        let mut app = App::new();
        app.cached_workspace = None;
        assert!(handle(&mut app, "/remember something", &mock_provider()).await);
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("No project detected"))
        );
    }

    #[tokio::test]
    async fn decision_and_decisions_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/decision use ratatui", &mock_provider()).await);
        assert!(handle(&mut app, "/decisions", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("use ratatui"));
    }

    #[tokio::test]
    async fn improvements_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/improve faster startup", &mock_provider()).await);
        assert!(handle(&mut app, "/improvements", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("[ ] #1 faster startup"));
        assert!(handle(&mut app, "/improvements done 1", &mock_provider()).await);
        assert!(handle(&mut app, "/improvements", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("[x] #1 faster startup"));
    }

    #[tokio::test]
    async fn init_generates_and_saves_brief() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/init", &mock_provider()).await);
        let store = ProjectStore::for_workspace(dir.path());
        assert_eq!(
            store.load_brief().unwrap(),
            "A test brief about the project."
        );
        // Brief now flows into the injected context.
        assert!(
            store
                .project_memory_context(10_000)
                .unwrap()
                .contains("A test brief")
        );
    }

    #[tokio::test]
    async fn unknown_command_not_claimed() {
        let mut app = App::new();
        assert!(!handle(&mut app, "/nonsense", &mock_provider()).await);
    }
}
