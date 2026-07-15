use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::*;
use crate::ai::provider::{AiProvider, ModelInfo, StreamEvent};
use tokio::sync::mpsc;

/// A scripted provider: `chat` returns the next canned response each call, and
/// clamps to the last one once exhausted (so a "always calls a tool" script can
/// drive the loop to its iteration cap).
struct MockProvider {
    responses: Vec<String>,
    idx: Mutex<usize>,
    fail: bool,
    /// When set, `chat` flips this to true the moment it receives a message
    /// containing the explore-nudge phrase — letting a test assert the nudge
    /// fired without exposing the whole message history.
    nudge_seen: Option<Arc<Mutex<bool>>>,
}

impl MockProvider {
    fn scripted(responses: &[&str]) -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: responses.iter().map(|s| s.to_string()).collect(),
            idx: Mutex::new(0),
            fail: false,
            nudge_seen: None,
        })
    }
    fn failing() -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: vec![],
            idx: Mutex::new(0),
            fail: true,
            nudge_seen: None,
        })
    }
    /// Like `scripted`, but also returns a flag set true once the loop delivers
    /// the "start IMPLEMENTING now" explore-nudge.
    fn scripted_capturing(responses: &[&str]) -> (Arc<dyn AiProvider>, Arc<Mutex<bool>>) {
        let flag = Arc::new(Mutex::new(false));
        let provider = Arc::new(Self {
            responses: responses.iter().map(|s| s.to_string()).collect(),
            idx: Mutex::new(0),
            fail: false,
            nudge_seen: Some(flag.clone()),
        });
        (provider, flag)
    }
}

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
        messages: &[ChatMessage],
        _model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        if self.fail {
            return Box::pin(async { Err(anyhow::anyhow!("mock provider failure")) });
        }
        if let Some(flag) = &self.nudge_seen
            && messages
                .iter()
                .any(|m| m.content.contains("start IMPLEMENTING now"))
        {
            *flag.lock().unwrap() = true;
        }
        let resp = {
            let mut idx = self.idx.lock().unwrap();
            let i = (*idx).min(self.responses.len().saturating_sub(1));
            *idx += 1;
            self.responses.get(i).cloned().unwrap_or_default()
        };
        Box::pin(async move { Ok(resp) })
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

#[tokio::test]
async fn finishes_immediately_when_no_tool_calls() {
    let provider = MockProvider::scripted(&["All done — nothing needed changing."]);
    let out = run_headless_agent(&provider, "m", "do nothing", 25, 5)
        .await
        .unwrap();
    assert_eq!(out.iterations, 0);
    assert!(!out.edited);
    assert!(!out.hit_max_iterations);
    assert!(out.final_response.contains("All done"));
}

#[tokio::test]
async fn runs_a_readonly_tool_then_finishes() {
    // First turn calls a read-only tool; second turn has no tools → done.
    let provider = MockProvider::scripted(&[
        "<tool_call>tool: list_files\npath: .</tool_call>",
        "Listed the directory. Task complete.",
    ]);
    let out = run_headless_agent(&provider, "m", "list the files", 25, 5)
        .await
        .unwrap();
    assert_eq!(out.iterations, 1);
    assert!(!out.edited, "list_files is not a write tool");
    assert!(!out.hit_max_iterations);
    assert!(out.final_response.contains("complete"));
}

#[tokio::test]
async fn flags_edited_when_a_write_tool_runs() {
    let provider = MockProvider::scripted(&[
        "<tool_call>tool: run_command\ncommand: echo headless-test</tool_call>",
        "Ran the command. Done.",
    ]);
    let out = run_headless_agent(&provider, "m", "echo something", 25, 5)
        .await
        .unwrap();
    assert_eq!(out.iterations, 1);
    assert!(out.edited, "run_command counts as a mutating tool");
}

#[tokio::test]
async fn a_failed_write_tool_does_not_flag_edited() {
    // A mutating tool that FAILS (here: a non-zero run_command) must not set
    // `edited` — otherwise the worker runs verify on an unchanged tree and logs
    // the job as a success that changed nothing.
    let provider = MockProvider::scripted(&[
        "<tool_call>tool: run_command\ncommand: exit 7</tool_call>",
        "The command failed; nothing was changed.",
    ]);
    let out = run_headless_agent(&provider, "m", "run a failing command", 25, 5)
        .await
        .unwrap();
    assert!(
        !out.edited,
        "a failed mutating tool must not flag the run as edited"
    );
}

#[tokio::test]
async fn all_tools_failed_flags_a_wedged_environment() {
    // Every tool round fails (a non-zero run_command each time) → the run must
    // flag all_tools_failed so the worker can self-heal with a fresh restart.
    let provider =
        MockProvider::scripted(&["<tool_call>tool: run_command\ncommand: exit 1</tool_call>"]);
    let out = run_headless_agent(&provider, "m", "everything fails", 5, 5)
        .await
        .unwrap();
    assert!(out.iterations >= 3);
    assert!(!out.edited);
    assert!(
        out.all_tools_failed,
        "a run where no tool ever succeeds must flag all_tools_failed"
    );
}

#[test]
fn project_memory_context_assembles_brief_conventions_and_design() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::project::ProjectStore::for_workspace(dir.path());
    store
        .save_brief("Vollgebucht is a booking assistant.")
        .unwrap();
    store
        .remember("Buttons use the terracotta accent.")
        .unwrap();
    store
        .save_design("No gradients. Warm paper background.")
        .unwrap();
    let ctx = project_memory_context_from(&store).expect("should assemble a context");
    assert!(ctx.contains("PROJECT KNOWLEDGE"));
    assert!(ctx.contains("booking assistant"));
    assert!(ctx.contains("terracotta"));
    assert!(ctx.contains("No gradients"));
}

#[test]
fn project_memory_context_is_none_without_nerve_memory() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::project::ProjectStore::for_workspace(dir.path());
    assert!(project_memory_context_from(&store).is_none());
}

#[tokio::test]
async fn all_tools_failed_is_false_when_a_tool_succeeds() {
    // A successful read-only tool then done → not wedged.
    let provider =
        MockProvider::scripted(&["<tool_call>tool: list_files\npath: .</tool_call>", "Done."]);
    let out = run_headless_agent(&provider, "m", "list", 25, 5)
        .await
        .unwrap();
    assert!(!out.all_tools_failed);
}

#[tokio::test]
async fn a_fresh_run_resets_an_exhausted_tool_counter() {
    // THE WEDGE REGRESSION. The tool-execution counter is process-global and was
    // never reset on the headless path, so a long-lived worker eventually crossed
    // MAX_TOOL_EXECUTIONS and then failed EVERY tool call — read_file included —
    // making jobs churn to their cap writing nothing. Simulate an exhausted
    // counter and assert a fresh run still works.
    crate::agent::tools::set_tool_count_for_test(usize::MAX / 2);
    // Always emits a tool call, so the run reaches the >=3 iterations that
    // `all_tools_failed` needs to be meaningful.
    let provider = MockProvider::scripted(&["<tool_call>tool: list_files\npath: .</tool_call>"]);
    let out = run_headless_agent(&provider, "m", "list", 4, 5)
        .await
        .unwrap();
    assert!(
        !out.all_tools_failed,
        "a fresh run must reset the global tool counter — otherwise every tool fails (the wedge)"
    );
}

#[tokio::test]
async fn stops_at_iteration_cap() {
    // Always emits a tool call → the loop must stop at max_iterations.
    let provider = MockProvider::scripted(&["<tool_call>tool: list_files\npath: .</tool_call>"]);
    let out = run_headless_agent(&provider, "m", "loop forever", 3, 5)
        .await
        .unwrap();
    assert_eq!(out.iterations, 3);
    assert!(out.hit_max_iterations);
}

#[tokio::test]
async fn nudges_to_implement_after_long_read_only_exploration() {
    // A run that only ever reads (never edits) should, past EXPLORE_NUDGE_AFTER
    // iterations, get the one-time "start IMPLEMENTING now" nudge — the
    // token-efficiency guard against endless exploration.
    let (provider, nudged) =
        MockProvider::scripted_capturing(&["<tool_call>tool: list_files\npath: .</tool_call>"]);
    let _ = run_headless_agent(&provider, "m", "explore", EXPLORE_NUDGE_AFTER + 3, 5)
        .await
        .unwrap();
    assert!(
        *nudged.lock().unwrap(),
        "expected an implement-now nudge after {EXPLORE_NUDGE_AFTER} read-only iterations"
    );
}

#[tokio::test]
async fn no_explore_nudge_when_work_is_quick() {
    // A run that reads once then finishes must NOT receive the explore nudge.
    let (provider, nudged) = MockProvider::scripted_capturing(&[
        "<tool_call>tool: list_files\npath: .</tool_call>",
        "Done.",
    ]);
    let _ = run_headless_agent(&provider, "m", "quick", 25, 5)
        .await
        .unwrap();
    assert!(!*nudged.lock().unwrap(), "should not nudge a quick run");
}

#[test]
fn parse_subtasks_from_json_fence() {
    let text = "Here is my plan.\n\n```json\n[\n  {\"title\": \"Pure matcher\", \"instruction\": \"Create lib/x.ts ...\"},\n  {\"title\": \"Wire it\", \"instruction\": \"Call it from y.ts ...\"}\n]\n```";
    let subs = parse_subtasks(text);
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].0, "Pure matcher");
    assert!(subs[0].1.contains("lib/x.ts"));
    assert_eq!(subs[1].0, "Wire it");
}

#[test]
fn parse_subtasks_from_bare_array() {
    let text = "prose [ {\"title\":\"a\",\"instruction\":\"do a\"} ] trailing";
    let subs = parse_subtasks(text);
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].0, "a");
}

#[test]
fn parse_subtasks_empty_on_garbage() {
    assert!(parse_subtasks("no json here at all").is_empty());
    assert!(parse_subtasks("```json\nnot valid json\n```").is_empty());
    // Items missing an instruction are dropped.
    assert!(parse_subtasks("[{\"title\":\"x\"}]").is_empty());
}

#[test]
fn exec_agent_request_and_outcome_json_roundtrip() {
    // The subprocess boundary sends an ExecAgentRequest over stdin and a
    // HeadlessOutcome back over stdout — both must serde round-trip exactly.
    let req = ExecAgentRequest {
        task: "do the thing".into(),
        model: "sonnet".into(),
        max_iterations: 40,
        timeout: 300,
        cwd: "/srv/repo".into(),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: ExecAgentRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.task, "do the thing");
    assert_eq!(back.max_iterations, 40);
    assert_eq!(back.cwd, "/srv/repo");

    let outcome = HeadlessOutcome {
        iterations: 7,
        edited: true,
        final_response: "done".into(),
        hit_max_iterations: false,
        all_tools_failed: false,
    };
    let oj = serde_json::to_string(&outcome).unwrap();
    let ob: HeadlessOutcome = serde_json::from_str(&oj).unwrap();
    assert_eq!(ob.iterations, 7);
    assert!(ob.edited);
    assert_eq!(ob.final_response, "done");
}

#[test]
fn parse_subtasks_caps_at_max() {
    let items: Vec<String> = (0..30)
        .map(|i| format!("{{\"title\":\"t{i}\",\"instruction\":\"do {i}\"}}"))
        .collect();
    let text = format!("[{}]", items.join(","));
    assert_eq!(parse_subtasks(&text).len(), MAX_SUBTASKS);
}

#[tokio::test]
async fn propagates_provider_error() {
    let provider = MockProvider::failing();
    let err = run_headless_agent(&provider, "m", "task", 25, 5)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("mock provider failure"));
}

#[tokio::test]
async fn nudges_a_full_role_that_talks_instead_of_acting() {
    // First response is prose with no tool call — a full role must be nudged to
    // act rather than finishing in 0 iterations having done nothing.
    let provider = MockProvider::scripted(&[
        "I will add the file and wire it up.", // prose, no tool → nudge
        "<tool_call>tool: run_command\ncommand: echo did-it</tool_call>", // acts after nudge
        "Done.",
    ]);
    let out = run_role(
        &provider,
        "m",
        "sys",
        ToolPolicy::Full,
        "do the task",
        25,
        5,
    )
    .await
    .unwrap();
    assert!(out.edited, "the nudge should have driven the agent to act");
    assert_eq!(out.iterations, 1);
}

#[tokio::test]
async fn nudges_past_a_confabulated_tool_limit_and_the_agent_then_acts() {
    // The model bails by inventing a "tool execution limit for this session"
    // before doing anything — the nudge must rebut it and drive it to act.
    let provider = MockProvider::scripted(&[
        "I've hit the tool execution limit for this session and cannot continue.", // confabulated bail → nudge
        "<tool_call>tool: run_command\ncommand: echo did-it</tool_call>", // acts after the rebuttal
        "Done.",
    ]);
    let out = run_role(
        &provider,
        "m",
        "sys",
        ToolPolicy::Full,
        "do the task",
        25,
        5,
    )
    .await
    .unwrap();
    assert!(
        out.edited,
        "the rebuttal should have driven the agent to act"
    );
    assert_eq!(out.iterations, 1);
}

#[tokio::test]
async fn read_only_role_finishes_with_prose_and_is_not_nudged() {
    // A planner/reviewer legitimately finishes with prose (no tools) — no nudge.
    let provider = MockProvider::scripted(&["Here is my plan:\n1. do x\n2. do y"]);
    let out = run_role(
        &provider,
        "m",
        "sys",
        ToolPolicy::ReadOnly,
        "plan it",
        25,
        5,
    )
    .await
    .unwrap();
    assert_eq!(out.iterations, 0);
    assert!(out.final_response.contains("plan"));
}

#[tokio::test]
async fn read_only_role_blocks_write_tools() {
    // A read-only role that tries to write must be blocked — no edit happens.
    let provider = MockProvider::scripted(&[
        "<tool_call>tool: write_file\npath: nope.txt\ncontent: x</tool_call>",
        "I was blocked from writing. Done.",
    ]);
    let out = run_role(
        &provider,
        "m",
        "sys",
        ToolPolicy::ReadOnly,
        "look only",
        25,
        5,
    )
    .await
    .unwrap();
    assert!(!out.edited, "read-only role must never edit");
    assert_eq!(out.iterations, 1);
}

#[tokio::test]
async fn workflow_runs_planner_then_coder_then_reviewer() {
    // planner (no tools → plan text) → coder (run_command → edited) → reviewer (verdict).
    let provider = MockProvider::scripted(&[
        "1. Add the file\n2. Wire it up", // planner plan
        "<tool_call>tool: run_command\ncommand: echo wf-coder</tool_call>", // coder acts
        "Implemented the plan.",          // coder done
        "Looks correct.\nVERDICT: APPROVED", // reviewer
    ]);
    let wf = run_workflow(&provider, "m", "build a thing", 25, 5)
        .await
        .unwrap();
    assert!(wf.edited, "coder should have edited (run_command)");
    assert!(
        wf.plan.contains("Add the file"),
        "plan captured: {}",
        wf.plan
    );
    assert!(
        wf.review.contains("APPROVED"),
        "review captured: {}",
        wf.review
    );
    assert!(!wf.hit_max_iterations);
}

#[tokio::test]
async fn workflow_runs_a_fix_round_when_reviewer_concludes_needs_fixes() {
    let provider = MockProvider::scripted(&[
        "1. plan",                                                      // planner
        "<tool_call>tool: run_command\ncommand: echo code</tool_call>", // coder acts
        "coded it",                                                     // coder done
        "VERDICT: NEEDS FIXES: add a guard", // reviewer concludes (not cap)
        "<tool_call>tool: run_command\ncommand: echo fix</tool_call>", // fix round acts
        "fixed it",                          // fix done
    ]);
    let wf = run_workflow(&provider, "m", "task", 25, 5).await.unwrap();
    // coder (1) + fix round (1) = 2 iterations, proving the fix round ran.
    assert_eq!(wf.coder_iterations, 2, "fix round should have run");
    assert!(wf.review.contains("NEEDS FIXES"));
    assert!(wf.edited);
}

#[test]
fn first_review_line_extracts_the_verdict() {
    assert_eq!(
        first_review_line("some notes\nVERDICT: NEEDS FIXES: missing test"),
        "VERDICT: NEEDS FIXES: missing test"
    );
    // No verdict line → a short prefix, not empty.
    assert!(!first_review_line("just some prose here").is_empty());
}

#[test]
fn format_results_marks_success_and_failure() {
    let results = vec![
        ToolResult {
            tool: "read_file".into(),
            success: true,
            output: "contents".into(),
        },
        ToolResult {
            tool: "run_command".into(),
            success: false,
            output: "boom".into(),
        },
    ];
    let msg = format_results(&results);
    assert!(msg.starts_with(crate::conversation::TOOL_RESULTS_PREFIX));
    assert!(msg.contains("### Tool 1: read_file [OK]"));
    assert!(msg.contains("### Tool 2: run_command [ERROR]"));
    assert!(msg.contains("Some tools failed"));
}

#[test]
fn compact_context_leaves_short_history_untouched() {
    let mut messages = vec![
        ChatMessage::system("sys"),
        ChatMessage::system("tools"),
        ChatMessage::user("the task"),
        ChatMessage::assistant("ok"),
        ChatMessage::user("small tool result"),
    ];
    let before = messages.clone();
    compact_context(&mut messages, 1); // tiny budget, but too few messages to touch
    assert_eq!(messages.len(), before.len());
    for (a, b) in messages.iter().zip(before.iter()) {
        assert_eq!(a.content, b.content);
    }
}

#[test]
fn compact_context_stubs_old_tool_output_but_keeps_task_reasoning_and_tail() {
    let big = "FILE CONTENTS ".repeat(60); // ~800 chars, an old tool dump
    let mut messages = vec![
        ChatMessage::system("SYS-A"),              // 0 head
        ChatMessage::system("SYS-B"),              // 1 head
        ChatMessage::user("TASK-original"),        // 2 head (the task)
        ChatMessage::assistant("reasoning-old"),   // 3 compact range (assistant → kept)
        ChatMessage::user(big.clone()),            // 4 compact range (big tool result → stubbed)
        ChatMessage::assistant("reasoning-old-2"), // 5 compact range (assistant → kept)
        ChatMessage::user("recent tool result"),   // 6 tail
        ChatMessage::assistant("recent-1"),        // 7 tail
        ChatMessage::user("recent tool result 2"), // 8 tail
        ChatMessage::assistant("recent-2"),        // 9 tail
        ChatMessage::user("recent tool result 3"), // 10 tail
        ChatMessage::assistant("final"),           // 11 tail
    ];
    compact_context(&mut messages, 50); // force compaction

    // The old big tool-output (index 4) is stubbed…
    assert!(
        messages[4].content.contains("compacted"),
        "old dump not compacted"
    );
    // …but the head (task + system prompts) is preserved verbatim…
    assert_eq!(messages[0].content, "SYS-A");
    assert_eq!(messages[2].content, "TASK-original");
    // …the model's own reasoning (assistant turns) is preserved…
    assert_eq!(messages[3].content, "reasoning-old");
    assert_eq!(messages[5].content, "reasoning-old-2");
    // …and the recent tail is untouched.
    assert_eq!(messages[6].content, "recent tool result");
    assert_eq!(messages[11].content, "final");

    // Idempotent: compacting again changes nothing further.
    let snapshot: Vec<String> = messages.iter().map(|m| m.content.clone()).collect();
    compact_context(&mut messages, 50);
    let after: Vec<String> = messages.iter().map(|m| m.content.clone()).collect();
    assert_eq!(snapshot, after);
}

#[test]
fn truncate_output_caps_long_output() {
    use crate::agent::tools::fs::MAX_TOOL_OUTPUT_CHARS;
    // Output larger than the shared cap is truncated to that cap.
    let long = "x".repeat(MAX_TOOL_OUTPUT_CHARS + 1000);
    let t = truncate_output(&long);
    assert!(t.contains(&format!(
        "[Output truncated: {} bytes total]",
        MAX_TOOL_OUTPUT_CHARS + 1000
    )));
    assert!(t.chars().count() < MAX_TOOL_OUTPUT_CHARS + 200);
    // A large-but-in-cap read (e.g. a 40k file `read_file` returned) is fed back
    // WHOLE — no longer clipped to the old 5,000-char window.
    let file = "y".repeat(40_000);
    assert_eq!(truncate_output(&file), file);
    // Short output is returned verbatim.
    assert_eq!(truncate_output("short"), "short");
}
