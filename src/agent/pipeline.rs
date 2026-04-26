//! Sequential multi-agent pipeline (planner → coder → reviewer).
//!
//! Each "agent" is a role — a system prompt plus a tool-access policy. The
//! pipeline is driven by the normal streaming flow: when a role's LLM turn
//! finishes (`StreamEvent::Done` in the event loop), the pipeline advances
//! to the next step, injects a transition user message, and the next role's
//! streaming call is kicked off.
//!
//! The conversation history records everything each role emits, so the user
//! sees the plan → implementation → review sequence exactly as it happened
//! and can scroll back through it like any normal chat.

/// Access a role has to the agent tool suite.
///
/// * `None` — no tool calls at all (the role produces plain text).
/// * `ReadOnly` — tools enabled but the system prompt restricts the role
///   to read-only operations (`read_file`, `list_files`, `search_code`,
///   `find_files`, `read_lines`). The shell-level check still applies to
///   `run_command`, but in practice the reviewer prompt asks the LLM not
///   to invoke write-capable tools.
/// * `Full` — full tool access, same as normal agent mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    None,
    ReadOnly,
    Full,
}

/// A single step in a pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStep {
    Planning,
    Coding,
    Reviewing,
    Done,
}

impl PipelineStep {
    pub fn label(self) -> &'static str {
        match self {
            PipelineStep::Planning => "Planner",
            PipelineStep::Coding => "Coder",
            PipelineStep::Reviewing => "Reviewer",
            PipelineStep::Done => "Done",
        }
    }

    /// Return the role config for the current step, or `None` for Done.
    pub fn role(self) -> Option<AgentRole> {
        match self {
            PipelineStep::Planning => Some(AgentRole::planner()),
            PipelineStep::Coding => Some(AgentRole::coder()),
            PipelineStep::Reviewing => Some(AgentRole::reviewer()),
            PipelineStep::Done => None,
        }
    }

    /// Next step in the sequence, or Done if we're already there.
    pub fn next(self) -> PipelineStep {
        match self {
            PipelineStep::Planning => PipelineStep::Coding,
            PipelineStep::Coding => PipelineStep::Reviewing,
            PipelineStep::Reviewing => PipelineStep::Done,
            PipelineStep::Done => PipelineStep::Done,
        }
    }
}

/// A role definition: what the LLM is told to do, and which tools it may
/// call.
#[derive(Debug, Clone)]
pub struct AgentRole {
    /// Role display name (used by tests and reserved for future UI badges
    /// — PipelineStep::label() is what main.rs actually renders today).
    #[allow(dead_code)]
    pub name: &'static str,
    pub tool_policy: ToolPolicy,
    /// System prompt for this role. This REPLACES the active mode's
    /// system prompt for the duration of the role's turn.
    pub system_prompt: String,
    /// User-message handoff appended to the conversation before the
    /// role's turn starts, so the LLM sees "Now do X with the work
    /// above". For the first role (planner) this is the user's original
    /// task verbatim.
    pub handoff_prompt: String,
}

impl AgentRole {
    fn planner() -> Self {
        Self {
            name: "Planner",
            tool_policy: ToolPolicy::None,
            system_prompt: PLANNER_PROMPT.into(),
            handoff_prompt: String::new(), // task is supplied separately
        }
    }

    fn coder() -> Self {
        Self {
            name: "Coder",
            tool_policy: ToolPolicy::Full,
            system_prompt: CODER_PROMPT.into(),
            handoff_prompt: "The plan above has been approved. Now execute it using your tools: \
                 read the relevant files, make the edits, run the build/tests to \
                 verify your work, and report what you changed. Do not re-plan — \
                 just execute."
                .into(),
        }
    }

    fn reviewer() -> Self {
        Self {
            name: "Reviewer",
            tool_policy: ToolPolicy::ReadOnly,
            system_prompt: REVIEWER_PROMPT.into(),
            handoff_prompt: "The coder has finished implementing the plan. Review the changes \
                 made above. Use ONLY read-only tools (read_file, read_lines, \
                 search_code, list_files, find_files) to inspect the result — do \
                 NOT write, edit, or run commands. Report:\n\
                 1. Correctness — does it satisfy the original task?\n\
                 2. Issues — bugs, missed edge cases, security concerns.\n\
                 3. Polish — naming, comments, tests that should exist.\n\
                 \n\
                 End with an overall verdict: APPROVED, NEEDS FIXES, or \
                 REJECTED."
                .into(),
        }
    }
}

/// Hard cap on coder ↔ reviewer iteration rounds. The reviewer can ask
/// for fixes up to this many times before the workflow finishes
/// regardless. Prevents runaway loops where the reviewer never accepts
/// the coder's output.
pub const MAX_ITERATIONS: usize = 3;

/// Live pipeline state. Stored on `App` while a workflow is active; cleared
/// when the pipeline reaches `Done` or the user cancels.
#[derive(Debug, Clone)]
pub struct PipelineState {
    /// Current step (what the NEXT/current turn is driving).
    pub step: PipelineStep,
    /// The user's original task — preserved so we can show it in status.
    pub task: String,
    /// Number of times we've looped Reviewing → Coding because the
    /// reviewer requested fixes. Capped at `MAX_ITERATIONS`.
    pub iterations_used: usize,
}

impl PipelineState {
    pub fn new(task: String) -> Self {
        Self {
            step: PipelineStep::Planning,
            task,
            iterations_used: 0,
        }
    }

    /// Advance to the next step. No-op if already Done.
    pub fn advance(&mut self) {
        self.step = self.step.next();
    }

    /// Loop back to the Coding step from Reviewing because the reviewer
    /// asked for fixes. Increments `iterations_used` and returns the
    /// new step. Caller is responsible for checking the iteration cap
    /// before calling this — see `should_iterate_on_feedback`.
    pub fn iterate_back_to_coding(&mut self) {
        self.iterations_used += 1;
        self.step = PipelineStep::Coding;
    }

    /// True once the pipeline has walked off the end of the sequence.
    /// Kept as a named helper (rather than inlining `step == Done`)
    /// because the meaning "workflow is finished" is referenced in
    /// tests and is a more stable API for future UI code.
    #[allow(dead_code)]
    pub fn is_done(&self) -> bool {
        self.step == PipelineStep::Done
    }
}

/// Decide whether the workflow should loop back to the coder based on
/// the reviewer's final response.
///
/// The reviewer's system prompt asks it to end with one of three
/// verdict lines. We look for the verdict pattern (case-insensitive)
/// rather than parsing the full response, so a chatty reviewer that
/// adds extra context after the verdict still works.
///
/// Returns true only if the response contains "NEEDS FIXES" verdict
/// AND we still have iteration budget. APPROVED, REJECTED, or any
/// missing/malformed verdict completes the workflow.
pub fn should_iterate_on_feedback(reviewer_response: &str, iterations_used: usize) -> bool {
    if iterations_used >= MAX_ITERATIONS {
        return false;
    }
    let upper = reviewer_response.to_uppercase();
    // Must mention NEEDS FIXES AND not also REJECTED (in case the
    // reviewer wrote both for some reason — the stricter verdict wins).
    upper.contains("NEEDS FIXES") && !upper.contains("REJECTED")
}

// ─── Role system prompts ─────────────────────────────────────────────────

const PLANNER_PROMPT: &str = "\
You are the PLANNER in a 3-agent software-engineering pipeline. A coder \
and a reviewer will follow you. Your ONLY output is a concrete, numbered \
implementation plan for the user's task.

Rules:
- Do NOT write code. Do NOT run tools. You have no tool access.
- Do NOT ask clarifying questions — the coder will handle ambiguity.
- Produce a numbered list of 3 to 10 concrete steps, in execution order.
- Each step must be a specific action (e.g. \"Add a `--json` flag to \
  `src/cli.rs`, wired to a new `Cli::json_output` field\"), not a category.
- If the task is trivial (one edit in one file), still output a plan — \
  that single step and any verification step.
- End your response with a single line: `---END PLAN---` so downstream \
  agents can parse the boundary cleanly.";

const CODER_PROMPT: &str = "\
You are the CODER in a 3-agent software-engineering pipeline. The \
PLANNER has produced a plan above; a REVIEWER will inspect your work \
afterwards.

You have full tool access (read_file, write_file, edit_file, run_command, \
list_files, search_code, create_directory, find_files, read_lines, \
web_search). Use them to execute the plan.

Rules:
- Follow the plan's steps in order. If a step becomes impossible \
  (compile error, missing dep), adjust only that step and note the \
  deviation in your final summary.
- Prefer `edit_file` over `write_file` for existing files. Never \
  overwrite a file you haven't read.
- Run build/test commands after meaningful changes to catch regressions \
  early (`cargo build`, `cargo test`, `npm test`, etc. — pick what fits \
  the project).
- Keep changes minimal and focused on the plan. No drive-by refactors.
- When finished, emit a short summary: which steps you completed, which \
  files changed, and any noteworthy deviations.";

const REVIEWER_PROMPT: &str = "\
You are the REVIEWER in a 3-agent software-engineering pipeline. The \
PLANNER produced a plan and the CODER implemented it. Your job is to \
inspect the result.

You have READ-ONLY tool access: use `read_file`, `read_lines`, \
`search_code`, `list_files`, and `find_files` ONLY. You MUST NOT use \
`write_file`, `edit_file`, `run_command`, or `create_directory` — if you \
need to verify something that requires running code, note it as a \
follow-up rather than executing it.

Rules:
- Read the files the coder changed. Cross-check against the plan and \
  the user's original task.
- Flag: missing steps, incorrect logic, security issues, missing tests, \
  poor error handling, inconsistent style.
- Do NOT rewrite the code. Report findings only.
- Be specific — cite file:line when possible.
- End with one of these verdicts on its own line:
  - `VERDICT: APPROVED` — ready to use
  - `VERDICT: NEEDS FIXES` — minor issues listed above
  - `VERDICT: REJECTED` — fundamental problems, start over";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_progression() {
        let mut state = PipelineState::new("task".into());
        assert_eq!(state.step, PipelineStep::Planning);
        state.advance();
        assert_eq!(state.step, PipelineStep::Coding);
        state.advance();
        assert_eq!(state.step, PipelineStep::Reviewing);
        state.advance();
        assert_eq!(state.step, PipelineStep::Done);
        assert!(state.is_done());
        state.advance(); // idempotent
        assert_eq!(state.step, PipelineStep::Done);
    }

    #[test]
    fn role_per_step() {
        assert_eq!(PipelineStep::Planning.role().unwrap().name, "Planner");
        assert_eq!(PipelineStep::Coding.role().unwrap().name, "Coder");
        assert_eq!(PipelineStep::Reviewing.role().unwrap().name, "Reviewer");
        assert!(PipelineStep::Done.role().is_none());
    }

    #[test]
    fn tool_policy_per_role() {
        assert_eq!(
            PipelineStep::Planning.role().unwrap().tool_policy,
            ToolPolicy::None
        );
        assert_eq!(
            PipelineStep::Coding.role().unwrap().tool_policy,
            ToolPolicy::Full
        );
        assert_eq!(
            PipelineStep::Reviewing.role().unwrap().tool_policy,
            ToolPolicy::ReadOnly
        );
    }

    #[test]
    fn labels() {
        assert_eq!(PipelineStep::Planning.label(), "Planner");
        assert_eq!(PipelineStep::Coding.label(), "Coder");
        assert_eq!(PipelineStep::Reviewing.label(), "Reviewer");
        assert_eq!(PipelineStep::Done.label(), "Done");
    }

    #[test]
    fn coder_and_reviewer_have_handoff_prompts() {
        // The planner doesn't need a handoff — the task comes from the user.
        assert!(
            PipelineStep::Planning
                .role()
                .unwrap()
                .handoff_prompt
                .is_empty()
        );
        assert!(
            !PipelineStep::Coding
                .role()
                .unwrap()
                .handoff_prompt
                .is_empty()
        );
        assert!(
            !PipelineStep::Reviewing
                .role()
                .unwrap()
                .handoff_prompt
                .is_empty()
        );
    }

    #[test]
    fn reviewer_prompt_mentions_read_only_tools() {
        let prompt = &PipelineStep::Reviewing.role().unwrap().system_prompt;
        // Should explicitly forbid write tools.
        assert!(prompt.contains("write_file"));
        assert!(prompt.contains("MUST NOT"));
    }

    #[test]
    fn planner_has_no_tools() {
        let role = PipelineStep::Planning.role().unwrap();
        assert_eq!(role.tool_policy, ToolPolicy::None);
        assert!(role.system_prompt.contains("no tool access"));
    }

    // ── Iteration loop ──────────────────────────────────────────────

    #[test]
    fn should_iterate_on_needs_fixes() {
        let response = "Some review notes...\nVERDICT: NEEDS FIXES";
        assert!(should_iterate_on_feedback(response, 0));
        assert!(should_iterate_on_feedback(response, MAX_ITERATIONS - 1));
    }

    #[test]
    fn should_not_iterate_on_approved() {
        let response = "Looks good.\nVERDICT: APPROVED";
        assert!(!should_iterate_on_feedback(response, 0));
    }

    #[test]
    fn should_not_iterate_on_rejected() {
        let response = "Fundamental problems.\nVERDICT: REJECTED";
        assert!(!should_iterate_on_feedback(response, 0));
    }

    #[test]
    fn should_not_iterate_when_at_iteration_cap() {
        let response = "VERDICT: NEEDS FIXES";
        assert!(!should_iterate_on_feedback(response, MAX_ITERATIONS));
        assert!(!should_iterate_on_feedback(response, MAX_ITERATIONS + 5));
    }

    #[test]
    fn should_not_iterate_on_missing_verdict() {
        // Reviewer that ignored the verdict format completes the
        // workflow rather than looping. User can /workflow again if
        // unhappy.
        assert!(!should_iterate_on_feedback("Just some prose.", 0));
        assert!(!should_iterate_on_feedback("", 0));
    }

    #[test]
    fn should_not_iterate_when_both_needs_fixes_and_rejected_appear() {
        // Stricter verdict wins; if reviewer wrote both, don't loop.
        let response = "NEEDS FIXES somewhere... actually REJECTED";
        assert!(!should_iterate_on_feedback(response, 0));
    }

    #[test]
    fn should_iterate_is_case_insensitive() {
        assert!(should_iterate_on_feedback("verdict: needs fixes", 0));
        assert!(should_iterate_on_feedback("Verdict: Needs Fixes", 0));
    }

    #[test]
    fn iterate_back_to_coding_increments_counter() {
        let mut state = PipelineState::new("t".into());
        state.step = PipelineStep::Reviewing;
        assert_eq!(state.iterations_used, 0);
        state.iterate_back_to_coding();
        assert_eq!(state.step, PipelineStep::Coding);
        assert_eq!(state.iterations_used, 1);
        state.iterate_back_to_coding();
        assert_eq!(state.iterations_used, 2);
    }

    #[test]
    fn pipeline_state_starts_with_zero_iterations() {
        let state = PipelineState::new("t".into());
        assert_eq!(state.iterations_used, 0);
    }
}
