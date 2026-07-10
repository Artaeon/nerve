<div align="center">

# Nerve

### The AI coding assistant that lives in your terminal.

[![CI](https://github.com/Artaeon/nerve/actions/workflows/ci.yml/badge.svg)](https://github.com/Artaeon/nerve/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-2563EB.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Rust-F74C00.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/Artaeon/nerve?color=7C3AED&label=Download)](https://github.com/Artaeon/nerve/releases/latest)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

**47K lines of Rust | 1,970+ tests | 196 prompts | 12 agent tools | 6 providers | per-project memory | multi-agent /workflow | 24/7 server | 6.8 MB binary**

[Install](#install) | [Quick Start](#quick-start) | [Features](#features) | [Agent Mode](#agent-mode) | [Manifesto](MANIFESTO.md)

</div>

---

Nerve is a fast, open-source AI coding assistant built entirely in Rust. It runs in your terminal, connects to 6 AI providers, ships 196 expert prompts, and includes a full coding agent with 12 tools plus a multi-agent (planner -> coder -> reviewer) `/workflow` pipeline -- all in a single 6.8 MB binary with zero runtime dependencies. It works out of the box: it health-checks your providers on startup, auto-activates the agent inside a project, and remembers what it learns about your repo across sessions in a per-project `.nerve/` directory.

It is designed for developers who think in keystrokes, not clicks.

---

## Install

### Pre-built binaries (recommended)

Download from [GitHub Releases](https://github.com/Artaeon/nerve/releases/latest):

```bash
# Linux
curl -LO https://github.com/Artaeon/nerve/releases/latest/download/nerve-linux-x86_64.tar.gz
tar xzf nerve-linux-x86_64.tar.gz && mv nerve ~/.local/bin/

# macOS (Apple Silicon)
curl -LO https://github.com/Artaeon/nerve/releases/latest/download/nerve-macos-arm64.tar.gz
tar xzf nerve-macos-arm64.tar.gz && mv nerve /usr/local/bin/

# macOS (Intel)
curl -LO https://github.com/Artaeon/nerve/releases/latest/download/nerve-macos-x86_64.tar.gz
tar xzf nerve-macos-x86_64.tar.gz && mv nerve /usr/local/bin/
```

Windows: download `nerve-windows-x86_64.zip` from the [releases page](https://github.com/Artaeon/nerve/releases/latest).

Every release includes SHA256 checksums.

### From source

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

Requires Rust 1.85+ (edition 2024). Install with [rustup](https://rustup.rs/).

---

## Quick Start

```bash
nerve                                        # Launch (defaults to Claude Code)
nerve --provider ollama --model llama3       # Use a local model
nerve -n "explain TCP vs UDP"                # One-shot (prints and exits)
cat src/main.rs | nerve --stdin -n "review"  # Pipe files in
git diff | nerve --stdin -n "commit message" # Generate commit messages
nerve --continue                             # Resume last session
```

---

## Features

### 6 AI Providers

Switch instantly with `Ctrl+T`:

| Provider | Setup | Cost |
|----------|-------|------|
| **Claude Code** | [Install CLI](https://claude.ai/code) | Subscription |
| **Ollama** | `ollama pull llama3` | Free (local) |
| **OpenAI** | `OPENAI_API_KEY=sk-...` | Per token |
| **OpenRouter** | `OPENROUTER_API_KEY=sk-or-...` | Per token |
| **GitHub Copilot** | [gh CLI](https://cli.github.com/) + Copilot | Subscription |
| **Custom** | Any OpenAI-compatible endpoint | Varies |

### Works out of the box

Open Nerve in a project and just describe what you want -- no setup ritual:

- **Provider health checks.** On startup Nerve verifies your provider can actually work (the `claude` CLI on PATH, an Ollama server reachable, an API key present). If the default can't run, it automatically falls back to the best available provider instead of failing on your first prompt. No provider at all? You get friendly, specific setup guidance.
- **Agent activates itself.** Inside any git repo or recognized project, a coding request ("fix the failing login test") turns on the agent automatically -- it reads, edits, and runs code without `/agent on`. Genuinely conversational messages (questions, translations) stay chat-only.
- **Any repo is a workspace.** Even a plain `git init` folder with no manifest is detected; the language is inferred from your source files.

### Agent Mode

Describe a task in a project and Nerve works autonomously (or type `/agent on` anywhere). The agent has 12 tools:

| Tool | What it does |
|------|-------------|
| `read_file` | Read any file into context |
| `write_file` | Create new files |
| `edit_file` | Surgical edits to existing files |
| `read_lines` | Read specific line ranges |
| `run_command` | Execute shell commands |
| `list_files` | Browse directory contents |
| `search_code` | Ripgrep-powered code search |
| `find_files` | Glob pattern file search |
| `create_dir` | Create directories |
| `web_search` | Search the web via DuckDuckGo |
| `remember` | Persist a project fact to `.nerve/memory.md` |
| `update_tasks` | Maintain the persistent task backlog |

Every agent write is recorded to a change journal (`/changes`). Roll back with `/agent undo`. Inspect changes with `/agent diff`. `/agent commit` runs your project's tests first and refuses to commit on red (`/agent commit force` overrides).

```
> "The login endpoint returns 500 for invalid credentials. Fix it and add tests."
```

The agent reads the code, writes a fix, runs the tests, and reports back.

### Persistent project memory

Nerve keeps a per-project `.nerve/` directory that is injected into every prompt, so it remembers your repo across sessions:

- `/init` -- analyze the repo once and save an engineering brief
- `/remember <fact>` -- persist a convention or gotcha
- `/decision <text>` -- record a decision (last 5 are always in context)
- `/task <title>` / `/tasks` -- a task backlog that **survives sessions** (unlike ephemeral todo lists)
- `/improve <idea>` -- an improvement backlog
- `/changes` -- the audit trail of what the agent changed

For security, `.nerve/` is write-protected from the agent's file tools -- a prompt-injected model can't plant persistent instructions; all writes go through Nerve's own sanitized API.

### 196 Smart Prompts

Press `Ctrl+K` to open the prompt library. 34 categories of expert-level prompts:

Engineering, Rust, Python, TypeScript, Go, Testing, Security, API, Database, Cloud, DevOps, UI/UX, Code Review, Debugging, Performance, Migration, Business, and more.

Each prompt is 5-15 lines of carefully crafted instructions -- not vague suggestions, but real engineering guidance.

Don't know which prompt to pick? Type `/suggest <description>` and Nerve ranks the library against your query (BM25, runs locally, ~1ms).

### Multi-agent workflow

For non-trivial tasks, type `/workflow <task>` to run a 3-role pipeline on a fresh conversation:

1. **Planner** -- reads the repo (read-only tools) and writes a numbered plan grounded in your actual code
2. **Approval gate** -- the plan is shown and **nothing runs until you `/approve`** (or `/reject`)
3. **Coder** -- executes the approved plan (full tools)
4. **Reviewer** -- inspects the result (read-only tools, enforced at the tool layer)

Reviewer ends with `VERDICT: APPROVED | NEEDS FIXES | REJECTED`. Press Esc at any time to stop.

### 24/7 server — hand work off and close the laptop

The same binary runs as an always-on **server** (`nerve --daemon`) with a
persistent job queue and a worker that executes jobs autonomously. From your
laptop's TUI you connect to it and schedule work — then walk away:

```
/server root@your-server              # connect (a ⛁ live queue indicator appears)
/server submit refactor the auth module and add tests
```

`/server submit` rsyncs your whole project to the server (with `.git` history and
`.nerve/` project memory — nothing lost), queues a job, and runs it on its own
`nerve/job-<id>` branch, committing the result for you to review. The transport
is plain **SSH** — no new ports, no new auth. See **[SERVER.md](SERVER.md)** for
the full setup and workflow.

### Developer Workflow

```bash
/file src/main.rs              # Add file as context
/file src/main.rs:10-50        # Specific line range
/test                          # Auto-detect and run tests
/build                         # Auto-detect and build
/lint                          # Run linter (clippy/eslint/ruff)
/format                        # Run formatter
/search "fn main"              # Ripgrep search
/diff                          # Git diff as context
/commit                        # AI-generated commit message
/map                           # Project structure overview
/web <query>                   # Search the web
```

### Keyboard-Driven

| Key | Action |
|-----|--------|
| `Ctrl+K` | Prompt library |
| `Ctrl+T` | Switch provider |
| `Ctrl+M` | Switch model |
| `Ctrl+N` | New conversation |
| `Ctrl+O` | History browser |
| `Ctrl+F` | Search in conversation |
| `Ctrl+B` | Clipboard manager |
| `Ctrl+,` | Settings overlay |
| `Ctrl+R` | Regenerate response |
| `Ctrl+E` | Edit last message |
| `Esc` | Stop streaming |
| `i / Esc` | Insert / normal mode |
| `j / k` | Scroll (normal mode) |
| `G / g` | Bottom / top |

Full vim-style navigation in normal mode.

### Sessions & Branching

- **Auto-save** -- every conversation is persisted automatically
- **Resume** -- `nerve --continue` picks up where you left off
- **Branch** -- `/branch save` and `/branch restore` for conversation branching
- **History** -- `Ctrl+O` to browse, search, and sort past sessions
- **Export** -- `/export` to save as markdown

### 10 Color Themes

`/theme list` to preview. Includes: Catppuccin Mocha, Tokyo Night, Gruvbox Dark, Nord, Solarized Dark, Dracula, One Dark, Rose Pine, High Contrast, Monochrome.

### Token Tracking

- Real-time cost estimates in the status bar
- `/usage` for detailed token breakdown
- `/limit set 5.00` to cap spending
- Auto-compacting context to stay within provider limits

---

## Configuration

Generated at `~/.config/nerve/config.toml` on first run:

```toml
default_model = "sonnet"
default_provider = "claude_code"
auto_agent = true

# temperature = 0.7
# top_p = 0.9
# context_limit = 64000

[providers.claude_code]
enabled = true

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openai]
api_key = ""   # or OPENAI_API_KEY env var
enabled = false
```

---

## Why Nerve?

| | Nerve | Claude Code | Cursor | Aider |
|---|---|---|---|---|
| Open source | MIT | No | No | Apache-2.0 |
| Terminal-native | Yes | Yes | No | Yes |
| Multi-provider | 6 | 1 | Limited | Multi |
| Local/offline | Ollama | No | No | Ollama |
| Agent tools | 12 | Yes | Yes | Yes |
| Per-project memory | `.nerve/` | CLAUDE.md | No | No |
| Persistent task backlog | Yes | No | No | No |
| Plan-approval gate | Yes | Yes | No | No |
| Built-in prompts | 196 | No | No | No |
| Cost | Free + API | $20/mo | $20/mo | Free + API |
| Binary size | 6.8 MB | ~150 MB | ~500 MB | Python |
| Vim keybindings | Native | No | Plugin | No |

---

## Project Quality

| Metric | Value |
|--------|-------|
| Language | 100% Rust |
| Lines of code | 47,000+ |
| Tests | 1,840 passing |
| Clippy | 0 warnings |
| Unsafe code | 0 blocks |
| Binary (release) | 6.8 MB (LTO + stripped) |

---

## All Commands

<details>
<summary>70+ commands -- click to expand</summary>

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/provider <name>` | Switch AI provider |
| `/model <name>` | Switch model |
| `/agent on\|off` | Toggle coding agent |
| `/agent undo` | Rollback agent changes |
| `/agent diff` | Show agent changes |
| `/agent commit [force]` | Commit agent changes (tests must pass) |
| `/init` | Analyze repo, save engineering brief |
| `/remember <fact>` | Persist a project fact/convention |
| `/memory` | Show project memory |
| `/decision <text>` | Record a decision |
| `/task <title>` | Add a task to the persistent backlog |
| `/tasks` | List the task backlog |
| `/changes` | Show the agent change journal |
| `/workflow <task>` | Run planner -> approve -> coder -> reviewer |
| `/approve` \| `/reject` | Approve or cancel a workflow plan |
| `/file <path>` | Read file as context |
| `/run <cmd>` | Execute command |
| `/test` | Run project tests |
| `/build` | Build project |
| `/lint` | Run linter |
| `/format` | Run formatter |
| `/search <pat>` | Search codebase |
| `/diff` | Git diff as context |
| `/commit [msg]` | Stage + commit |
| `/stage [files]` | Stage files |
| `/stash` | Git stash operations |
| `/gitbranch` | Branch management |
| `/git <subcmd>` | Git operations |
| `/template <name>` | Create from template |
| `/scaffold <desc>` | AI-generated project |
| `/kb add <dir>` | Add to knowledge base |
| `/web <query>` | Search the web |
| `/theme <name>` | Switch color theme |
| `/usage` | Token usage and cost |
| `/limit set <$>` | Set spending limit |
| `/export` | Export as markdown |
| `/branch save` | Save conversation branch |
| `/session save` | Save session |
| `/map` | Project file tree |
| `/alias <n> <cmd>` | Create command alias |
| `/status` | System overview |

Run `/help` inside Nerve for the complete list.

</details>

---

## Project Templates

```bash
/template list
```

| Template | Stack |
|----------|-------|
| `rust-cli` | Rust + clap |
| `rust-lib` | Rust library |
| `rust-web` | Rust + Axum |
| `node-api` | Express + TypeScript |
| `node-react` | React + Vite + TypeScript |
| `python-cli` | Python + argparse |
| `python-api` | FastAPI + Pydantic |
| `go-api` | Go + net/http |

---

## Documentation

- **[User Guide](docs/GUIDE.md)** -- comprehensive guide (2,200+ lines)
- **[Server & client (24/7)](SERVER.md)** -- run nerve on a server, schedule work from your laptop
- **[Manifesto](MANIFESTO.md)** -- why Nerve exists
- **[Changelog](CHANGELOG.md)** -- release history

---

## Contributing

Contributions welcome. Please open an issue first to discuss changes.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo test          # 1,840 tests
cargo clippy        # 0 warnings
cargo fmt --check   # format check
```

---

## License

MIT -- see [LICENSE](LICENSE).

---

<div align="center">

Built by [Artaeon](https://github.com/Artaeon)

</div>
