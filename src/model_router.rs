//! Prompt- and role-driven model routing.
//!
//! Nerve runs a *high* model where thinking pays off (planning, review) and a
//! *small* one where it doesn't (trivial edits, short asks), instead of paying
//! for the top model on every turn. Routing is **relative to the model the user
//! picked** so it never surprises: with the default `sonnet` baseline, planning
//! escalates to `opus` and a one-line rename drops to `haiku`; if the user pins
//! `opus`, that becomes the ceiling and routing only ever moves *down* from it.
//!
//! Only providers with a known capability ladder are routed (Claude, OpenAI);
//! for everything else routing is a no-op and the baseline model is used as-is.

use crate::agent::pipeline::PipelineStep;

/// A relative capability tier, resolved against the user's baseline model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    /// One rung down — small/cheap, for trivial work.
    Light,
    /// The user's chosen baseline.
    Standard,
    /// One rung up — the strong model, for planning/review/hard problems.
    Heavy,
}

/// Capability ladder for a provider, cheapest → strongest. Empty when we can't
/// confidently tier the provider's models (routing then no-ops).
fn ladder(provider: &str) -> &'static [&'static str] {
    match provider {
        "claude_code" | "claude" => &["haiku", "sonnet", "opus"],
        "openai" => &["gpt-4o-mini", "gpt-4o"],
        _ => &[],
    }
}

/// Resolve a tier to a concrete model name for `provider`, relative to the
/// `baseline` model the user selected. Falls back to `baseline` unchanged when
/// the provider has no ladder or the baseline isn't on it (an unknown baseline
/// means we can't reason about "one up / one down", so we leave it alone).
pub fn resolve_model(provider: &str, tier: ModelTier, baseline: &str) -> String {
    let ladder = ladder(provider);
    if ladder.is_empty() {
        return baseline.to_string();
    }
    let Some(idx) = ladder.iter().position(|m| *m == baseline) else {
        return baseline.to_string();
    };
    let target = match tier {
        ModelTier::Light => idx.saturating_sub(1),
        ModelTier::Standard => idx,
        ModelTier::Heavy => (idx + 1).min(ladder.len() - 1),
    };
    ladder[target].to_string()
}

/// The tier a pipeline role should run at: planning and review get the strong
/// model, coding runs at the baseline. Steps that run no LLM turn return `None`.
pub fn tier_for_step(step: PipelineStep) -> Option<ModelTier> {
    match step {
        PipelineStep::Planning => Some(ModelTier::Heavy),
        PipelineStep::Reviewing => Some(ModelTier::Heavy),
        PipelineStep::Coding => Some(ModelTier::Standard),
        PipelineStep::AwaitingApproval | PipelineStep::Done => None,
    }
}

/// Signals that a single-turn request is hard enough to warrant the strong
/// model — design, cross-cutting change, or genuine investigation.
const HEAVY_SIGNALS: &[&str] = &[
    "architect",
    "architecture",
    "design",
    "refactor",
    "redesign",
    "rewrite",
    "migrate",
    "debug",
    "investigate",
    "root cause",
    "why does",
    "why is",
    "optimize",
    "optimise",
    "performance",
    "concurren",
    "race condition",
    "deadlock",
    "security",
    "vulnerab",
    "across the",
    "multiple files",
    "whole codebase",
    "entire codebase",
    "end to end",
    "end-to-end",
];

/// Signals that a request is trivial enough for the small model — mechanical,
/// localized edits with tiny context.
const LIGHT_SIGNALS: &[&str] = &[
    "typo",
    "rename",
    "spelling",
    "whitespace",
    "format ",
    "reformat",
    "run fmt",
    "add a comment",
    "add comment",
    "docstring",
    "bump the version",
    "bump version",
    "print statement",
    "log line",
    "capitalize",
    "lowercase",
    "uppercase",
];

/// Longest single-turn message (chars) still eligible for the small model when
/// it carries no complexity signal. Deliberately tight: only a genuinely terse
/// instruction downgrades on length alone, so a normal medium request keeps the
/// baseline model rather than being quietly starved.
const SHORT_MESSAGE_CHARS: usize = 35;

/// Classify a single-turn request into a tier from its text. Conservative:
/// defaults to `Standard`, only escalating or downgrading on clear signals so a
/// normal request is never quietly starved of capability.
pub fn classify_message(text: &str) -> ModelTier {
    let lower = text.to_lowercase();
    if HEAVY_SIGNALS.iter().any(|s| lower.contains(s)) {
        return ModelTier::Heavy;
    }
    let has_light_signal = LIGHT_SIGNALS.iter().any(|s| lower.contains(s));
    let is_short = text.trim().chars().count() <= SHORT_MESSAGE_CHARS;
    if has_light_signal || is_short {
        return ModelTier::Light;
    }
    ModelTier::Standard
}

/// The model a turn should stream with, given the full context.
///
/// Precedence: routing disabled → baseline unchanged; an active pipeline role →
/// that role's tier; otherwise the single-turn message's classified tier.
pub fn route(
    routing_enabled: bool,
    provider: &str,
    baseline: &str,
    pipeline_step: Option<PipelineStep>,
    message: &str,
) -> String {
    if !routing_enabled {
        return baseline.to_string();
    }
    if let Some(tier) = pipeline_step.and_then(tier_for_step) {
        return resolve_model(provider, tier, baseline);
    }
    resolve_model(provider, classify_message(message), baseline)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_moves_relative_to_baseline() {
        // Default sonnet baseline: down to haiku, up to opus.
        assert_eq!(
            resolve_model("claude_code", ModelTier::Light, "sonnet"),
            "haiku"
        );
        assert_eq!(
            resolve_model("claude_code", ModelTier::Standard, "sonnet"),
            "sonnet"
        );
        assert_eq!(
            resolve_model("claude_code", ModelTier::Heavy, "sonnet"),
            "opus"
        );
    }

    #[test]
    fn resolve_respects_a_pinned_ceiling() {
        // Pin opus: Heavy can't go higher, Light steps down to sonnet.
        assert_eq!(resolve_model("claude", ModelTier::Heavy, "opus"), "opus");
        assert_eq!(resolve_model("claude", ModelTier::Light, "opus"), "sonnet");
        // Pin haiku: Light can't go lower.
        assert_eq!(resolve_model("claude", ModelTier::Light, "haiku"), "haiku");
        assert_eq!(resolve_model("claude", ModelTier::Heavy, "haiku"), "sonnet");
    }

    #[test]
    fn resolve_openai_ladder() {
        assert_eq!(
            resolve_model("openai", ModelTier::Light, "gpt-4o"),
            "gpt-4o-mini"
        );
        assert_eq!(
            resolve_model("openai", ModelTier::Heavy, "gpt-4o-mini"),
            "gpt-4o"
        );
    }

    #[test]
    fn resolve_noop_for_unknown_provider_or_baseline() {
        // Provider with no ladder.
        assert_eq!(
            resolve_model("ollama", ModelTier::Heavy, "llama3"),
            "llama3"
        );
        // Baseline not on the ladder — leave it alone rather than guess.
        assert_eq!(
            resolve_model("claude_code", ModelTier::Heavy, "some-custom-model"),
            "some-custom-model"
        );
    }

    #[test]
    fn pipeline_roles_map_to_tiers() {
        assert_eq!(
            tier_for_step(PipelineStep::Planning),
            Some(ModelTier::Heavy)
        );
        assert_eq!(
            tier_for_step(PipelineStep::Reviewing),
            Some(ModelTier::Heavy)
        );
        assert_eq!(
            tier_for_step(PipelineStep::Coding),
            Some(ModelTier::Standard)
        );
        assert_eq!(tier_for_step(PipelineStep::AwaitingApproval), None);
        assert_eq!(tier_for_step(PipelineStep::Done), None);
    }

    #[test]
    fn classify_escalates_hard_work() {
        assert_eq!(
            classify_message("refactor the auth layer to remove the race condition"),
            ModelTier::Heavy
        );
        assert_eq!(
            classify_message("investigate why the streaming deadlocks under load"),
            ModelTier::Heavy
        );
    }

    #[test]
    fn classify_downgrades_trivial_work() {
        assert_eq!(
            classify_message("fix the typo in the readme"),
            ModelTier::Light
        );
        assert_eq!(classify_message("rename foo to bar"), ModelTier::Light);
        // Very short asks are treated as light even without a keyword.
        assert_eq!(classify_message("add a test"), ModelTier::Light);
    }

    #[test]
    fn classify_defaults_to_standard() {
        assert_eq!(
            classify_message(
                "implement pagination for the users endpoint and return a next-page cursor"
            ),
            ModelTier::Standard
        );
    }

    #[test]
    fn route_precedence_disabled_pipeline_message() {
        // Disabled → baseline no matter what.
        assert_eq!(
            route(
                false,
                "claude_code",
                "sonnet",
                Some(PipelineStep::Planning),
                "anything"
            ),
            "sonnet"
        );
        // Pipeline role wins over message classification.
        assert_eq!(
            route(
                true,
                "claude_code",
                "sonnet",
                Some(PipelineStep::Planning),
                "fix a typo"
            ),
            "opus"
        );
        // No pipeline → message classification.
        assert_eq!(
            route(
                true,
                "claude_code",
                "sonnet",
                None,
                "fix the typo in the docs"
            ),
            "haiku"
        );
        assert_eq!(
            route(
                true,
                "claude_code",
                "sonnet",
                None,
                "refactor the whole codebase"
            ),
            "opus"
        );
    }
}
