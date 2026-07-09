//! Project-memory commands: `/init`, `/remember`, `/memory`, `/decision`,
//! `/decisions`, `/changes`, `/activity`, `/design`, `/design-check`,
//! `/improve`, `/improvements`, `/task`, `/tasks`.
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
        "/changes" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let changes = store.recent_changes(20);
            if changes.is_empty() {
                app.set_status("No changes journaled yet — the agent's writes are recorded here");
                return true;
            }
            let mut out = String::from("## Change journal (most recent 20)\n\n");
            for c in &changes {
                let ts = format_change_timestamp(&c.timestamp);
                out.push_str(&format!("- [{ts}] {} {} — {}\n", c.tool, c.path, c.summary));
            }
            app.add_assistant_message(out);
            true
        }
        "/activity" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let activity = store.recent_activity(20);
            if activity.is_empty() {
                app.set_status(
                    "No activity captured yet — completed agent turns are recorded here",
                );
                return true;
            }
            let mut out = String::from("## Recent activity (most recent 20)\n\n");
            for a in &activity {
                let ts = format_change_timestamp(&a.timestamp);
                let edited = if a.edited { "edited" } else { "no edits" };
                out.push_str(&format!(
                    "- [{ts}] {} ({edited}, verify: {})\n",
                    a.request, a.verify
                ));
            }
            app.add_assistant_message(out);
            true
        }
        "/design" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            if args.is_empty() {
                match store.load_design() {
                    Some(design) => app.add_assistant_message(design),
                    None => app.set_status(
                        "No design principles yet — /design <principle> to add one \
                         (auto-applied to UI/design work)",
                    ),
                }
                return true;
            }
            match store.append_design(args) {
                Ok(()) => app.set_status(format!(
                    "Design principle saved ({} — auto-applied to UI/design turns)",
                    store.design_path().display()
                )),
                Err(e) => app.set_status(format!("Could not save design principle: {e}")),
            }
            true
        }
        "/design-check" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let root = app
                .cached_workspace
                .as_ref()
                .map(|ws| ws.root.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("."));

            // Resolve the target: the given arg, else the project's main
            // stylesheet in a conventional search order.
            let target = if args.is_empty() {
                let candidates = [
                    "app/globals.css",
                    "src/app.css",
                    "styles/globals.css",
                    "src/index.css",
                ];
                match candidates.iter().find(|c| root.join(c).is_file()) {
                    Some(found) => root.join(found),
                    None => {
                        app.set_status(
                            "No stylesheet found — pass a path: /design-check <file.css>",
                        );
                        return true;
                    }
                }
            } else {
                let p = std::path::Path::new(args);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    root.join(p)
                }
            };

            let content = match std::fs::read_to_string(&target) {
                Ok(c) => c,
                Err(e) => {
                    app.set_status(format!("Could not read {}: {e}", target.display()));
                    return true;
                }
            };

            let principles = store.load_design();
            let findings = crate::design::lint_design(
                &target.to_string_lossy(),
                &content,
                principles.as_deref(),
            );

            if findings.is_empty() {
                app.add_assistant_message(format!(
                    "No design issues found in {}.",
                    target.display()
                ));
            } else {
                let path = target.display();
                let mut out = format!(
                    "## Design check — {} issue(s) in {path}\n\n",
                    findings.len()
                );
                for f in &findings {
                    out.push_str(&format!("{path}:{}  [{}]  {}\n", f.line, f.rule, f.message));
                }
                app.add_assistant_message(out);
            }
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
        "/task" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            if args.is_empty() {
                app.set_status("Usage: /task <title>, or /task done|start|fail <id>");
                return true;
            }
            // "/task done|start|fail <id>" updates a status — check BEFORE
            // treating the args as a new task title.
            let (verb, rest) = match args.find(char::is_whitespace) {
                Some(pos) => (&args[..pos], args[pos..].trim()),
                None => (args, ""),
            };
            let status = match verb {
                "done" => Some("done"),
                "start" => Some("in_progress"),
                "fail" => Some("failed"),
                _ => None,
            };
            if let Some(status) = status {
                match rest.parse::<u64>() {
                    Ok(id) => match store.set_task_status(id, status) {
                        Ok(true) => app.set_status(format!("Task #{id} marked {status}")),
                        Ok(false) => app.set_status(format!("No task with id {id}")),
                        Err(e) => app.set_status(format!("Could not update task: {e}")),
                    },
                    Err(_) => app.set_status(format!("Usage: /task {verb} <id>")),
                }
                return true;
            }
            match store.add_task(args) {
                Ok(id) => app.set_status(format!("Task #{id} added — /tasks to list")),
                Err(e) => app.set_status(format!("Could not add task: {e}")),
            }
            true
        }
        "/tasks" => {
            let Some(store) = open_store(app) else {
                return true;
            };
            let tasks = store.list_tasks();
            if tasks.is_empty() {
                app.set_status("Task backlog is empty — /task <title> to add one");
                return true;
            }
            let marker = |status: &str| match status {
                "in_progress" => "[~]",
                "done" => "[x]",
                "failed" => "[!]",
                _ => "[ ]",
            };
            let open = tasks
                .iter()
                .filter(|t| t.status == "pending" || t.status == "in_progress");
            let closed = tasks
                .iter()
                .filter(|t| t.status != "pending" && t.status != "in_progress");
            let mut out = String::from("## Task backlog\n\n");
            for t in open.chain(closed) {
                out.push_str(&format!("- {} #{} {}\n", marker(&t.status), t.id, t.title));
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

/// Render an RFC 3339 timestamp as "YYYY-MM-DD HH:MM" for journal listings.
/// Falls back to the raw string when it isn't the expected simple shape.
fn format_change_timestamp(ts: &str) -> String {
    let spaced = ts.replacen('T', " ", 1);
    if spaced.len() >= 16 && spaced.is_char_boundary(16) {
        spaced[..16].to_string()
    } else {
        spaced
    }
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
    async fn changes_lists_journal_entries() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        let store = ProjectStore::for_workspace(dir.path());
        store
            .record_change("write_file", "src/lib.rs", "wrote 42 bytes")
            .unwrap();
        assert!(handle(&mut app, "/changes", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("## Change journal"));
        assert!(content.contains("write_file src/lib.rs — wrote 42 bytes"));
        // Timestamp is rendered as "YYYY-MM-DD HH:MM" (no 'T', no seconds).
        assert!(!content.contains('T') || !content.contains("+00:00"));
    }

    #[tokio::test]
    async fn changes_empty_sets_status() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/changes", &mock_provider()).await);
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("No changes journaled yet"))
        );
    }

    #[test]
    fn change_timestamp_is_truncated_to_minutes() {
        assert_eq!(
            super::format_change_timestamp("2026-07-04T12:34:56.789Z"),
            "2026-07-04 12:34"
        );
        // Unexpected shapes fall back to the (de-T'd) raw string.
        assert_eq!(super::format_change_timestamp("bogus"), "bogus");
    }

    #[tokio::test]
    async fn design_add_then_show_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(
            handle(
                &mut app,
                "/design use an 8px spacing scale",
                &mock_provider()
            )
            .await
        );
        assert!(handle(&mut app, "/design", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("# Design principles"));
        assert!(content.contains("use an 8px spacing scale"));
    }

    #[tokio::test]
    async fn design_empty_sets_status() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/design", &mock_provider()).await);
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("No design principles yet"))
        );
    }

    #[tokio::test]
    async fn design_check_flags_and_reports() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        let file = dir.path().join("app.css");
        std::fs::write(&file, ".a { padding: 13px; }").unwrap();
        assert!(
            handle(
                &mut app,
                &format!("/design-check {}", file.display()),
                &mock_provider()
            )
            .await
        );
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("Design check"));
        assert!(content.contains("[off-grid-spacing]"));
        assert!(content.contains("13px"));
    }

    #[tokio::test]
    async fn design_check_clean_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        let file = dir.path().join("app.css");
        std::fs::write(&file, ".a { padding: 16px; }").unwrap();
        assert!(
            handle(
                &mut app,
                &format!("/design-check {}", file.display()),
                &mock_provider()
            )
            .await
        );
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("No design issues found"));
    }

    #[tokio::test]
    async fn design_check_missing_file_sets_status() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(
            handle(
                &mut app,
                "/design-check does-not-exist.css",
                &mock_provider()
            )
            .await
        );
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("Could not read"))
        );
    }

    #[tokio::test]
    async fn design_check_no_arg_no_stylesheet_sets_status() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/design-check", &mock_provider()).await);
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("No stylesheet found"))
        );
    }

    #[tokio::test]
    async fn design_check_honors_principles() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        // Record a principle forbidding gradients, then lint a file using one.
        assert!(
            handle(
                &mut app,
                "/design no gradients, use flat fills",
                &mock_provider()
            )
            .await
        );
        let file = dir.path().join("app.css");
        std::fs::write(&file, ".a { background: linear-gradient(#fff, #000); }").unwrap();
        assert!(
            handle(
                &mut app,
                &format!("/design-check {}", file.display()),
                &mock_provider()
            )
            .await
        );
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("[gradients]"), "got: {content}");
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
    async fn task_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/task ship the release", &mock_provider()).await);
        assert!(handle(&mut app, "/task fix the flaky test", &mock_provider()).await);
        assert!(handle(&mut app, "/tasks", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("## Task backlog"));
        assert!(content.contains("[ ] #1 ship the release"));
        assert!(content.contains("[ ] #2 fix the flaky test"));

        assert!(handle(&mut app, "/task start 1", &mock_provider()).await);
        assert!(handle(&mut app, "/task done 2", &mock_provider()).await);
        assert!(handle(&mut app, "/tasks", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("[~] #1 ship the release"));
        assert!(content.contains("[x] #2 fix the flaky test"));
        // Open tasks are listed before closed ones.
        assert!(content.find("#1").unwrap() < content.find("#2").unwrap());

        assert!(handle(&mut app, "/task fail 1", &mock_provider()).await);
        assert!(handle(&mut app, "/tasks", &mock_provider()).await);
        let (_role, content) = app.current_conversation().messages.last().unwrap();
        assert!(content.contains("[!] #1 ship the release"));
    }

    #[tokio::test]
    async fn task_status_verbs_are_not_titles() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_workspace(dir.path());
        assert!(handle(&mut app, "/task done 42", &mock_provider()).await);
        // No task was created; the unknown id is reported instead.
        assert!(
            ProjectStore::for_workspace(dir.path())
                .list_tasks()
                .is_empty()
        );
        assert!(
            app.status_message
                .as_ref()
                .is_some_and(|s| s.contains("No task with id 42"))
        );
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
        // Brief now flows into the always-on injected header.
        assert!(
            crate::memory_recall::always_on_context(&store, 1200)
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
