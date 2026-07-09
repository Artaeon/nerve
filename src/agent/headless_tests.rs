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
}

impl MockProvider {
    fn scripted(responses: &[&str]) -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: responses.iter().map(|s| s.to_string()).collect(),
            idx: Mutex::new(0),
            fail: false,
        })
    }
    fn failing() -> Arc<dyn AiProvider> {
        Arc::new(Self {
            responses: vec![],
            idx: Mutex::new(0),
            fail: true,
        })
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
        _messages: &[ChatMessage],
        _model: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + '_>> {
        if self.fail {
            return Box::pin(async { Err(anyhow::anyhow!("mock provider failure")) });
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
async fn propagates_provider_error() {
    let provider = MockProvider::failing();
    let err = run_headless_agent(&provider, "m", "task", 25, 5)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("mock provider failure"));
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
fn truncate_output_caps_long_output() {
    let long = "x".repeat(6000);
    let t = truncate_output(&long);
    assert!(t.contains("[Output truncated: 6000 bytes total]"));
    assert!(t.len() < 5200);
    // Short output is returned verbatim.
    assert_eq!(truncate_output("short"), "short");
}
