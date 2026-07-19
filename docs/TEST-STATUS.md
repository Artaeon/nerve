# Nerve — Test Status (honest)

A living, honest record of what has been *verified* vs. *assumed* vs. *untested*,
so we never lose context and can keep improving deliberately. Pairs with
[FEATURES.md](FEATURES.md) (what nerve can do) and [../SERVER.md](../SERVER.md)
(the 24/7 server).

**Last updated:** 2026-07-15 · **Unit tests:** 2,100+ passing (`cargo test`),
clippy clean, fmt clean.

Legend: ✅ proven end-to-end · 🧪 unit-tested only · 💨 smoke-tested · ❔ not
tested (trusted via unit tests) · 🐞 known issue.

---

## ✅ Proven end-to-end (on a real server + real project)

The **24/7 coding server** path is the most thoroughly exercised. On a real
Ubuntu server (key-only SSH, hardened) running `nerve --daemon`, driving real
vollgebucht work:

- **Daemon + queue + worker** — jobs persist `queued → running → done/failed`,
  survive restarts; submit/list/cancel over SSH. ✅
- **Headless agent runs real coding tasks** — Claude via the CLI, isolated on a
  `nerve/job-<id>` git branch, committed for review, `main` never touched. ✅
- **Verify gate** — after edits, runs the project's `tsc`/`cargo check` and feeds
  failures back to self-correct before committing. Observed passing on real
  features. ✅
- **Context compaction** — bounds the headless history so long jobs stay
  token-efficient and keep the task. 🧪 (unit-tested; not yet stress-tested on a
  very long job)
- **Project journaling** — each job recorded to `.nerve/activity.jsonl`. ✅
- **Multi-agent workflow** (planner → coder → reviewer) — all three phases
  observed in logs; read-only roles' write attempts correctly blocked; reviewer
  gets the diff; ends with a VERDICT; verify gate runs on top. ✅
- **Project sync** — `/server submit` rsyncs the whole project (incl. `.git` and
  `.nerve/`, excl. `node_modules`/`target`/…) to `~/nerve-repos/<name>`. ✅
- **TUI remote connection** — `/server <host>` + live `⛁` queue indicator,
  PTY-verified. ✅
- **Provider auth on the server** — `claude auth login` (claude.ai subscription).
  ✅

### Features nerve itself built this session (each independently verified)
| Feature | Mode | Result |
|---|---|---|
| `ARCHITECTURE.md` for vollgebucht | single agent | committed; accurate, high-quality |
| studio data-export (route + pure lib + tests) | single agent | `tsc` clean, 125 tests pass |
| `computeUpcomingByService` (pure + tests) | **workflow** | VERDICT: APPROVED, `tsc` clean, 128 tests |
| iCalendar `.ics` export (4 files) | **workflow** | `tsc` clean, 129 tests pass |
| health/readiness endpoint (improve existing) | **workflow** | ❌ **produced nothing** — see below |

*(These live on server branches `nerve/job-*`; not all merged into vollgebucht `main`.)*

**Honest failure (2026-07-10):** a `--workflow` job to *improve an existing*
minimal `app/api/health/route.ts` (add a DB check + a pure `lib/health.ts`)
**committed nothing** — the coder read files but never wrote, then the reviewer
**ran out of iterations without inspecting** ("Unable to review — tool execution
limit reached") which wrongly triggered a fix round. Two lessons: (1) the
workflow is **not bulletproof** — "improve an existing file" tasks can leave the
coder reading without acting (a variant of the over-exploration bug that the
decisive prompt fixed for greenfield tasks); (2) a **logic bug** — a
cap-exhausted reviewer's "NEEDS FIXES" was treated as a real verdict. **(2) is
now fixed**: the fix round only runs if `!reviewer.hit_max_iterations`. (1)
remains a real reliability gap for edit-existing tasks.

---

## 🧪 Unit-tested (2,100+ tests, all green)

Well-covered by companion `*_tests.rs`: the agent context manager & compaction,
tool-call parser, headless runner (mock provider), the job queue & state machine,
the daemon protocol, remote/sync arg-building, the worker (dirty-tree safety,
verify runner, path parsing), the command catalog, the design linter, config,
sessions, shell escaping/denylist, and much of the UI helpers. Green under
`TZ=UTC` too (where relevant).

---

## 💨 Smoke-tested only (booted, no deep interaction)

- TUI boot + render (status bar, context gauge `[░░░░░]`) via PTY. 💨
- TUI `/server nerve-server` → live `⛁` indicator render via PTY. 💨

---

## ❔ NOT tested end-to-end (trusted via unit tests — the honest gap)

These are believed-working from the test suite but were **not** driven
interactively this session:

- **The interactive TUI experience**: agent mode in the TUI, `/workflow`, the
  command palette / Nerve Bar, prompt picker, model/provider selectors, history
  browser, clipboard manager, search overlay, settings.
- **5 of 6 providers**: only `claude_code` is set up and used. OpenAI,
  OpenRouter, Ollama, Copilot, custom — **untested here**.
- **Knowledge base / RAG** (`/kb`), **plugins**, **automations**, **web scraper**
  (`/url`), **scaffolding** (`/scaffold`, `/template`), **design commands**
  (`/design`, `/design-check`), the **196 prompts**, **sessions & branching** UX,
  **themes**, **model routing** per-turn.
- **Long TUI sessions** with real compaction/usage under load.

---

## 🐞 Known issues / limitations

- **`nerve -n` (non-interactive CLI) does NOT run the agent loop** — it prints
  the model's first turn only. Useless for agentic CLI automation; only the TUI
  and the daemon/worker run the full loop. *(Fix: wire `run_headless_agent` to a
  CLI flag.)*
- **The workflow is thorough but slow** — a 4-file feature took ~16 min (planner
  alone ~8 min / 18 read-only iterations) vs. ~2–4 min single-agent. Use
  `--workflow` for non-trivial features only. *(Lever: curb planner exploration;
  run planner/reviewer on a cheaper model.)*
- **`.nerve/journal.jsonl` (`record_change`) summaries are still mechanical**
  ("replaced N-char with M-char snippet") rather than semantic — low-signal but
  complete. *(Note: the separate `activity.jsonl` job/turn journal was upgraded
  — see below — this line is only about the per-tool `journal.jsonl`.)*
- ~~**`.nerve/activity.jsonl` records only that a job ran**~~ **FIXED (2026-07-12).**
  Records now carry the agent's own summary (what changed & why), the concrete
  files touched, and the iterations spent — the *semantic* record, not just
  `{request, edited, verify}`. Old records still deserialize (`#[serde(default)]`).
  `/activity` and the always-on recall header surface the summary + files.
  Unit-tested (semantic round-trip, long-summary bound, legacy back-compat). ✅
- **In-place `--submit` on a live working tree** switches your checked-out branch
  and (before the fix) could sweep unrelated WIP into a commit. Fixed to
  commit-only-what-changed; still: **run jobs against a dedicated server copy**,
  never a repo you're actively editing.
- **Setup is not turnkey** — SSH keys, `claude login`, `npm install` for the
  verify gate, and the dedicated-copy discipline are all required.

---

## Session 2026-07-13 — worker reliability trilogy + a full app built on nerve

**nerve reliability (all found by dogfooding a long multi-wave build, all with tests, all deployed):**
- **Semantic activity journal** — `.nerve/activity.jsonl` now records the agent's own summary + files touched + iterations (not just that a job ran). ✅
- **Deterministic sampling default** for unattended runs (temp 0 where the provider allows; `claude_code` CLI has no knob — documented in `DETERMINISM.md`). ✅
- **First empirical determinism measurement** (`DETERMINISM.md §4a`): the same task run twice produced **byte-identical code**, only the tests differed. ✅
- **`edited` keys on write SUCCESS not invocation** — a job whose writes all failed no longer fakes a green verify. ✅
- **Worker wedge — the trilogy.** After ~a dozen jobs the long-running worker reaches a state where *every* tool call fails (not fds/mem — accumulated in-process state; only a fresh process clears it). Three layers now handle it: (1) **reactive self-heal** — detect "several tool rounds, zero successes" → exit for a systemd restart; (2) **proactive recycle** — restart every `RESTART_AFTER_JOBS=6` before it can wedge; (3) **auto-requeue** — a job that hit the wedge is put back on the queue (`Job.attempts` ≤ `MAX_WEDGE_RETRIES=2`) so the fresh worker retries it with no manual resubmit. After deploying these, a 5-job waitlist vertical + a theme + SEO batch all landed cleanly. ✅
- **`base_branch` fork** — every queued job forks from a clean base, not the previous job's branch. ✅
- **Anti-confabulation** — prompt + nudge rebut the model's invented "tool execution limit." (Reduced, not eliminated — still recurs on very edit-heavy single jobs; mitigation = decompose.) 🧪

**Proof nerve builds a real app (vollgebucht, ~20+ features this session, each reviewed + verified + merged):**
iCalendar export · health readiness probe · security headers · timezone-parameterized formatters · SMS fallback sender + delivery tier · Vercel cron + DEPLOY.md · extracted+tested validation schemas · **per-studio WhatsApp channels end-to-end** (outbound override + inbound resolver + routing) · SEO robots/sitemap · 404 + error boundary + EmptyState · "Dein Plan" page · booking-page customization · **terracotta 12px theme + consistent radii + terracotta navbar** · **per-studio booking headline** (schema→migration→settings→render) · **waitlist / cancellation-fill vertical** (table→service→API+form→dashboard, proven end-to-end with a live POST). vollgebucht: **191 tests, tsc clean**, running locally.

**Honest quality findings this session:**
- nerve is production-grade on **additive / pure / testable / prescriptive** work; **unreliable when handed a whole cross-cutting change at once** (especially editing existing *mocked tests*) → **decompose** into prescriptive, code-only, ≤2-file jobs.
- Its verify gate checks type/test correctness, **not deployment safety** (caught a `tsx`-in-prod-config hazard and a Vercel-cron GET/Bearer mismatch in review) → **human review still required**.
- Its design-system UI is on-brand and self-consistent when the tokens/classes are named, but it doesn't always check whether a class/component **already exists** (duplicated `.card`/`.well`; downgraded an existing empty state) → **review for duplication/regressions**.

---

## Session 2026-07-14 — context-durability audit + three deterministic fixes

A deep audit of nerve's own context management (token efficiency + "does it
forget?") drove three fixes, each with tests, all landed green (2,007 tests,
clippy + fmt clean):

- **Interactive compaction now pins the task.** `compact_messages`
  (`src/agent/context.rs`) previously protected only *leading system* prompts,
  so on a long TUI conversation the original user request could be summarized
  into a lossy 150-char blurb and lost irreversibly. It now keeps the first user
  turn verbatim — parity with the headless loop's `HEAD_KEEP`. This was the
  single biggest durability asymmetry. ✅ (regression test added)
- **Tool-result feedback no longer truncated below the read cap.** `read_file`
  returns up to 50,000 chars but both runners re-truncated every tool result to
  5,000 — the model saw only the first 10% of a file it just read, and
  re-reading returned the same clipped window. A single source of truth
  (`MAX_TOOL_OUTPUT_CHARS = 50_000`, `src/agent/tools/fs.rs`) now governs the
  read cap and both feedback caps. ✅
- **Default provider retries transient failures.** `claude_code.rs` (the default,
  and what drives the unattended worker) had *no* retry — a network blip or API
  429/5xx/529/overload aborted the whole turn; only `openai.rs` used the existing
  backoff. Extracted `chat_once` and wrapped `chat()` in `retry_async`; taught the
  classifier about Anthropic's 529 "overloaded". ✅

**Honest verdict from the audit:** token efficiency is above average (pull-based
BM25 memory, <1,200-char always-on header, last-turn-only `@file` expansion); the
headless/worker path does **not** forget (task + project `.nerve` pinned, journals
durable + corruption-tolerant); the interactive path *could* forget until the fix
above. Remaining known weaknesses: two divergent compactors (interactive vs
headless) should eventually be unified, and budgeting still uses a `chars/3`
heuristic rather than a real tokenizer.

**Dogfooding finding (vollgebucht, 2026-07-14):** a "reschedule button" job
(edit-existing UI wiring) produced a **stub** — it imported `rescheduleAppointment`
and defined helper constants but rendered no form and called nothing; tsc passed
only because vollgebucht's config doesn't flag unused locals. Same "reads but
half-acts" failure mode seen before on edit-existing tasks. **Mitigation that
worked:** re-issue with a fully prescriptive spec (exact function signatures, the
exact JSX to render, explicit acceptance criteria: "X must actually be CALLED",
"a datetime input must actually be RENDERED"). Reinforces the standing rule:
decompose cross-cutting UI wiring into prescriptive, code-only jobs, and
human-review every result for completeness — the verify gate checks type/test
correctness, not feature completeness.

### 2026-07-14 continued — five more reliability/efficiency fixes + a full feature wave

Driven by dogfooding a large vollgebucht build (14 jobs), five more fixes shipped
(all tested → **2,018 green**, deployed):
- **Cap-stopped jobs flagged INCOMPLETE** (`worker.rs`) — a single-agent job that
  hits the iteration cap can commit a partial stub that type-checks; the
  incompleteness is now appended to the commit message + activity journal so a
  reviewer sees it. Caught 3 real stubs this session. ✅
- **Orphaned `Running` jobs reclaimed on startup** (`queue.rs`) — a crash/hang/
  restart no longer strands a job; the fresh worker requeues it (bounded). Fixed a
  live worker hang. ✅
- **Every job starts from a pristine tree** (`worker.rs`) — `checkout <base>` left
  stale dirt from a reclaimed job, which `pre_dirty` then excluded from the commit,
  silently dropping the real edit (a committed branch that didn't even compile
  while the log said "verify passed"). Now force-checkout + reset --hard + clean.
  This is the subtle one — it made a "verify passed" commit non-compiling. ✅
- **Explore-nudge (token efficiency)** (`headless.rs`) — after 8 tool-iterations
  with no edit, a one-time positive "you have enough context, implement now" nudge
  (never says "limit"). **Measured effect: comparable jobs went 29–40 → 8–24
  iterations.** ✅
- **Quota-defer** (`worker.rs`/`queue.rs`) — a provider session/quota limit now
  DEFERS the job (`Job.not_before`) to resume after the window resets, instead of
  failing it. Hit live when a job burst exhausted the claude.ai session quota. ✅
- **Test suite in the verify gate** (`verify.rs`/`worker.rs`) — the gate ran
  lint/typecheck only, so a job could commit a *failing test* (seen live). It now
  chains the project's test command (`detect_test_command`; cargo test / npm test,
  skipping watch-mode) after the type-check into the same self-correct loop. ✅

**Efficiency verdict (answers "does nerve burn too many tokens?"):** the context
*system* is lean, but the *execution loop* was expensive (29–40 iters, quadratic
re-send) — enough to exhaust the claude.ai session quota mid-batch. The
explore-nudge + prescriptive specs cut that to 8–24 iters. Open lever: route
exploration to a cheaper model.

**Feature-wave result (8 vollgebucht features, nerve-built, verified + merged):**
reports page · customer CRM page · review link in email · reschedule UI ·
responsive mobile nav · responsive tables · studio cover image (2 jobs). Clean
first try on additive/pure/new-file jobs (8–21 iters); cross-cutting edit-existing
jobs (customer-linking, DSGVO consent) still failed at the cap → decompose. The
INCOMPLETE flag + running tsc+vitest per branch caught every bad output.

### 2026-07-14 (late) — the self-decompose loop, built + dogfooded

The one real nerve weakness (cross-cutting edit-existing tasks thrash to the
iteration cap) got a systematic fix: `--decompose` runs a read-only PLANNER that
splits the task into small ordered JSON sub-tasks, then executes each through the
normal agent, with a safe fallback to a single run if planning yields nothing.
Wired end-to-end (`--decompose` → `SUBMITDC` → `Job.decompose` → worker dispatch),
+6 tests, deployed (2,023 tests).

**First dogfood — the vollgebucht marketing overhaul as ONE `--decompose` job:**
- ✅ The planner produced **8 sensible, well-scoped sub-tasks** (globals utility
  classes → footer grid → per-page hero/well fixes) — exactly the right split.
- ✅ Executed steps 1-7 cleanly.
- 🐞 **Then the worker WEDGED** — a decompose job runs 9+ sub-agents in ONE
  process, well past the wedge threshold. The auto-requeue's clean-start
  **discarded steps 1-7**, and the retry's planner didn't emit parseable JSON so
  it **fell back to a single agent** (the safety net) — which completed the
  (mechanical) overhaul in 19 iters; the test-gate (`lint && npm test`) verified
  it. Result merged, correct, 222 tests.
- **Fix shipped (commit adf1ad2):** decompose now COMMITS each finished step to
  the branch, and a requeued job's clean-start KEEPS its branch commits instead
  of resetting to base — so a mid-run wedge now RESUMES with the done steps
  instead of starting over. Deployed; the durable end-state (subprocess isolation
  per sub-task, which would make decompose wedge-immune) remains the next big
  lever.

**Honest read:** the decompose loop's *core is sound* (it planned + executed a
real cross-cutting task well) and the fallback makes it safe, but it stresses the
long-standing worker wedge harder than any single job — the wedge is now the
top reliability bottleneck to eliminate.

### 2026-07-15 (later) — THE WEDGE, ROOT-CAUSED AT LAST 🎯

**The wedge was never mysterious "accumulated in-process state". It was a
one-line bug: a process-global tool counter that the headless path never reset.**

- `src/agent/tools/mod.rs`: `static TOOL_EXEC_COUNT` + `MAX_TOOL_EXECUTIONS`
  (was 100). Past the cap, `execute_tool` returns
  `success: false, "Tool execution limit reached (100). Start a new session."`
  for **every** tool — `read_file` included.
- `reset_tool_counter()` existed, but its ONLY non-test caller was the TUI's
  `/agent off` (`commands/ai.rs`). `run_role` never called it, so a long-lived
  worker accumulated tool calls **across jobs** until everything failed.

This retroactively explains the entire reliability saga:
- "After ~a dozen jobs every tool fails" — that's ~100 tool calls.
- Why **every** mitigation worked: `RESTART_AFTER_JOBS`, the `all_tools_failed`
  self-heal, the wedge requeue, and subprocess isolation all just handed the run
  a **fresh process → fresh counter**. They treated the symptom.
- Why the marketing decompose died after step 7 but the SEO decompose (subprocess
  per step) sailed through 7 — each child got its own budget.
- **The agent was never confabulating.** `AGENT_SYSTEM` insisted "there is NO
  tool-execution limit … never claim you have hit one" while the code enforced
  exactly that limit and returned exactly that string. The model was accurately
  reporting our own error message and we told it its observation was false. The
  prompt + nudge are now honest.

**Fix:** `reset_tool_counter()` at the top of `run_role` (every run gets its own
budget) and the runaway backstop raised 100 → 500 (100 was below what a single
legitimate 40-iteration job needs). +1 regression test, **verified to fail
without the reset**. ✅

*Lesson for this record: "never root-caused" in the notes below was a standing
invitation to keep building scaffolding. A deep read of the actual tool path
found it in one pass. Read the code before architecting around a ghost.*

### 2026-07-15 — subprocess isolation (still valuable) ✅

The worker wedge (accumulated in-process state that, after ~a dozen tool-heavy
runs, makes *every* tool fail — never root-caused) is eliminated for the path
that hit it hardest. Each decompose STEP now runs in a fresh `nerve --exec-agent`
child process (stdin: `ExecAgentRequest` JSON → runs one full-tool agent → stdout:
`HeadlessOutcome` JSON → exits). No state carries between steps. Falls back to
in-process if spawning fails. Plus decompose now commits each step, and a requeued
job keeps its branch commits, so any interruption resumes instead of restarting.

**Proven live, back-to-back — the cleanest A/B this project has produced:**
| Run | Isolation | Result |
|---|---|---|
| Marketing overhaul (`--decompose`, 8 steps) | none (in-process) | 🐞 **wedged after step 7**, progress discarded, fell back to a single agent |
| SEO overhaul (`--decompose`, 7 steps) | **subprocess per step** | ✅ **all 7 steps completed, no wedge**, `lint && npm test` passed, merged |

`ps` confirmed the `nerve --exec-agent` child running under the daemon during the
run, and the branch carried one commit per step (`decompose step 7/7: …`). The
decompose parent now only runs the read-only planner + spawns children, so it
stays light. **Remaining lever:** extend `--exec-agent` to every job (not just
decompose steps), which would retire `RESTART_AFTER_JOBS` entirely.

## How to extend this record
When a feature is genuinely exercised, move it up (❔ → 💨 → ✅) with a one-line
note of *what was run* and *what was observed*. Keep it honest: "passes its unit
tests" is 🧪, not ✅.
