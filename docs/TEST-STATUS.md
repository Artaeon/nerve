# Nerve — Test Status (honest)

A living, honest record of what has been *verified* vs. *assumed* vs. *untested*,
so we never lose context and can keep improving deliberately. Pairs with
[FEATURES.md](FEATURES.md) (what nerve can do) and [../SERVER.md](../SERVER.md)
(the 24/7 server).

**Last updated:** 2026-07-10 · **Unit tests:** 1,990 passing (`cargo test`),
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

## 🧪 Unit-tested (1,990 tests, all green)

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

## How to extend this record
When a feature is genuinely exercised, move it up (❔ → 💨 → ✅) with a one-line
note of *what was run* and *what was observed*. Keep it honest: "passes its unit
tests" is 🧪, not ✅.
