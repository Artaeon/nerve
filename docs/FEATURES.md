# Nerve — Complete Feature & Capability Inventory

Grounded in the source (not marketing). Pairs with [TEST-STATUS.md](TEST-STATUS.md)
(what's actually verified) and [../SERVER.md](../SERVER.md) (the 24/7 server).

Nerve is a terminal-native, multi-provider AI coding assistant in Rust (edition
2024), on `ratatui`/`crossterm` + `tokio`. It runs as an interactive TUI, a
one-shot/pipe CLI, or a 24/7 background server draining a persistent job queue
with an autonomous agent. Beyond chat: a 13-tool coding agent (git-checkpoint
undo, auto-verify gate, planner→coder→reviewer workflow), per-project persistent
memory in `.nerve/`, pull-based memory retrieval, per-turn model routing, a local
knowledge base, a 196-entry prompt library, project scaffolding, a design linter,
automations, and a sandbox-lite plugin system.

**Headline numbers (from source):** ~53,300 lines of Rust across ~90 files ·
**1,991 tests** passing (1 ignored), clippy+fmt clean · **102 catalogued slash
commands** (+~12 dispatched but uncatalogued) · **13 agent tools** · **6 provider
names via 3 impls** · **196 prompts** · **10 themes** · **8 scaffold templates** ·
**3 design presets** · **5 automations** · v0.2.0, MIT, rust 1.85.

---

## 1. CLI flags & subcommands (`src/main.rs`)

- **`[prompt]`** (positional) — runs non-interactively (prints response, exits). No prompt → interactive TUI.
- **`-m/--model`**, **`-p/--provider`** (`claude_code`/`openai`/`ollama`/`openrouter`/`copilot`) — an explicit provider is never silently switched; if unavailable, nerve errors with setup guidance.
- **`--stdin`** — pipe mode; **`--list-models`**; **`-n/--non-interactive`**; **`-c/--continue`** (resume last session); **`--no-splash`**; **`--completions <SHELL>`**.
- **Server/queue (Unix):** **`--daemon`** (start server + worker), **`--stop-daemon`**, **`--query <Q>`**, **`--submit <PROMPT>`** (repo = cwd), **`--with-session`** (attach full session), **`--repo-path <PATH>`**, **`--workflow`** (multi-agent), **`--jobs`** (+ **`--json`**), **`--cancel-job <ID>`**.
- **Startup:** loads config → provider health-check with auto-fallback (only when provider wasn't explicit) → workspace detect → plugins → verify-command resolve → (`-c`) restore session.

## 2. Slash commands (`src/commands/catalog.rs` — single source of truth)

**102 commands** in a compile-time table feeding autocomplete + the command palette (tests enforce no duplicates, ≥90). By category: Chat (10), AI Provider (18), Knowledge (12), Shell & Git (25), Scaffolding (5), Automation (5), Sessions (4), Branching (5), Workspace (1), Server (4), Usage & Cost (6), System (2), Power User (3), Plugins (3).

**Honest gap — dispatched but NOT catalogued (~12):** `/workflow`, `/approve`, `/reject` (pipeline control), `/suggest` (BM25 prompt search), and the project-memory family: `/init`, `/remember`, `/decision`, `/design` (+ `/design preset`, `/design-check`), `/improve` (+ `/improvements done`), `/task`/`/tasks`.

## 3. Agent tools (`src/agent/tools/`)

**13 tools**, executed via `execute_tool`, capped at **100 executions/session**, parsed from the model's `<tool_call>` text blocks (native provider tools disabled so the text protocol is the only path):
`read_file`, `write_file`, `edit_file` (exact string replace), `list_files`, `create_directory`, `find_files` (glob), `read_lines`; `run_command` (shell, timeout-honoring), `search_code` (grep), `web_search` (DuckDuckGo), `remember` (append to `.nerve/memory.md`), `recall` (search project memory), `update_tasks` (`.nerve/tasks.json`). System prompt frames UNDERSTAND→PLAN→IMPLEMENT→VERIFY→TEST.

## 4. AI providers (`src/provider_setup.rs`, `src/ai/`)

3 `AiProvider` impls → 6 names:
- **Claude Code** — shells the `claude` CLI; code mode passes `--dangerously-skip-permissions` + the critical **`--tools ""`** (removes native tools so nerve's text tools work — `--allowedTools ""` does NOT). Default `sonnet`; ladder opus/sonnet/haiku; cost $0 (subscription).
- **OpenAI-compatible** — one impl for `openai`/`ollama`/`openrouter`/custom (differ by base URL/key/name); applies temperature/top_p + retry; keys from config then env.
- **Copilot** — via `gh` CLI + Copilot extension.
Retry/backoff in `ai/retry.rs`; health checks + friendly guidance in `provider_health.rs`. Last provider+model persisted.

## 5. Server / queue / worker (24/7 autonomous)

- **Daemon (`daemon.rs`, Unix):** Unix socket at `~/.nerve/nerve.sock` (dir 0700, socket 0600 — not in world-writable /tmp). Spawns the worker on start. Tab-separated protocol: `PING/SUBMIT/SUBMITWF/LIST/LISTJSON/ATTACH/STATUS/CANCEL/__SHUTDOWN__`; pure `process_command` unit-tested.
- **Queue (`queue.rs`):** directory-backed at `~/.nerve/queue/`, one atomic JSON per `Job {id, repo, prompt, status, branch, has_context, workflow, timestamps, error}`. Lifecycle Queued→Running→Done/Failed/Cancelled. Optional `<id>.context.json` session bundle. 8-hex ids double as branch names.
- **Worker (`worker.rs`):** drains **sequentially** (poll 3s). Per job: isolate on `nerve/job-<id>` branch → build provider from current config → fold any attached session into the task → **dirty-tree safety** (commits only paths the job newly touched, never `git add -A`) → CWD=repo → single agent OR workflow → **verify gate** → commit `nerve job <id>: …` → journal to `.nerve/activity.jsonl`. Uses `git -c safe.directory=*` per call.
- **Headless agent (`agent/headless.rs`):** TUI-less loop reusing the same tools/parsing. `DEFAULT_MAX_ITERATIONS=40`. **Context compaction:** over a 100k-token budget it stubs old tool-result content, keeping head (system+task) + tail (recent) verbatim and all assistant reasoning.
- **Multi-agent workflow (`run_workflow`):** **planner** (read-only, plan grounded in real code) → **coder** (full tools) → **reviewer** (read-only, handed the real `git diff`, ends `VERDICT: APPROVED|NEEDS FIXES`); one coder fix round only if the reviewer actually concluded. `ToolPolicy::ReadOnly` enforced at the tool layer.
- **Verify→fix loop:** after edits, runs the detected verify command; feeds failures back up to `MAX_VERIFY_ROUNDS=2`; records e.g. `cargo check → passed after 1 fix round`.
- **Remote + sync (`remote.rs`):** TUI ↔ server over plain `ssh` (no new port). `sync_repo` rsyncs to `~/nerve-repos/<name>` (`-az --delete`, excludes build/deps, **includes `.git` + `.nerve/`**). Live "2 running · 5 queued" badge.
- **TUI `/server`:** `/server <host>` connect, `/server` show queue, `/server status`, `/server submit [--workflow] <prompt>`, `/server off`.

## 6. Context management (`src/agent/context.rs` + UI)

- Token estimate = chars/3+1 (char-based; CJK/emoji safe). `smart_truncate` UTF-8-safe on sentence/word boundaries.
- Per-provider limits: Claude 200k / OpenAI 60k / OpenRouter 30k / Ollama 32k / default 30k, overridable.
- Two strategies: `compact_tool_results` (shorten old tool dumps, keep last 6) and `compact_messages` (summarize older turns into a synthetic system summary).
- UI: status-bar context meter; `/context`, `/tokens`, `/compact`, `/summary`. Usage recorded from the payload *actually sent*.

## 7. Project memory — the `.nerve/` directory (`src/project.rs`)

All plain-text/JSONL, inspectable, atomic writes, one-line-sanitized (can't forge extra rows):
`memory.md` (facts), `brief.md` (`/init`), `design.md` (principles/presets), `decisions.jsonl`, `journal.jsonl` (`{timestamp,tool,path,summary}`), `activity.jsonl` (`{request,edited,verify}` per turn + per worker job), `improvements.json`, `tasks.json`.
**Security:** `.nerve/` is a protected write target — a prompt-injected model can't poison memory via write/edit tools; only user commands + the inert `remember` tool write it.
**Pull-based retrieval (`memory_recall.rs`):** memory is *retrieved, not force-fed* — `always_on_context` injects only a tiny header (project headline, open tasks, last-3 activity, count-only pointers); the `recall` tool searches on demand, so token cost scales with relevance, not memory size.

## 8. Other subsystems

- **Prompts (`prompts/`):** 196 built-ins (`{{input}}` templates), `LazyLock`-cached; `/suggest` uses **BM25** (K1=1.5, B=0.75); custom prompts from `~/.config/nerve/prompts/*.toml`.
- **Knowledge base / RAG (`knowledge/`):** local, **no embeddings**. Ingests a dir into 500-word chunks (JSON at `~/.local/share/nerve/knowledge/`); search = fuzzy (Skim) + exact-keyword (distinct from `/suggest`'s BM25). `/kb add|search|list|status|clear`.
- **Scaffolding (`scaffold.rs`):** 8 embedded templates (rust-cli/lib/web, node-api/react, python-cli/api, go-api) via `/template`; AI `/scaffold <desc>` generates a whole project (needs code mode).
- **Design (`design.rs`, `design_presets.rs`):** deterministic regex-free linter — always-on `off-grid-spacing` (4px scale, spacing props only) + `color-sprawl` (>12 hex); principle-gated `gradients`/`emoji`/`shadow-heavy` (EN+DE keywords in `.nerve/design.md`); 3 presets (basecamp/apple/linear); advisory auto-check on edited UI files.
- **Automations (`automation.rs`):** multi-step prompt chains (`{{prev_output}}`, optional per-step model); 5 built-ins; custom TOML in `~/.config/nerve/automations/`.
- **Plugins (`plugins.rs`):** subdir + `plugin.toml`; child process, 30s timeout, 1MB cap, ANSI-stripped. **No OS sandbox** — isolation is timeout+cap+sanitization (honest caveat).
- **Sessions & branching (`session.rs`):** persist to `~/.local/share/nerve/sessions/` (last + 25 named, auto-saved each turn, `-c` restores). **Conversation branches live on App, NOT in Session → they do not survive save/load (limitation).**
- **Web (`scraper/`):** `/web` = DuckDuckGo Instant Answer (no key); `/url` scrapes with strong **SSRF protection** (blocks private/localhost, resolves+pins the verified socket vs DNS-rebind, manual redirect re-validation, 5MB cap).
- **Model routing (`model_router.rs`):** per-turn tiers relative to baseline (Claude haiku/sonnet/opus; OpenAI mini/4o); planning/review→heavy, coding→standard; length alone never downgrades; only ever routes *down* from a strong baseline.
- **Verify gate (`verify.rs`):** `cargo check --quiet` or first of npm `typecheck/type-check/lint/check`; `MAX_VERIFY_ROUNDS=2`; used by both the TUI loop and the server worker.
- **Usage/cost (`usage.rs`):** pricing table (Claude/Ollama/Copilot $0; OpenAI 4o≈$7.50/M, mini≈$0.30/M); `/usage`, `/cost`; `SpendingLimit` (default $5, off by default). Costs are explicitly rough.
- **Themes/keybinds:** 10 themes (`/theme`). `keybinds.rs` parses key strings but is **not yet wired to a configurable keymap (partial, dead_code)**.
- **Shell security (`shell.rs`):** POSIX `shell_escape`; `is_dangerous_command` denylist + structural `has_catastrophic_rm` (flag-order/path independent); `is_protected_path`/`is_sensitive_file` (blocks `.env*`, SSH/GPG/AWS creds); `is_protected_write_target` (`.git/hooks`, `.nerve/`, rc files, authorized_keys); `run_command_with_timeout` kills the whole process tree on timeout; `mask_api_key`.
- **Modes & intent (`app.rs`, `agent/intent.rs`):** modes Standard/Efficient/Thorough/Agent/Learning (+auto/code/review); auto-agent activates on tool-needing messages inside a workspace; auto-context auto-includes up to 3 referenced files (≤12k chars).

## 9. Testing

`cargo test` → **1,991 passing, 1 ignored** (single binary). Companion `*_tests.rs` for app/config/main/queue/remote/shell/headless/fs; inline `mod tests` elsewhere. Covers security (SSRF, denylist, protected paths), compaction/truncation multibyte edges, the queue state machine, intent detection, model routing, workflow verdict logic. CI enforces clippy/fmt.

---

## Honest caveats (from the code)
1. The command "single source of truth" omits ~12 dispatchable commands (see §2).
2. Conversation branches aren't persisted across sessions.
3. `keybinds.rs` is implemented + tested but not wired to a configurable keymap.
4. Several helper APIs are present but `dead_code` (custom-prompt save/delete, automation result types).
5. Prompt-category header comments drifted from the true 196 total.
6. Two search algorithms coexist: BM25 for `/suggest`, fuzzy+keyword (no embeddings) for the KB.
7. The server worker is single-agent-capable and workflow-capable, but the workflow is **slow** and **not bulletproof on "improve existing file" tasks** — see [TEST-STATUS.md](TEST-STATUS.md).
