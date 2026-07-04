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

/// Single WORDS that warrant the strong model. Matched on word boundaries (not
/// as substrings) so e.g. "design" does not fire on "designate"/"designer".
const HEAVY_WORDS: &[&str] = &[
    "architect",
    "architecture",
    "design",
    "redesign",
    "refactor",
    "refactoring",
    "rewrite",
    "rewriting",
    "migrate",
    "migration",
    "debug",
    "debugging",
    "investigate",
    "optimize",
    "optimise",
    "optimization",
    "performance",
    "concurrency",
    "concurrent",
    "deadlock",
    "security",
    "vulnerability",
    "threadsafe",
];

/// Multi-word PHRASES that warrant the strong model. Matched as substrings —
/// safe because each phrase is distinctive enough not to false-match.
const HEAVY_PHRASES: &[&str] = &[
    "root cause",
    "race condition",
    "why does",
    "why is",
    "end to end",
    "end-to-end",
    "thread safe",
    "thread-safe",
];

/// Scope signals: the task spans many files / the whole project. These force at
/// least the strong model even when a "light" verb (rename/format/…) is present,
/// so a large cross-cutting change is never downgraded to the small model.
const SCOPE_PHRASES: &[&str] = &[
    "across",
    "codebase",
    "call site",
    "throughout",
    "every file",
    "all files",
    "each file",
    "entire ",
    "whole project",
    "multiple files",
    "everywhere",
];

/// Single WORDS that mark a trivial, mechanical edit (small context). Matched on
/// word boundaries. Only downgrade when NO scope signal is present.
const LIGHT_WORDS: &[&str] = &[
    "typo",
    "typos",
    "rename",
    "spelling",
    "whitespace",
    "reformat",
    "docstring",
    "docstrings",
    "capitalize",
    "lowercase",
    "uppercase",
    "indent",
    "indentation",
];

/// Trivial multi-word phrases (substring-matched).
const LIGHT_PHRASES: &[&str] = &[
    "add a comment",
    "add comment",
    "bump the version",
    "bump version",
    "print statement",
    "log line",
    "log message",
    "run fmt",
];

/// The set of lowercase alphanumeric words in a string, for word-boundary
/// signal matching (so short keywords can't false-match inside longer words).
fn word_set(lower: &str) -> std::collections::HashSet<&str> {
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Classify a single-turn request into a tier from its text.
///
/// Conservative and quality-first: defaults to `Standard`. Escalates to `Heavy`
/// on a word-boundary heavy keyword, a heavy phrase, OR any scope signal
/// (multi-file / whole-project work). Downgrades to `Light` ONLY for an
/// explicitly trivial, scope-free edit — length alone never downgrades, so a
/// short-but-real request ("fix the login bug") keeps the baseline model.
pub fn classify_message(text: &str) -> ModelTier {
    let lower = text.to_lowercase();
    let words = word_set(&lower);

    let has_scope = SCOPE_PHRASES.iter().any(|s| lower.contains(s));
    let is_heavy = has_scope
        || HEAVY_WORDS.iter().any(|w| words.contains(w))
        || HEAVY_PHRASES.iter().any(|p| lower.contains(p));
    if is_heavy {
        return ModelTier::Heavy;
    }

    let is_light = !has_scope
        && (LIGHT_WORDS.iter().any(|w| words.contains(w))
            || LIGHT_PHRASES.iter().any(|p| lower.contains(p)));
    if is_light {
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
        assert_eq!(
            classify_message("add a docstring to this function"),
            ModelTier::Light
        );
    }

    #[test]
    fn classify_defaults_to_standard() {
        assert_eq!(
            classify_message(
                "implement pagination for the users endpoint and return a next-page cursor"
            ),
            ModelTier::Standard
        );
        // A short-but-real request must NOT be downgraded on length alone
        // (regression: the old ≤35-char rule sent these to the weakest model).
        assert_eq!(classify_message("fix the login bug"), ModelTier::Standard);
        assert_eq!(classify_message("make the tests pass"), ModelTier::Standard);
        assert_eq!(classify_message("add rate limiting"), ModelTier::Standard);
        // "add a test" is real work, not a mechanical edit → baseline, not tiny.
        assert_eq!(classify_message("add a test"), ModelTier::Standard);
    }

    #[test]
    fn classify_scope_overrides_light_verb() {
        // A "light" verb on a cross-cutting task must escalate, not downgrade
        // (regression F1: "rename … across all files" was going to haiku).
        assert_eq!(
            classify_message(
                "rename UserService to AccountService across all 40 files and update every call site"
            ),
            ModelTier::Heavy
        );
        assert_eq!(
            classify_message("reformat every file in the codebase"),
            ModelTier::Heavy
        );
    }

    #[test]
    fn classify_heavy_words_match_on_boundaries_not_substrings() {
        // Regression F3: "design" must not fire on "designate"/"designer".
        assert_eq!(
            classify_message("designate the code owners in CODEOWNERS"),
            ModelTier::Standard
        );
        assert_eq!(
            classify_message("update the designer credits in the footer"),
            ModelTier::Standard
        );
        // But the real word still escalates.
        assert_eq!(
            classify_message("design a new caching layer"),
            ModelTier::Heavy
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
