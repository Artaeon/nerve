# Determinism & context management in nerve

**Goal:** the same job, run twice, produces the same good result — and no
context is ever lost between runs, sessions, or machines. This document is the
honest account of *how far* nerve gets there today, *which levers* exist, and
*what is inherently out of our control*.

Last updated: 2026-07-12.

---

## 1. The three layers of (non-)determinism

Determinism in an LLM coding agent has three layers. nerve controls two of them
fully and the third only as far as the model/provider allows.

| Layer | What it is | Who controls it | nerve's stance |
|-------|-----------|-----------------|----------------|
| **Harness** | The loop: prompt framing, tool parsing, when a run is "done", verify/retry, context handling | nerve | **Deterministic** — same inputs → same control flow. Unit-tested. |
| **Sampling** | Temperature / top-p / seed used to draw each token | The provider | **Pinned where possible** (see §3). |
| **Model** | The weights themselves + provider-side batching/routing | The provider vendor | **Out of our hands.** Even at temperature 0, hosted models are not bit-exact run-to-run. |

The practical target is therefore **outcome determinism**, not token-exactness:
regardless of the exact tokens, the run should reach the same *correct, verified*
end state. The harness is built to funnel variance toward that fixed point.

---

## 2. Harness-level guards (the part we fully control)

These are the mechanisms that make two runs converge even when the tokens differ.
All are unit-tested in `src/agent/headless_tests.rs` and `src/worker.rs`.

- **The nudge** (`headless.rs`, `MAX_NUDGES = 2`). A full-tool agent that replies
  with prose ("here's my plan…") instead of a `<tool_call>` is nudged to *act*,
  up to twice, before the run is accepted as done. This closed the single biggest
  source of "built nothing" non-determinism: the same task previously did nothing
  twice, then succeeded — purely because the model sometimes narrated instead of
  editing. Read-only roles (planner/reviewer) are *not* nudged; finishing with
  prose is correct for them.
- **The verify gate** (`verify.rs`, `worker.rs`). After any edit, nerve runs the
  project's verify command (`cargo check` / `npm run typecheck|lint`) and feeds a
  failure back to the agent to self-correct, up to `MAX_VERIFY_ROUNDS = 2`. Two
  runs that take different paths still both have to end in a **passing** state —
  so the *verified* outcome is stable even when the diff isn't identical.
- **Reviewer-cap gating.** A reviewer that hit its iteration cap no longer emits a
  bogus "NEEDS FIXES" that triggers a pointless fix round; the fix round is gated
  on `!reviewer.hit_max_iterations`.
- **Decisive system prompt.** The headless `AGENT_SYSTEM` prompt tells the model
  to stop exploring and start writing once it understands the pattern — reducing
  the variance that comes from open-ended "let me read 20 more files" wandering.
- **Bounded, structured context** (see §4) — the same job sees the same shape of
  history regardless of how long it ran, so late iterations don't drift.

---

## 3. Sampling: pinned where the provider allows it

- **OpenAI-compatible providers** (`openai`, `ollama`, `openrouter`, custom):
  temperature and top-p are honored from config (`apply_sampling` in
  `provider_setup.rs`). **Unattended worker runs now default `temperature` to 0**
  (`default_deterministic_sampling` in `worker.rs`) so a background job is
  reproducible by default — a worker exists for reproducibility, not creativity.
  An operator-set `config.temperature` is always respected.
- **`claude_code` CLI provider** (the default on the reference server):
  **the `claude` CLI exposes no `--temperature`, `--seed`, or `--top-p` flag**
  (verified: `claude --help` has none). We therefore *cannot* pin sampling on
  this provider. Its residual non-determinism is **inherent** — the honest bottom
  line is that on `claude_code`, determinism lives entirely at the harness layer
  (§2), not the sampling layer.

**Honest caveat:** even with temperature 0 on an OpenAI-compatible endpoint,
hosted inference is not bit-exact (floating-point non-associativity across batch
sizes, MoE routing, vendor-side changes). Temperature 0 sharply *reduces*
variance; it does not eliminate it. For true bit-exactness you need a locally
pinned model + fixed seed, which only the local/ollama path can approach.

---

## 4. Context management: nothing lost, everything findable

Determinism is worthless if context leaks away between runs. nerve's rule is
**retrieve, don't force-feed**, backed by durable on-disk state.

- **Bounded running history** (`compact_context`, `CONTEXT_BUDGET_TOKENS = 100k`).
  Over budget, *old tool-result dumps* (the big file/command outputs) are stubbed
  to a one-line placeholder while the **head** (system prompts + the original
  task) and **tail** (recent turns) and **every assistant reasoning turn** are
  kept verbatim. The model never loses sight of the task or its own reasoning; it
  can cheaply re-read a file if it needs the bytes again. Idempotent, unit-tested.
- **Durable project memory** (`.nerve/`, `project.rs`). Survives across sessions
  and machines (it travels in the rsync sync): `brief.md`, `memory.md`,
  `decisions.jsonl`, `design.md`, `tasks.json`, and the **semantic
  `activity.jsonl`** — which, as of 2026-07-12, records the agent's own summary of
  *what changed and why*, the files it touched, and the iterations spent, not just
  that a job ran. So a later run (or a human) picks up the thread without
  re-deriving it from a diff.
- **Pull-based recall** (`memory_recall.rs`). A tiny always-on header (project
  headline + open tasks + recent semantic activity + a pointer) plus a BM25
  search that pulls only the facts/decisions relevant to *this* turn. Token cost
  scales with relevance, not with how much memory has accumulated — so memory can
  grow without ever bloating the prompt.
- **Nothing-lost sync** (`remote.rs`). `rsync --filter=protect .git/**` means a
  re-sync never deletes the server's `nerve/job-*` result branches, and `.git` +
  `.nerve/` always travel — so scheduled work and its memory survive going
  offline and reconnecting.

---

## 5. What would make it *more* deterministic (open levers)

Honest backlog, roughly by leverage:

1. **A reproducibility harness** — run the same job N times against a fixed repo
   snapshot and diff the outcomes (files changed, verify result). We assert
   *harness* determinism in unit tests but have not yet *measured* end-to-end
   outcome variance. This is the single most valuable next step, because it turns
   "feels flaky" into a number we can drive down.
2. **Local pinned model + seed** for the paths that need true reproducibility
   (ollama with a fixed seed) — the only route to near-bit-exactness.
3. **Plan-adherence enforcement** in the workflow — feed the coder the plan
   verbatim and check each step off, so two runs follow the same skeleton.
4. **Semantic `journal.jsonl`** — the per-tool change journal is still mechanical
   ("replaced N chars"); making it semantic would raise recall quality further.

---

## 6. One-paragraph honest summary

nerve's **harness is deterministic and tested**; its **context management is
durable and nothing is silently lost**; its **sampling is pinned to temperature 0
for unattended runs wherever the provider allows it**. The irreducible
non-determinism is the **model itself** — and on the default `claude_code`
provider we cannot even pin sampling, so there we rely entirely on the harness
guards (nudge + verify gate) to funnel different token paths to the same verified
outcome. The next real gain is *measuring* outcome variance with a reproducibility
harness rather than only asserting the harness logic.
