# Changelog

All notable changes to Nerve will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Out-of-the-box experience
- Provider health checks at startup (PATH scan for `claude`/`gh`, TCP probe for Ollama, key presence for OpenAI/OpenRouter) with automatic fallback to the best available provider when the default can't run
- Friendly multi-provider setup guidance when no provider is available, instead of a raw error on the first prompt
- Workspace-default agent activation: inside a detected project, coding requests activate the agent automatically; clearly conversational messages stay chat-only (`intent::should_activate_agent`)
- Any git repository is now detected as a workspace even without a language manifest; language inferred from the dominant source-file extension
- Claude CLI failures surface the CLI's own message with login guidance instead of a raw JSON blob

#### Per-project persistent memory (`.nerve/`)
- `/init` -- analyze the repo once and save an engineering brief injected into every prompt
- `/remember <fact>`, `/memory` -- persist and view project facts/conventions
- `/decision <text>`, `/decisions` -- append-only decision log (last 5 always in context)
- `/task <title>`, `/tasks`, `/task done|start|fail <id>` -- a task backlog that survives sessions
- `/improve <idea>`, `/improvements` -- improvement backlog
- `/changes` -- audit trail (`.nerve/journal.jsonl`) of every agent file write
- New agent tools `remember` and `update_tasks` (12 tools total) so the model maintains memory/tasks itself
- `.nerve/` is write-protected from the agent's file tools; all writes go through a sanitized API (prompt-injection persistence defense)

#### Multi-agent workflow
- Plan-approval gate: `/workflow` now pauses after planning -- nothing executes until you `/approve` (or `/reject`); `workflow_auto_approve` config restores the old behavior
- Planner runs with read-only repo access so plans reference real files and symbols
- Green-gate commits: `/agent commit` runs the project's tests first and refuses to commit on red; `/agent commit force` overrides

#### Developer Workflow
- `/lint` command -- auto-detect and run project linter (clippy, eslint, ruff, golangci-lint, rubocop, credo)
- `/format` (`/fmt`) command -- auto-detect and run code formatter (cargo fmt, prettier, ruff, gofmt, rubocop, mix format)
- `/search <pattern>` command -- search codebase with ripgrep, results added as AI context
- `/commit` now uses `--author` from configured git_user_name/email
- AI-generated commit messages via `/commit` (no message argument)

#### AI & Prompt Engineering
- `temperature` config option (0.0-2.0) for controlling response creativity
- `top_p` config option (0.0-1.0) for nucleus sampling
- `context_limit` config option to override provider default context window size
- Mode-specific system prompts: Efficient (concise), Thorough (detailed), Agent (workflow), Learning (Socratic)
- Ollama default context raised from 8K to 32K tokens

#### UI/UX
- Color-coded token usage percentage in status bar (green/yellow/red)
- Vim `G`/`g` keys to jump to bottom/top of conversation
- `PageUp`/`PageDown` for fast scrolling (30 lines)
- Context-aware status bar hints (different hints per mode)
- Working directory display in status bar when code mode is active
- Auto-agent mode: automatically enables tools when message needs them

### Fixed

#### Plan-approval gate & workflow hardening (adversarial review)
- **Approval-gate bypass**: a parked workflow is now advanced only by an explicit `/approve`; the event-loop no longer executes the plan when an unrelated message is sent while awaiting approval
- Ordinary messages are blocked with guidance while a workflow awaits approval
- `remember` and `update_tasks` are now treated as write tools, so read-only roles (the pre-approval Planner, the Reviewer) cannot mutate `.nerve/` project memory
- `Esc` cancels a workflow parked at the approval gate; `/clear` tears the pipeline down like `/new`
- `/agent commit` green gate uses a full-suite timeout (600s) instead of the 30s per-tool timeout, so real test suites no longer falsely "time out"

#### Security Hardening
- **SSRF**: Proper URL parsing replaces string-matching blocklist; blocks IPv6 loopback, link-local, ULA (fc00::/7), multicast (ff00::/8), IPv4-mapped IPv6, IPv4-compatible IPv6, CGN range
- **SSRF**: Final URL re-validated after redirect chain to prevent redirect-based SSRF
- **Command injection**: All agent tool shell commands use `shell_escape()` (verify_file_syntax, search_code, find_files, git_diff)
- **Path traversal**: `normalize_path()` resolves `..` segments without filesystem access; blocks `/tmp/x/../../etc/passwd`
- **Path traversal**: History conversation IDs sanitized to alphanumeric+hyphens
- **Symlink attacks**: `validate_write_path()` canonicalizes paths before protection check
- **Device file DoS**: Block char/block devices, FIFOs, sockets, symlinks from file reads
- **Command filter**: Block poweroff, halt, systemctl reboot, shred, wipe, chpasswd; block pipe to zsh, python, perl, ruby, ksh, dash
- **ANSI injection**: Strip ANSI escape sequences (CSI, OSC, SS2/SS3) from plugin output
- **Clipboard**: Propagate permission-setting errors instead of silently ignoring
- **Knowledge search**: Fix ln(0) edge case when scoring empty chunks
- **Memory**: `truncate_output()` avoids collecting all lines before truncating
- **SSRF (redirects)**: Redirects are followed manually with per-hop resolve-and-pin — every hop is re-validated against private IPs, closing DNS-rebind/redirect SSRF that a string-only final-URL check missed
- **Secret-file leak**: Auto-context no longer reads `.env`/`id_rsa`/`.ssh`/credential files and injects them to the LLM (now matches the `read_file` tool guard)
- **Path traversal**: `create_directory` and `/template` are now validated (previously bypassable with `..`/absolute paths)
- **Persistence/exfil targets**: Agent writes are blocked to `~/.ssh/authorized_keys`, `~/.ssh/config`, shell rc files, `.git/hooks/*`, and credential files (`.aws/credentials`, `.netrc`, `.pgpass`) — the classic prompt-injection targets
- **Dangerous-command denylist**: Structural `rm -r -f` detection resists flag-order / long-flag / path-prefixed-binary bypasses (`rm --recursive --force /`, `/bin/rm -rf /`)
- **Daemon**: Control socket moved off world-shared `/tmp/nerve.sock` to a per-user `~/.nerve` dir (0700) with a 0600 socket, HOME-anchored for a deterministic client/daemon path
- **Clipboard**: History file written 0600 atomically with no world-readable window; tests never touch the real OS clipboard or data dir

#### Reliability & Context
- **@file expansion**: Expanded once, on the latest user turn only — was re-reading each referenced file (up to 1 MB) and re-injecting it on *every* request (quadratic token growth); compaction now runs on the true payload
- **Shared message builder**: The initial send, post-tool follow-up, and regenerate all build context the same way, so the active mode/persona, knowledge-base results, auto-context, and `@file` content are never silently dropped after a tool round or on regenerate
- **Compaction**: Preserves chronological order and no longer hoists (or silently drops) mid-stream file/command context; notifies the user when it summarizes older turns
- **Auto-agent**: Reliably reverts after a tool-running turn (no leak into the next plain message); injected project context no longer accumulates across activations
- **Token accounting**: Unified estimator across the status bar, `/tokens`, `/context`, the spending check, and compaction (was inconsistent — the on-screen % was ~25% off)
- Panic fixes on multibyte / edge-case input; bounded network and file reads; concurrent plugin pipe draining (fixes >64 KB output deadlock); u16 overflow fix in popup sizing on wide terminals

#### UI/UX
- Welcome/onboarding screen now appears on the common in-workspace first run
- Typo'd slash commands are caught (`Unknown command …`) instead of being sent to the model
- Friendly, provider-specific setup help on first run when no key/CLI/local server is configured
- Errors/warnings linger longer than transient confirmations; `Esc: stop` hint during streaming; scrollable help overlay; bounded scroll

### Changed
- Removed dead `/commit` handler from shell.rs (git.rs handles it)
- Moved `build_git_author_flag` to git.rs where it's used
- Split god files into cohesive modules: `main.rs` 5,100 → 2,280 lines (extracted `splash`, `provider_setup`, `completion`, `conversation`, `input`); extracted `ui::markdown`, `ui::{theme,selectors,status_bar,input_box}`, and `agent::tools::parse`

### Quality
- Tests: 1,345 -> 1,760+
- 0 clippy warnings (`-D warnings`)
- 0 formatting issues
- Extensive additional security, context-management, and reliability hardening; no god files remaining

## [0.1.0] - 2025-04-01

### Added

#### AI Providers
- Claude Code (no API key needed — uses subscription)
- OpenAI (API key)
- OpenRouter (API key, 100+ models)
- Ollama (local, free, offline)
- GitHub Copilot (gh CLI)
- Custom OpenAI-compatible endpoints

#### Chat
- Streaming responses with syntax-highlighted code blocks
- Markdown rendering (bold, italic, headers, lists, blockquotes, links)
- Line numbers in code blocks
- Animated thinking spinner and streaming progress bar
- Message number badges (1-9) for quick copy
- Multi-line input with Shift+Enter
- Dynamic input area that grows with content
- Input history with Up/Down arrows
- Vim keybindings (i/Esc, j/k scroll)
- Mouse scroll support

#### Agent Mode
- 9 tools: read_file, write_file, edit_file, run_command, list_files, search_code, create_directory, find_files, read_lines
- Plan-execute-observe loop (max 10 iterations)
- Robust tool call parsing (handles XML tags, JSON, markdown fences, missing closing tags)
- Auto-verify file syntax after edits (Rust, Python, JS, JSON, YAML, TOML)
- Git safety net (auto-stash on start, /agent undo to rollback)
- Project map injection (file tree + key symbols)
- Provider-aware context compaction
- Works with any provider (not just Claude Code)

#### Smart Prompts
- 166 expert prompts across 28 categories
- Categories: Engineering, Coding, Writing, Design, Git, Rust, Python, TypeScript, Go, UI/UX, Testing, Security, API, Database, Cloud, DevOps, Business, Marketing, and more
- Each prompt is 5-15 lines of detailed system instructions
- Nerve Bar (Ctrl+K) with fuzzy search, category tabs, and template preview
- Custom prompts via TOML files

#### Developer Workflow
- File context: /file, /files, @file inline references
- Shell integration: /run, /test, /build, /diff, /pipe, /git
- Workspace detection (Rust, Node, Python, Go, Java, Ruby, Elixir, Zig, C#, C++)
- Project scaffolding: 8 templates + AI-generated /scaffold
- Tab completion for file paths and slash commands

#### Productivity
- Persistent sessions (auto-save, resume with --continue)
- Conversation branching (/branch save, restore, diff)
- Conversation history browser (Ctrl+O) with search, sort, delete confirmation
- Clipboard manager (Ctrl+B) with fuzzy search
- In-chat search (Ctrl+F)
- Stop (Esc), regenerate (Ctrl+R), edit (Ctrl+E) responses
- Export conversations as markdown
- Knowledge base (RAG) with document ingestion
- Automations (5 built-in multi-step pipelines)
- Command aliases (/alias)

#### Settings & Customization
- Interactive settings overlay (Ctrl+,) with 4 tabs
- 10 color themes (Catppuccin, Tokyo Night, Gruvbox, Nord, Solarized, Dracula, One Dark, Rose Pine, High Contrast, Monochrome)
- TOML configuration at ~/.config/nerve/config.toml
- Plugin system (executable scripts in ~/.config/nerve/plugins/)

#### Token Management
- Provider-aware context limits (Claude 200K, OpenAI 60K, OpenRouter 30K, Ollama 32K)
- Auto-compaction with smart summarization
- Tool result compaction in agent mode
- Usage tracking (/usage) and spending limits (/limit)
- max_tokens on OpenAI/OpenRouter requests

#### Security
- Shell injection blocking (30+ dangerous patterns)
- Protected system paths (agent can't write to /etc, /usr, /bin, etc.)
- Sensitive file blocking (.env, SSH keys, .aws/credentials)
- Tool execution rate limiting (100/session)
- Config file permissions (0600 on Unix)
- API key masking in display

#### CLI
- Non-interactive mode: nerve -n "prompt"
- Pipe mode: cat file | nerve --stdin -n "review"
- Daemon mode: nerve --daemon / --query / --stop-daemon
- Resume sessions: nerve --continue
- Provider/model override: nerve --provider ollama --model llama3

#### Quality
- 820 tests across 42 source files (now 1,345 tests across 55 files)
- 0 clippy warnings
- 0 unsafe code
- 0 panics in production code
- Graceful error handling with helpful messages
- CI/CD with GitHub Actions (build, test, clippy, auto-release)

[Unreleased]: https://github.com/Artaeon/nerve/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Artaeon/nerve/releases/tag/v0.1.0
