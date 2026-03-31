<div align="center">

# Nerve

**The open-source AI powerhouse for your terminal.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-461%20passing-brightgreen.svg)](#)
[![CI](https://img.shields.io/badge/CI-GitHub%20Actions-2088FF.svg)](.github/workflows/ci.yml)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

Chat. Code. Ship. All from your terminal.

20K lines of Rust. 461 tests. 134 smart prompts. 7 agent tools.
6 AI providers. 8 project templates. Plugin system. ~7MB binary.

**Nerve is free, open source, and built for developers who live in the terminal.**

[Get Started](#quick-start) | [Why Nerve?](#why-nerve) | [Agent Mode](#agent-mode) | [All Features](#features) | [Contributing](#contributing)

</div>

---

## What is Nerve?

Nerve is a terminal-native AI assistant and autonomous coding agent. It connects to 6 AI providers (Claude, OpenAI, Ollama, OpenRouter, GitHub Copilot, or any custom endpoint), gives the AI tools to read files, write code, and run commands, and wraps it all in a fast, beautiful TUI with vim keybindings.

Think of it as **cursor for the terminal** -- but open source, provider-agnostic, and extensible via plugins.

```
nerve                                    # Launch the TUI
nerve -n "explain this error"            # One-shot from the CLI
cat src/main.rs | nerve --stdin -n "review this"  # Pipe anything in
nerve --continue                         # Resume where you left off
```

---

## Why Nerve?

| What you get | How it works |
|-------------|-------------|
| **No vendor lock-in** | 6 providers. Switch with Ctrl+T. Use Claude today, Ollama tomorrow. |
| **No API key for Claude** | Uses your Claude Code subscription directly. Zero setup. |
| **Autonomous coding agent** | `/agent on` -- the AI reads files, writes code, runs tests, iterates. Works with ANY provider. |
| **Token-efficient** | Provider-aware context management. Auto-compacts to minimize API costs. |
| **134 expert prompts** | Not one-liners. Each is a 5-15 line professional system prompt for code review, architecture, debugging, writing, and more. |
| **Instant and tiny** | ~7MB Rust binary. No Electron, no browser, no runtime dependencies. Starts in milliseconds. |
| **Extensible** | Plugin system. Custom prompts. Custom providers. Your workflow, your way. |
| **Persistent** | Sessions auto-save. Resume with `nerve -c`. Conversation branching. Full history. |

---

## Screenshots

### Chat with Syntax-Highlighted Code

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Nerve v0.1 │ Fix authentication bug          │ Claude Code > sonnet │ 3│
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│ ──────────────────────────────────────────────────────────────────────  │
│  You   2m ago                                                           │
│ │ Can you review this authentication middleware and find the bug?       │
│ │                                                                       │
│ │ ```rust                                                               │
│ │ pub async fn auth_middleware(req: Request) -> Result<Request> {       │
│ │     let token = req.headers().get("Authorization");                  │
│ │     match verify_token(token) {                                      │
│ │         Ok(claims) => Ok(req.with_extension(claims)),                │
│ │         Err(_) => Err(StatusCode::UNAUTHORIZED),                     │
│ │     }                                                                 │
│ │ }                                                                     │
│ │ ```                                                                   │
│                                                                         │
│ ──────────────────────────────────────────────────────────────────────  │
│  AI   1m ago                                                            │
│ │ I found the bug. The `token` variable is `Option<&HeaderValue>`,     │
│ │ but you're passing it directly to `verify_token` without unwrapping. │
│ │                                                                       │
│ │ **Fix:**                                                              │
│ │ ╭─ rust ────────────────────────────────────────────────────────     │
│ │ │  1 │ pub async fn auth_middleware(req: Request) -> Result<...> {   │
│ │ │  2 │     let token = req.headers()                                 │
│ │ │  3 │         .get("Authorization")                                 │
│ │ │  4 │         .and_then(|v| v.to_str().ok())                       │
│ │ │  5 │         .ok_or(StatusCode::UNAUTHORIZED)?;                    │
│ │ │  6 │     let claims = verify_token(token)                          │
│ │ │  7 │         .map_err(|_| StatusCode::UNAUTHORIZED)?;              │
│ │ │  8 │     Ok(req.with_extension(claims))                            │
│ │ │  9 │ }                                                              │
│ │ ╰──────────────────────────────────────────────────────────────     │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│ INS  Type your message... (Enter to send)              (0 words)       │
├─────────────────────────────────────────────────────────────────────────┤
│ Ready │ Claude Code > sonnet │ 142 words │ Conv 1/1                    │
└─────────────────────────────────────────────────────────────────────────┘
```

### Nerve Bar -- 134 Smart Prompts with Category Filtering

```
╭─ Nerve Bar ───────────────────────────────────────────────────────────╮
│ > code review▌                                                         │
│ ─────────────────────────────────────────────────────────────────────  │
│ All  Writing  Coding  Engineering  Design  Git  Rust  ...              │
│ ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│ ► Full Code Review                                  [Engineering]      │
│   Senior-level code review with severity ratings                       │
│                                                                         │
│   PR Review Checklist                             [Best Practices]     │
│   Review changes against best practices                                │
│                                                                         │
│   Rust Code Review                                        [Rust]       │
│   Rust-specific: ownership, lifetimes, idioms                          │
│ ─────────────────────────────────────────────────────────────────────  │
│ Preview:                                                                │
│ You are a senior software engineer performing a thorough code          │
│ review. Analyze the following code with extreme attention to detail... │
│                                                                         │
│ 3/134 prompts                    Tab: category | Enter: use | Esc     │
╰───────────────────────────────────────────────────────────────────────╯
```

---

## Quick Start

```bash
# Install from source
git clone https://github.com/Artaeon/nerve.git
cd nerve && cargo build --release
cp target/release/nerve ~/.local/bin/

# Launch (defaults to Claude Code -- no API key needed)
nerve

# Or use any other provider
nerve --provider ollama --model llama3
OPENAI_API_KEY="sk-..." nerve --provider openai
```

Once inside:
- Type a message and press **Enter** to chat
- Press **Ctrl+K** to open the Nerve Bar (134 smart prompts)
- Type **/agent on** to enable the coding agent
- Type **/help** to see all 42+ slash commands

Resume your last session anytime: `nerve --continue`

---

## Features

### Coding Agent

Enable with `/agent on`. The AI gets 7 tools and autonomously plans, codes, and verifies:

```
/agent on
/file src/auth.rs
"The login endpoint returns 500 for invalid credentials. Fix it and add tests."
```

The agent will read the file, identify the bug, edit the code, write tests, and run them -- all automatically. Max 10 iterations per task. Works with **any provider** (not just Claude).

| Tool | Description |
|------|-------------|
| `read_file` | Read any file |
| `write_file` | Create or overwrite files |
| `edit_file` | Find-and-replace within files |
| `run_command` | Execute shell commands |
| `list_files` | Browse directories |
| `search_code` | Grep across the codebase |
| `create_directory` | Create directory trees |

### 6 AI Providers

| Provider | Auth | Best For |
|----------|------|----------|
| **Claude Code** | Subscription (no API key) | Heavy coding, huge context (1M tokens) |
| **OpenAI** | API key | GPT-4o, reliable and fast |
| **OpenRouter** | API key | 100+ models, cost control |
| **Ollama** | None (local) | Privacy, offline, free |
| **GitHub Copilot** | gh CLI | Quick code suggestions |
| **Custom** | API key + URL | Any OpenAI-compatible endpoint |

Switch instantly: **Ctrl+T** or `/provider <name>`

### 134 Smart Prompts

Not simple one-liners. Each prompt is a 5-15 line expert system prompt across 22 categories:

**Engineering** (24) -- Full Code Review, Architect Solution, Debug Detective, Scaffold Project, API Design, Security Audit, Performance Optimization, CI/CD Pipeline, and more

**Writing** (15) -- Technical Blog Post, Professional Email, Long-Form Article, Summarize, Rewrite

**Design** (10) -- UX Review, Design System, Accessibility Audit, Typography, Wireframes

**Git** (8) -- Commit Message, PR Description, Workflow Design, Changelog, Release Notes

**+ 18 more categories**: Coding, Translation, Analysis, Creative, Productivity, Best Practices, Rust, Data, Business, DevOps, Communication, Learning, Marketing, Legal, Product, Personal

Access via the **Nerve Bar** (Ctrl+K) with fuzzy search, category filtering (Tab to cycle), and live template preview.

### Developer Workflow

```bash
# File context
/file src/main.rs                   # Read file into conversation
/file src/main.rs:42-80             # Specific lines
"review @src/auth.rs for bugs"      # Inline @file references

# Shell integration
/run cargo clippy                   # Run command, show output
/test                               # Auto-detect and run tests
/build                              # Auto-detect and build
/diff                               # Git diff as AI context
/pipe git log --oneline -20         # Any command output as context

# Project scaffolding
/template rust-web myapi            # 8 built-in templates
/scaffold a CLI in Go with cobra    # AI-generated custom projects
```

### Workspace Awareness

Nerve auto-detects your project on startup (Rust, Node, Python, Go, Java, Ruby, Elixir, Zig, C#, C++) and injects relevant context -- project name, dependencies, tech stack -- into the AI system prompt.

### Context Management

Provider-aware token limits keep costs down:

| Provider | Token Limit | Auto-Compact |
|----------|-------------|-------------|
| Claude Code | 200K | At 200K |
| OpenAI | 60K | At 60K |
| OpenRouter | 30K | At 30K |
| Ollama | 8K | At 8K |

Old messages are summarized at sentence boundaries. Agent tool results are compacted aggressively. Check usage with `/tokens`, optimize with `/compact`.

### Productivity

- **Persistent sessions** -- Auto-saves on quit. Resume with `nerve -c`
- **Conversation branching** -- `/branch save`, `/branch restore`, `/branch diff`
- **History browser** (Ctrl+O) -- Search and reload past conversations
- **Clipboard manager** (Ctrl+B) -- Full history with fuzzy search
- **In-chat search** (Ctrl+F) -- Find text across all messages
- **Stop / Regenerate / Edit** -- Esc stops, Ctrl+R regenerates, Ctrl+E edits last message
- **Export** -- `/export` saves as clean markdown
- **Knowledge base** -- `/kb add <dir>` for RAG document ingestion
- **Automations** -- `/auto run "Code Review Pipeline"` for multi-step AI pipelines
- **Plugin system** -- Add custom commands via scripts in `~/.config/nerve/plugins/`

---

## Agent Mode

The most powerful feature. Three real-world examples:

### Fix a Bug
```
/agent on
/file src/api/handler.rs
"The create_user endpoint panics when email is empty. Add validation and return 400."
```

### Add a Feature
```
/agent on
"Add a /metrics endpoint that returns request count, uptime, and memory usage as JSON"
```

### Scaffold and Build
```
/template rust-web myservice
/cd myservice
/agent on
"Add JWT authentication, user registration, and a protected /me endpoint"
```

### Agent vs Code Mode

| | `/agent on` | `/code on` |
|-|-------------|-----------|
| Works with | Any provider | Claude Code only |
| Tools | Prompt-based (7 tools) | Claude's native tools |
| Tokens | Your provider's tokens | Claude subscription |
| Best for | Any provider, token-efficient | Heavy coding with Claude |

---

## Installation

### From Source

```bash
# 1. Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone and build
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release

# 3. Install
cp target/release/nerve ~/.local/bin/

# 4. Verify
nerve --version
```

### Requirements

- Rust 2024 edition (1.85+)
- At least one AI provider:
  - **Claude Code** (recommended, no API key): [claude.ai/code](https://claude.ai/code)
  - **Ollama** (local, free): [ollama.ai](https://ollama.ai)
  - **OpenAI** (API key): [platform.openai.com](https://platform.openai.com)
  - **OpenRouter** (API key): [openrouter.ai](https://openrouter.ai)
  - **GitHub Copilot**: [cli.github.com](https://cli.github.com) with Copilot extension

### Pre-built Binaries

GitHub Releases include pre-built binaries for Linux (x86_64, aarch64) and macOS (x86_64, aarch64). See [Releases](https://github.com/Artaeon/nerve/releases).

---

## All Commands

### Chat & Conversations

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/clear` | Clear conversation |
| `/new` | New conversation |
| `/rename <title>` | Rename conversation |
| `/delete [all]` | Delete conversation(s) |
| `/copy [all\|last]` | Copy to clipboard |
| `/export` | Export as markdown |
| `/system <prompt>` | Set system prompt |
| `/summary` | Conversation statistics |
| `/branch save\|list\|restore\|delete\|diff` | Conversation branching |
| `/session save\|list\|restore` | Session management |

### AI & Agent

| Command | Description |
|---------|-------------|
| `/provider <name>` | Switch provider (claude, openai, ollama, openrouter, copilot) |
| `/model <name>` | Switch model (opus, sonnet, haiku, gpt-4o, llama3, ...) |
| `/agent on\|off` | Toggle coding agent (7 tools, any provider) |
| `/code on\|off` | Toggle code mode (Claude Code only, native tools) |
| `/tokens` | Token usage breakdown with provider limits |
| `/compact` | Compact conversation to save tokens |
| `/context` | Inspect what the AI currently sees |

### Files, Shell & Projects

| Command | Description |
|---------|-------------|
| `/file <path>[:<lines>]` | Read file(s) as context |
| `/run <cmd>` | Execute shell command |
| `/test` | Auto-detect and run tests |
| `/build` | Auto-detect and build |
| `/diff` | Git diff as context |
| `/git <subcmd>` | Git operations |
| `/pipe <cmd>` | Command output as context |
| `/cd <dir>` | Change directory |
| `/workspace` | Show detected project |
| `/template <name>` | Create from template |
| `/scaffold <desc>` | AI-generated project |
| `/url <url>` | Scrape webpage as context |

### Knowledge & Plugins

| Command | Description |
|---------|-------------|
| `/kb add\|search\|list\|status\|clear` | Knowledge base (RAG) |
| `/auto list\|run\|info` | Automations |
| `/plugin list\|init\|reload` | Plugin management |

---

## Keybindings

| Key | Action |
|-----|--------|
| **Ctrl+K** or **/** | Nerve Bar (134 prompts with fuzzy search) |
| **Ctrl+T** | Switch AI provider |
| **Ctrl+M** | Switch model |
| **Ctrl+N** | New conversation |
| **Tab** / **Shift+Tab** | Next / previous conversation |
| **Ctrl+O** | History browser |
| **Ctrl+F** | Search in conversation |
| **Ctrl+B** | Clipboard manager |
| **Ctrl+R** | Regenerate last response |
| **Ctrl+E** | Edit last message |
| **Esc** (streaming) | Stop generation |
| **Shift+Enter** | New line in input |
| **i** / **Esc** | Insert mode / Normal mode (vim) |
| **j** / **k** | Scroll (Normal mode) |

---

## Project Templates

```
/template list
```

| Template | Stack | Includes |
|----------|-------|---------|
| `rust-cli` | Rust + clap | CLI, error handling, tests |
| `rust-lib` | Rust | Library, docs, tests |
| `rust-web` | Rust + Axum | Routes, middleware, error types, graceful shutdown |
| `node-api` | Express + TypeScript | Routes, middleware, validation, Helmet |
| `node-react` | React + Vite + TS | Components, routing |
| `python-cli` | Python + argparse | CLI, pyproject.toml |
| `python-api` | FastAPI + Pydantic | CRUD routes, models, tests |
| `go-api` | Go + net/http | Handlers, middleware, tests |

Each template creates a complete, runnable project with tests, README, and .gitignore.

---

## Configuration

Auto-generated at `~/.config/nerve/config.toml` on first run:

```toml
default_model = "sonnet"
default_provider = "claude_code"

[providers.claude_code]
enabled = true

[providers.openai]
api_key = ""   # or set OPENAI_API_KEY env var
base_url = "https://api.openai.com/v1"
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openrouter]
api_key = ""   # or set OPENROUTER_API_KEY env var
enabled = false
```

See [docs/GUIDE.md](docs/GUIDE.md) for the full configuration reference.

---

## Documentation

The [User Guide](docs/GUIDE.md) (2,200+ lines) covers everything:

- Provider setup tutorials for all 6 providers
- Agent mode walkthrough with real examples
- Context management strategies for token efficiency
- All 134 prompts by category
- Plugin development guide
- Troubleshooting (9 common issues)
- FAQ (13 questions)
- Migration guides from ChatGPT, Copilot, Cursor, and Aider
- Performance tuning

---

## Architecture

```
nerve/
├── src/                         40 files, ~20K lines of Rust
│   ├── main.rs                  Entry point, event loop, 42+ slash commands
│   ├── app.rs                   App state machine (modes, conversations, branching)
│   ├── config.rs                TOML config with auto-generation
│   ├── session.rs               Persistent session save/restore
│   ├── plugins.rs               Plugin loading and execution
│   ├── ai/
│   │   ├── provider.rs          AiProvider trait (dyn-compatible)
│   │   ├── claude_code.rs       Claude Code CLI (subprocess streaming)
│   │   ├── openai.rs            OpenAI-compatible API (SSE streaming)
│   │   └── copilot.rs           GitHub Copilot CLI
│   ├── agent/
│   │   ├── tools.rs             7 agent tools (read, write, edit, run, search, ...)
│   │   └── context.rs           Token estimation, conversation compaction
│   ├── ui/                      8 overlay panels + main layout
│   ├── prompts/                 134 built-in SmartPrompts
│   ├── knowledge/               RAG: ingestion, chunking, search
│   ├── scaffold.rs              8 project templates
│   ├── workspace.rs             Project type detection (10 languages)
│   ├── shell.rs                 Command execution with auto-detection
│   ├── files.rs                 File reading with line ranges
│   └── [8 more modules]
├── docs/GUIDE.md                2,200-line user guide
├── .github/workflows/           CI (build+test+clippy), release, auto-tag
└── prompts/                     Custom prompt templates
```

---

## Integration with Granit

Nerve integrates with [Granit](https://github.com/Artaeon/granit), an open-source terminal knowledge management system (166K+ lines of Go, 12 AI bots, Obsidian-compatible vaults):

- **Now:** Ingest Granit vaults as knowledge bases (`/kb add ~/my-vault`)
- **Planned:** Launch from Granit's command palette, vault-aware context, HTTP/MCP API

---

## Contributing

Nerve is open source and welcomes contributions. Whether it's a bug fix, new prompt, plugin, or feature -- we'd love your help.

### Getting Started

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test    # 461 tests
cargo clippy  # Lint check
```

### Ways to Contribute

- **Report bugs** -- Open an [issue](https://github.com/Artaeon/nerve/issues)
- **Add prompts** -- Create a PR adding SmartPrompts to `src/prompts/builtin.rs`
- **Build plugins** -- Write a plugin and share it (see `/plugin init` for the template)
- **Add templates** -- Add project templates to `src/scaffold.rs`
- **Improve docs** -- The user guide always needs more examples and clarity
- **Add providers** -- Implement `AiProvider` trait for new AI backends
- **Port to new platforms** -- Windows support, ARM builds
- **Spread the word** -- Star the repo, write about Nerve, tell a friend

### Project Principles

- **Terminal-first** -- Everything must work beautifully in a terminal
- **Provider-agnostic** -- Never lock users into one AI vendor
- **Token-efficient** -- Respect users' API budgets
- **Test everything** -- 461 tests and counting
- **Ship fast, stay stable** -- CI runs on every push

---

## Roadmap

- [ ] MCP (Model Context Protocol) client support
- [ ] Windows support
- [ ] `cargo install nerve` on crates.io
- [ ] AUR / Homebrew packages
- [ ] Conversation sharing and collaboration
- [ ] Custom themes (Catppuccin, Tokyo Night, Gruvbox)
- [ ] HTTP API for external tool integration
- [ ] Vector embeddings for smarter RAG
- [ ] Image/multimodal support

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">

**[Star this repo](https://github.com/Artaeon/nerve)** if Nerve helps your workflow.

Built by [Artaeon](https://github.com/Artaeon)

</div>
