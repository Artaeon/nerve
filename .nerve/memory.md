# Project memory — nerve editing nerve

Facts and conventions for working on nerve's OWN Rust source. Read before changing agent/parser/worker/context code. These are hard-won: each line prevented a real, shipped bug.

## THE TOOL-CALL PROTOCOL — control plane vs data plane (the #1 rule)
- `src/agent/tools/parse.rs` turns model text into `ToolCall`s. **File CONTENT must NEVER be re-scanned as control.** A multiline arg (`content`/`old_text`/`new_text`) is read LITERALLY to the tool-call block terminator — do not stop it at "a line that looks like a key", and accept `tool:` only once, before any multiline arg. Violating this caused three silent bugs: a `path:` line inside written code hijacked the path and truncated the file (junk files named `['serviceId'],`, `text('path').notNull(),`); a fence inside content was stripped so nerve couldn't write a doc with a code example; a `tool:` line inside a doc changed which tool executed. If you touch the parser, add a test that writes a file whose CONTENT contains `path:`, `content:`, ` ``` `, and `<tool_call>` — all must survive intact.
- Defense in depth: `fs.rs::looks_like_code_fragment` rejects a write whose PATH carries quotes/brackets/semicolons/commas. Keep it.

## THE MODEL IS FRAGILE IN SPECIFIC WAYS — respect these or it misbehaves
- **NEVER mention a "limit", a cap, or an iteration count to the model.** Naming a limit makes it quit early ("I've hit the tool limit"). All nudges use POSITIVE framing only. This is why the wedge looked like confabulation for weeks — the model was accurately reporting a real internal counter while the prompt insisted none existed.
- The `claude_code` provider MUST pass `--tools ""` (empty), NOT `--allowedTools ""`. The latter only withholds auto-approval; the model then reaches for the CLI's native Edit tool and every edit fails "permission not granted" in headless mode.
- The `claude_code` CLI has NO `--temperature/--seed/--top-p`. Determinism lives ENTIRELY at the harness layer (nudges, verify gate, prompts), never at sampling.

## THE GATE PROVES COMPILATION, NOT CORRECTNESS
- The auto-verify gate runs `cargo check` + `cargo test` (and a project's `.nerve/verify.toml` extra steps). It does NOT run clippy, and it CANNOT see mangled markdown/CSS/YAML output. **Always run `cargo clippy --all-targets -- -D warnings` yourself before declaring done** — clippy is what catches code that is written but never wired in (an `unused import` exposed a "fix" whose logic was never connected, though its comment claimed otherwise).
- `run_verify` has a 15-min timeout (a hung build must not freeze the sequential worker). Keep it.
- A job that changed no files reports `JobStatus::NoChanges`, decided by the GROUND-TRUTH diff, never the agent's self-reported `edited`. The whole bug class is an agent believing it acted when it didn't.

## CONTEXT & MEMORY — one path, never two
- `src/project_context.rs::build(store, opts)` is the SINGLE source of truth for injected project context. BOTH the TUI (`conversation.rs`) and the headless worker (`agent/headless.rs`) call it. Do NOT add a second context builder — the two silently diverged once and the worker lost auto-recall entirely (`recall` was called 0 times in 2,362 tool calls). Injection caps: brief 4k / memory 12k / design 5k chars.
- `.nerve/` is the project's MEMORY. The worker's pristine-tree reset uses `git clean -fd -e .nerve` — never clean the memory dir. Curated files (memory.md/brief.md/design.md) are TRACKED git artifacts; the append-only logs (activity.jsonl/journal.jsonl) are gitignored so a reset can't wipe them.

## WORKING ON THIS CODEBASE
- Rust. The gate is `cargo test` + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check`. No `unwrap()` in non-test code. Every logic file has a `_tests.rs` companion (via `#[path]`) or an inline `#[cfg(test)] mod tests`.
- Comments say WHY, not what — and cite the real bug a rule prevents (see `parse.rs`, `verify.rs`, `worker.rs` for the house style). Match it.
- Prescriptive, single-target, ≤2-file tasks succeed cheaply. Open-ended "find a good place to…" tasks burn the whole iteration budget on search — decompose or name the exact target.
- `src/agent/headless.rs` is the most-edited file and the base-drift magnet: when two jobs both touch it, one will fork from an older base and silently REVERT the other. Before taking any file a job changed outside its stated scope, check it descends from the current tip.
- Never edit `.nerve/` from agent tools — it is write-protected on purpose.
- The memory-DB substrate (SQLite + FTS5 + link graph, OUTSIDE the repo at ~/.nerve/projects/<hash>/memory.db) is fully specified in docs/MEMORY-DB-SPEC.md. Build it in the 6 sequenced steps there — never as one big job, and never before checking the spec's acceptance tests.
