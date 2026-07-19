use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use super::*;
use crate::agent::tools::ToolCall;
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
    /// When set, records the content of every message the loop sends, so a test
    /// can assert what the model was actually charged for.
    transcript: Option<Arc<Mutex<Vec<String>>>>,
}

impl MockProvider {
    fn scripted(responses: &[&str]) -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: responses.iter().map(|s| s.to_string()).collect(),
            idx: Mutex::new(0),
            fail: false,
            nudge_seen: None,
            transcript: None,
        })
    }
    fn failing() -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: vec![],
            idx: Mutex::new(0),
            fail: true,
            nudge_seen: None,
            transcript: None,
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
            transcript: None,
        });
        (provider, flag)
    }

    /// Like `scripted`, but records every message the loop actually SENDS, so a
    /// test can assert on what the model would really have paid tokens for.
    fn scripted_recording(responses: &[&str]) -> (Arc<dyn AiProvider>, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(Self {
            responses: responses.iter().map(|s| s.to_string()).collect(),
            idx: Mutex::new(0),
            fail: false,
            nudge_seen: None,
            transcript: Some(log.clone()),
        });
        (provider, log)
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
        if let Some(log) = &self.transcript {
            let mut l = log.lock().unwrap();
            for m in messages {
                l.push(m.content.clone());
            }
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
    // Every tool round is BLOCKED before it ever runs (never proves the tool
    // layer works) → the run must flag all_tools_failed so the worker can
    // self-heal with a fresh restart. Deliberately NOT a `run_command` that
    // merely exits non-zero (e.g. `exit 1`) — a command that actually
    // executed and reported a failing build/test is proof the tool layer is
    // fine (see `tool_layer_worked`), so it must not false-fire this
    // detector. A genuinely dangerous command that gets blocked pre-exec is
    // the real signature of "no tool ever truly ran".
    let provider =
        MockProvider::scripted(&["<tool_call>tool: run_command\ncommand: rm -rf /</tool_call>"]);
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

#[tokio::test]
async fn a_failing_build_is_not_mistaken_for_a_wedged_worker() {
    // THE REGRESSION. `run_command`'s ToolResult.success is the COMMAND's exit
    // status, so an agent whose opening move is "run the tests, watch them fail,
    // fix them" produced only `success: false` results — and the detector
    // concluded the worker was wedged, requeuing the job and restarting a
    // perfectly healthy worker. A command that RAN and reported failure proves
    // the tool layer works.
    let provider =
        MockProvider::scripted(&["<tool_call>tool: run_command\ncommand: exit 1</tool_call>"]);
    let out = run_headless_agent(&provider, "m", "run failing tests", 5, 5)
        .await
        .unwrap();
    assert!(out.iterations >= 3, "needs >=3 rounds to be meaningful");
    assert!(
        !out.all_tools_failed,
        "a command that ran and merely exited non-zero must NOT look like a wedge"
    );
}

#[tokio::test]
async fn an_identical_re_read_is_collapsed_to_a_pointer() {
    // TOKEN EFFICIENCY: models re-read files they already have. The first read
    // must arrive in full; an identical repeat must collapse to a pointer, since
    // the real content is still verbatim earlier in the same conversation.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sample.txt");
    let body = "unique-marker-content ".repeat(40);
    std::fs::write(&file, &body).unwrap();
    let path = file.to_string_lossy().to_string();

    // Read the same path three times, then finish.
    let call = format!("<tool_call>tool: read_file\npath: {path}</tool_call>");
    let (provider, transcript) = MockProvider::scripted_recording(&[&call, &call, &call, "Done."]);
    let _ = run_headless_agent(&provider, "m", "read it", 25, 5)
        .await
        .unwrap();

    // The loop feeds each tool result back as its own user message, and every
    // later turn re-sends the whole history — so count DISTINCT messages, which
    // is exactly "how many copies of this file entered the conversation".
    let sent = transcript.lock().unwrap().clone();
    let distinct: std::collections::HashSet<&String> = sent.iter().collect();
    let bodies = distinct
        .iter()
        .filter(|m| m.contains("unique-marker-content"))
        .count();
    let pointers = distinct
        .iter()
        .filter(|m| m.contains("identical to your earlier read_file"))
        .count();

    // THE PROPERTY: three reads of an unchanged file put the body in context
    // ONCE. Without the dedupe this is 3 — and each copy is then re-sent on
    // every subsequent turn, which is what made re-reads so expensive.
    assert_eq!(
        bodies, 1,
        "the file body must enter the conversation exactly once, got {bodies}"
    );
    assert_eq!(
        pointers, 1,
        "the repeats must collapse to the (identical) pointer message, got {pointers}"
    );
}

#[test]
fn compaction_reaches_its_budget_even_when_the_tail_is_huge() {
    // A tool result may be up to MAX_TOOL_OUTPUT_CHARS (50k). Six of those in the
    // protected tail blow the budget on their own, and the old pass only shrank
    // messages BETWEEN head and tail — so it could return having freed nothing
    // and the very next provider call would overflow the context window and kill
    // the job. Compaction must always converge under its budget.
    let budget = 2_000;
    let huge = "y".repeat(50_000);
    let mut msgs: Vec<ChatMessage> = vec![
        ChatMessage::system("base prompt"),
        ChatMessage::system("tools prompt"),
        ChatMessage::user("THE ORIGINAL TASK"),
    ];
    // A tail of six enormous tool results, with assistant turns interleaved.
    for _ in 0..6 {
        msgs.push(ChatMessage::assistant("thinking"));
        msgs.push(ChatMessage::user(&huge));
    }

    compact_context(&mut msgs, budget);

    let total: usize = msgs
        .iter()
        .map(|m| crate::agent::context::ContextManager::estimate_tokens(&m.content))
        .sum();
    assert!(
        total <= budget,
        "compaction must get under budget, still at {total} tokens (budget {budget})"
    );
    // The task must survive regardless — never trade the goal for space.
    assert!(
        msgs.iter().any(|m| m.content == "THE ORIGINAL TASK"),
        "the original task must never be compacted away"
    );
}

#[test]
fn compact_context_reports_whether_it_stubbed() {
    // The re-read cache is dropped whenever compaction fires, so compaction MUST
    // report honestly — otherwise a model could be pointed at content that is no
    // longer in its context.
    let mut small = vec![ChatMessage::user("hi")];
    assert!(
        !compact_context(&mut small, 10),
        "nothing to compact in a tiny history"
    );

    let big = "x".repeat(80_000);
    let mut msgs: Vec<ChatMessage> = (0..12).map(|_| ChatMessage::user(&big)).collect();
    assert!(
        compact_context(&mut msgs, 100),
        "must report true when it stubs output away"
    );
}

#[test]
fn tool_layer_worked_distinguishes_ran_from_never_ran() {
    let mk = |tool: &str, success: bool, output: &str| ToolResult {
        tool: tool.into(),
        success,
        output: output.into(),
    };
    // Ran, but the command failed (a red test suite) → the layer works.
    assert!(tool_layer_worked(&mk(
        "run_command",
        false,
        "FAILED\n[completed in 0.2s]"
    )));
    // Any tool that outright succeeded → the layer works.
    assert!(tool_layer_worked(&mk("read_file", true, "contents")));
    // Never ran: refused before execution.
    assert!(!tool_layer_worked(&mk(
        "run_command",
        false,
        "Blocked: this command is potentially destructive"
    )));
    // Never completed: killed at the timeout.
    assert!(!tool_layer_worked(&mk(
        "run_command",
        false,
        "Command timed out after 30.0s (limit: 30s)"
    )));
    // A genuinely broken tool layer (what a real wedge looked like).
    assert!(!tool_layer_worked(&mk(
        "read_file",
        false,
        "Tool execution limit reached (500). Start a new session."
    )));
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
    let ctx =
        project_memory_context_from(&store, "some task text").expect("should assemble a context");
    assert!(ctx.contains("PROJECT KNOWLEDGE"));
    assert!(ctx.contains("booking assistant"));
    assert!(ctx.contains("terracotta"));
    assert!(ctx.contains("No gradients"));
}

#[test]
fn project_memory_context_is_none_without_nerve_memory() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::project::ProjectStore::for_workspace(dir.path());
    assert!(project_memory_context_from(&store, "some task text").is_none());
}

#[test]
fn project_memory_context_advertises_recall_like_the_interactive_path_does() {
    // Regression test: `recall` was called 0 times across 2,362 real tool
    // calls because headless jobs never saw `memory_recall::always_on_context`
    // — the header that tells the model the `recall` tool exists at all. The
    // advertisement line only appears once there is at least one fact/decision
    // on file (see `always_on_context`'s `fact_count` gate), so a brief alone
    // isn't enough to exercise it — remember a fact too.
    let dir = tempfile::tempdir().unwrap();
    let store = crate::project::ProjectStore::for_workspace(dir.path());
    store
        .save_brief("Vollgebucht is a booking assistant.")
        .unwrap();
    store
        .remember("Buttons use the terracotta accent.")
        .unwrap();
    let ctx =
        project_memory_context_from(&store, "some task text").expect("should assemble a context");
    assert!(ctx.contains("`recall` tool"));
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
    // iterations, get the "start IMPLEMENTING now" nudge — the token-efficiency
    // guard against endless exploration.
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

#[tokio::test]
async fn explore_nudge_escalates_when_stuck_never_editing() {
    // THE REGRESSION (job e2915f05): a trivial one-utility-class CSS fix ran
    // the FULL 40 iterations and made ZERO edits (tool census: run_command 17,
    // search_code 9, read_lines 7, read_file 6, edit_file 0, write_file 0).
    // The old nudge was a one-shot `bool` latch: it fired once at iteration 8
    // and then never again, so the agent explored the remaining 32 iterations
    // under no further pressure. A run that keeps reading and never edits must
    // now get the nudge AGAIN every `EXPLORE_NUDGE_INTERVAL` iterations, with
    // escalating (and DIFFERENT) wording each time.
    let read_call = "<tool_call>tool: read_file\npath: src/main.rs</tool_call>";
    let (provider, transcript) = MockProvider::scripted_recording(&[read_call]);
    let _ = run_headless_agent(&provider, "m", "add one utility class", 22, 5)
        .await
        .unwrap();

    let sent = transcript.lock().unwrap().clone();
    let distinct: std::collections::HashSet<&String> = sent.iter().collect();

    let first_nudge = distinct
        .iter()
        .find(|m| m.contains("start IMPLEMENTING now"))
        .copied();
    let second_nudge = distinct
        .iter()
        .find(|m| m.contains("Make your FIRST edit now"))
        .copied();
    let third_nudge = distinct
        .iter()
        .find(|m| m.contains("Stop reading. Based on what you already know"))
        .copied();

    assert!(
        first_nudge.is_some(),
        "expected the first (gentle) explore-nudge to fire around iteration 8"
    );
    assert!(
        second_nudge.is_some() || third_nudge.is_some(),
        "expected the nudge to RE-FIRE at least once more — a one-shot nudge is exactly the bug \
         job e2915f05 hit (40 iterations, 0 edits)"
    );
    if let (Some(a), Some(b)) = (first_nudge, second_nudge) {
        assert_ne!(
            a, b,
            "the re-fired nudge must be worded differently (firmer) than the first"
        );
    }
}

#[tokio::test]
async fn no_explore_nudge_once_the_agent_has_edited() {
    // Once `edited` becomes true, the explore-nudge must never fire again —
    // even if the run keeps going (and re-reading) for many more iterations.
    let (provider, transcript) = MockProvider::scripted_recording(&[
        "<tool_call>tool: read_file\npath: src/main.rs</tool_call>",
        "<tool_call>tool: run_command\ncommand: echo edited</tool_call>",
        "<tool_call>tool: read_file\npath: src/main.rs</tool_call>",
    ]);
    let _ = run_headless_agent(&provider, "m", "task", 22, 5)
        .await
        .unwrap();
    let sent = transcript.lock().unwrap().clone();
    assert!(
        !sent.iter().any(|m| m.contains("start IMPLEMENTING now")
            || m.contains("Make your FIRST edit now")
            || m.contains("Stop reading. Based on what you already know")),
        "must never nudge once the agent has already edited"
    );
}

#[tokio::test]
async fn read_only_role_never_nudged_even_when_stuck() {
    // Planner/reviewer (ReadOnly) roles legitimately finish with prose after
    // exploring — they must NEVER receive the explore-nudge, no matter how
    // many read-only iterations they run.
    let read_call = "<tool_call>tool: read_file\npath: src/main.rs</tool_call>";
    let (provider, transcript) = MockProvider::scripted_recording(&[read_call]);
    let _ = run_role(
        &provider,
        "m",
        "sys",
        ToolPolicy::ReadOnly,
        "plan the trivial css tweak",
        22,
        5,
    )
    .await
    .unwrap();
    let sent = transcript.lock().unwrap().clone();
    assert!(
        !sent.iter().any(|m| m.contains("start IMPLEMENTING now")
            || m.contains("Make your FIRST edit now")
            || m.contains("Stop reading. Based on what you already know")),
        "read-only roles must never be nudged"
    );
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
fn tool_call_hint_shows_command_for_run_command() {
    // job e2915f05's log showed "run_command" 17 times with no indication of
    // WHAT ran — this is what fixes that.
    let call = ToolCall {
        tool: "run_command".into(),
        args: [("command".to_string(), "npm run -s lint".to_string())]
            .into_iter()
            .collect(),
    };
    assert_eq!(tool_call_hint(&call), "run_command(npm run -s lint)");
}

#[test]
fn tool_call_hint_shows_path_for_file_tools() {
    let call = ToolCall {
        tool: "edit_file".into(),
        args: [("path".to_string(), "app/globals.css".to_string())]
            .into_iter()
            .collect(),
    };
    assert_eq!(tool_call_hint(&call), "edit_file(app/globals.css)");
}

#[test]
fn tool_call_hint_shows_pattern_for_search_tools() {
    let call = ToolCall {
        tool: "search_code".into(),
        args: [("pattern".to_string(), "fn foo".to_string())]
            .into_iter()
            .collect(),
    };
    assert_eq!(tool_call_hint(&call), "search_code(fn foo)");
}

#[test]
fn tool_call_hint_truncates_long_commands_and_flattens_newlines() {
    let long_cmd = format!("echo start\n{}", "x".repeat(100));
    let call = ToolCall {
        tool: "run_command".into(),
        args: [("command".to_string(), long_cmd)].into_iter().collect(),
    };
    let hint = tool_call_hint(&call);
    assert!(hint.starts_with("run_command("));
    assert!(!hint.contains('\n'), "newlines must be flattened: {hint}");
    // Not exactly 60 (smart_truncate may break at a word boundary and adds
    // "..."), but must stay short — not the whole 111-char command.
    assert!(hint.len() < 90, "hint should stay short: {hint}");
}

#[test]
fn tool_call_hint_falls_back_to_bare_name_for_unmapped_tools() {
    let call = ToolCall {
        tool: "remember".into(),
        args: Default::default(),
    };
    assert_eq!(tool_call_hint(&call), "remember");
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

#[test]
fn stuck_on_file_nudge_none_below_threshold_some_at_threshold() {
    // Exact boundary, referenced via the constant so this test still means
    // something (and still fails loudly) if STUCK_ON_FILE_THRESHOLD ever changes.
    let mut accesses = std::collections::HashMap::new();
    accesses.insert("src/remote.rs".to_string(), STUCK_ON_FILE_THRESHOLD - 1);
    assert!(
        stuck_on_file_nudge(&accesses).is_none(),
        "one below the threshold must not nudge"
    );

    accesses.insert("src/remote.rs".to_string(), STUCK_ON_FILE_THRESHOLD);
    assert!(
        stuck_on_file_nudge(&accesses).is_some(),
        "exactly at the threshold must nudge"
    );
}

#[test]
fn stuck_on_file_nudge_message_names_the_offending_path() {
    // The whole point of this nudge over the generic explore-nudge is that it
    // NAMES the file — a generic "stop exploring" nudge already existed and did
    // not help (see job 5029e587), so assert the path text actually appears.
    let mut accesses = std::collections::HashMap::new();
    accesses.insert("src/remote.rs".to_string(), STUCK_ON_FILE_THRESHOLD);
    let (path, message) = stuck_on_file_nudge(&accesses).expect("threshold crossed");
    assert_eq!(path, "src/remote.rs");
    assert!(
        message.contains("src/remote.rs"),
        "message must name the offending path: {message}"
    );
}

#[test]
fn stuck_on_file_nudge_picks_the_highest_count_when_several_are_over() {
    let mut accesses = std::collections::HashMap::new();
    accesses.insert("src/a.rs".to_string(), STUCK_ON_FILE_THRESHOLD);
    accesses.insert("src/b.rs".to_string(), STUCK_ON_FILE_THRESHOLD + 5);
    accesses.insert("src/c.rs".to_string(), STUCK_ON_FILE_THRESHOLD + 1);
    let (path, _) = stuck_on_file_nudge(&accesses).expect("at least one over threshold");
    assert_eq!(
        path, "src/b.rs",
        "must pick the single MOST-read path, not just any path over threshold"
    );
}

#[test]
fn stuck_on_file_nudge_ignores_a_path_well_under_threshold() {
    // Two different paths must not interfere with each other's counts: only the
    // one actually over the threshold is reported.
    let mut accesses = std::collections::HashMap::new();
    accesses.insert("src/hot.rs".to_string(), STUCK_ON_FILE_THRESHOLD + 2);
    accesses.insert("src/cold.rs".to_string(), 1);
    let (path, _) = stuck_on_file_nudge(&accesses).expect("hot.rs is over threshold");
    assert_eq!(path, "src/hot.rs");
}

#[test]
fn stuck_on_file_nudge_none_for_empty_map() {
    let accesses = std::collections::HashMap::new();
    assert!(stuck_on_file_nudge(&accesses).is_none());
}
