<div align="center">

# Nerve

**Raw AI power in your terminal.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-370%20passing-brightgreen.svg)](#)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

A blazing-fast terminal AI assistant. Multi-model, multi-provider, with 130 smart prompts,
knowledge bases, project scaffolding, and a beautiful TUI. ~7MB binary, 370 tests.

[Features](#features) | [Install](#installation) | [Quick Start](#quick-start) | [Commands](#slash-commands) | [Config](#configuration)

</div>

---

## Screenshots

### Chat with Syntax Highlighting

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
│ │ **Issue:** `token` could be `None` if the header is missing,         │
│ │ which would cause a panic or type error.                              │
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

### Nerve Bar -- 130 Smart Prompts

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
│ 3/130 prompts                    Tab: category | Enter: use | Esc     │
╰───────────────────────────────────────────────────────────────────────╯
```

### Welcome Screen

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Nerve v0.1 │ New conversation                 │ Claude Code > sonnet │ 1│
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ╭──────────────────────────────────────────────────────╮              │
│   │                                                      │              │
│   │      ███╗   ██╗███████╗██████╗ ██╗   ██╗███████╗    │              │
│   │      ████╗  ██║██╔════╝██╔══██╗██║   ██║██╔════╝    │              │
│   │      ██╔██╗ ██║█████╗  ██████╔╝██║   ██║█████╗      │              │
│   │      ██║╚██╗██║██╔══╝  ██╔══██╗╚██╗ ██╔╝██╔══╝      │              │
│   │      ██║ ╚████║███████╗██║  ██║ ╚████╔╝ ███████╗    │              │
│   │      ╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝  ╚═══╝  ╚══════╝    │              │
│   │                                                      │              │
│   │          Raw AI power in your terminal               │              │
│   │                                                      │              │
│   ╰──────────────────────────────────────────────────────╯              │
│                                                                         │
│   Quick Start                                                           │
│   ─────────────────                                                     │
│   Type a message and press Enter to chat                                │
│                                                                         │
│   Ctrl+K    Nerve Bar (command palette)                                 │
│   Ctrl+T    Switch provider                                             │
│   Ctrl+M    Switch model                                                │
│   Ctrl+P    Browse prompts                                              │
│   Ctrl+O    History browser                                             │
│   Ctrl+B    Clipboard manager                                           │
│   Ctrl+H    Help & keybindings                                          │
│                                                                         │
│   /help     Show all slash commands                                     │
│   /url      Scrape a webpage for context                                │
│   /kb       Manage knowledge base                                       │
│   /auto     Run automations                                             │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│ NOR  Type your message... (Enter to send)              (0 words)       │
├─────────────────────────────────────────────────────────────────────────┤
│ Ready │ Claude Code > sonnet │ 0 words │ Conv 1/1                      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Why Nerve?

- **No API keys required** -- Uses your Claude Code subscription natively. Just install Claude Code and go.
- **Blazing fast** -- Rust + ratatui. LTO-stripped ~7MB binary with instant startup.
- **Multi-provider** -- Claude Code, OpenAI, Ollama (local), OpenRouter, GitHub Copilot, any OpenAI-compatible endpoint. Switch on the fly with Ctrl+T.
- **Terminal-native** -- No browser, no Electron. Designed for developers who live in the terminal.
- **Smart, not bloated** -- 130 built-in prompts across 20 categories, knowledge base RAG, automations, project scaffolding. All in a single binary.
- **Open source** -- MIT licensed. Free forever.

---

## Features

### For Developers

- **File Context** (`/file`, `@file`) -- Read any file into the conversation, with optional line ranges (`/file src/main.rs:10-50`)
- **Multi-File Context** (`/files`) -- Read multiple files at once for cross-file analysis
- **Workspace Awareness** (`/workspace`) -- Auto-detects your project type (Rust, Node, Python, Go, etc.)
- **Shell Integration** (`/run`, `/pipe`, `/test`, `/build`) -- Execute commands without leaving Nerve
- **Git Integration** (`/diff`, `/git`) -- Git operations with AI-powered analysis and commit messages
- **Code Mode** (`/code on`) -- Give Claude full file and terminal access (Claude Code only)
- **Project Scaffolding** (`/template`, `/scaffold`) -- Generate projects from 8 built-in templates or AI-generate custom ones

### AI & Prompts

- **Multi-Provider** -- Claude Code (no API key), OpenAI, Ollama, OpenRouter, GitHub Copilot, any OpenAI-compatible endpoint
- **130 Smart Prompts** -- 20 categories: Writing, Coding, Engineering, Analysis, Creative, Productivity, Design, Git, Rust, DevOps, Best Practices, Business, Communication, Data, Learning, Legal, Marketing, Personal, Product, Translation
- **Nerve Bar** (Ctrl+K) -- Fuzzy command palette with category tabs, search, and prompt preview
- **Streaming** -- Responses render token-by-token with full syntax highlighting via syntect
- **Knowledge Base** (`/kb`) -- RAG with document ingestion, chunking, fuzzy search, and auto-context injection
- **Automations** (`/auto`) -- Multi-step AI pipelines that chain prompts together

### Productivity

- **Conversation History** (Ctrl+O) -- Browse, search, and reload past conversations grouped by date
- **Clipboard Manager** (Ctrl+B) -- Full clipboard history with fuzzy search and source tracking
- **Search** (Ctrl+F) -- Find text within conversations
- **Stop / Regenerate / Edit** -- Esc to stop streaming, Ctrl+R to regenerate, Ctrl+E to edit last message
- **Vim Keybindings** -- i/Esc modes, j/k scroll, / opens Nerve Bar, x deletes last exchange
- **Export** (`/export`) -- Save conversations as markdown
- **System Prompts** (`/system`) -- Set, view, or clear per-conversation system prompts
- **Web Scraping** (`/url`) -- Fetch any URL and inject the content as AI context

---

## Installation

### From Source

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

### Arch Linux (AUR)

Coming soon.

### Requirements

- Rust 2024 edition (1.85+)
- At least one AI provider:
  - **Claude Code** (recommended) -- Install from [https://claude.ai/code](https://claude.ai/code)
  - **Ollama** -- Install from [https://ollama.ai](https://ollama.ai)
  - **OpenAI** or **OpenRouter** API key

---

## Quick Start

```bash
# Launch (defaults to Claude Code -- no API key needed)
nerve

# Read a file and ask about it
/file src/main.rs
"What does this code do?"

# Run tests and ask about failures
/test
"Why did test X fail?"

# Review git changes
/diff
"Write a commit message for these changes"

# Scaffold a new project
/template rust-cli myapp
# Or use AI to generate custom projects
/code on
/scaffold a REST API in Rust with JWT auth and Postgres

# Use smart prompts
# Press Ctrl+K, type "review", select "Full Code Review"
```

### CLI & Pipe Mode

```bash
# One-shot mode: get an answer and exit
nerve -n "explain the difference between TCP and UDP"

# Pipe mode: feed content via stdin
cat src/main.rs | nerve --stdin -n "review this code"
git diff | nerve --stdin -n "write a commit message for these changes"

# Switch provider from CLI
nerve --provider ollama --model llama3
nerve --provider openai --model gpt-4o

# List available models
nerve --list-models
```

### Daemon Mode

```bash
# Start the daemon (listens on /tmp/nerve.sock)
nerve --daemon

# Send queries to the running daemon
nerve --query "explain monads"

# Stop the daemon
nerve --stop-daemon
```

---

## Slash Commands

### Chat

| Command | Description |
|---------|-------------|
| `/help` | Show all available commands |
| `/clear` | Clear current conversation |
| `/new` | Start new conversation |
| `/delete` | Delete current conversation |
| `/delete all` | Delete all conversations |
| `/rename <title>` | Rename current conversation |
| `/export` | Export conversation to markdown |
| `/copy` | Copy last AI response to clipboard |
| `/copy all` | Copy entire conversation |
| `/copy last` | Copy last message (any role) |
| `/system <prompt>` | Set system prompt for conversation |
| `/system clear` | Remove system prompt |

### AI Provider

| Command | Description |
|---------|-------------|
| `/provider <name>` | Switch provider |
| `/providers` | List available providers |
| `/model <name>` | Switch model |
| `/models` | List available models |
| `/code on\|off` | Toggle code mode (Claude Code only) |
| `/cwd <dir>` | Set working directory for code mode |

### Knowledge & Context

| Command | Description |
|---------|-------------|
| `/file <path>` | Read file as context |
| `/file <path>:S-E` | Read specific line range |
| `/files <p1> <p2>` | Read multiple files |
| `/kb add <dir>` | Add directory to knowledge base |
| `/kb search <query>` | Search knowledge base |
| `/kb list` | List knowledge bases |
| `/kb status` | Show KB statistics |
| `/kb clear` | Clear knowledge base |
| `/url <url> [question]` | Scrape URL for context |

### Shell & Git

| Command | Description |
|---------|-------------|
| `/run <command>` | Run shell command and show output |
| `/pipe <command>` | Run command and add output as context |
| `/diff [args]` | Show git diff (adds as context) |
| `/test` | Auto-detect and run project tests |
| `/build` | Auto-detect and run project build |
| `/git [subcommand]` | Quick git operations (status/log/diff/branch) |

### Project Scaffolding

| Command | Description |
|---------|-------------|
| `/template list` | List available project templates |
| `/template <name> [dir]` | Create project from template |
| `/scaffold <description>` | AI-generate a project from description |

### Automation

| Command | Description |
|---------|-------------|
| `/auto list` | List automations |
| `/auto run <name>` | Run automation |
| `/auto info <name>` | Show automation details |
| `/auto create <name>` | Create custom automation |
| `/auto delete <name>` | Delete custom automation |

### Workspace

| Command | Description |
|---------|-------------|
| `/workspace` | Show detected project info |
| `/status` | Show system status |

---

## Keybindings

### General

| Key | Action |
|-----|--------|
| `Ctrl+C` | Quit |
| `Ctrl+H` | Toggle help |
| `Ctrl+K` or `/` | Open Nerve Bar |
| `Ctrl+T` | Switch provider |
| `Ctrl+M` | Switch model |
| `Ctrl+N` | New conversation |
| `Tab` | Next conversation |
| `Shift+Tab` | Previous conversation |
| `Ctrl+O` | History browser |
| `Ctrl+P` | Prompt picker |
| `Ctrl+B` | Clipboard manager |
| `Ctrl+F` | Search in conversation |
| `Ctrl+R` | Regenerate last response |
| `Ctrl+E` | Edit last message |

### Navigation (Normal Mode)

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `j` / `k` | Scroll down / up |
| `x` | Delete last exchange |
| `q` | Quit |

### Editing (Insert Mode)

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Esc` | Normal mode (or stop streaming) |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+Y` | Copy last AI response |
| `Ctrl+L` | Clear conversation |

---

## AI Providers

Nerve supports multiple AI backends and lets you switch between them at runtime with Ctrl+T:

| Provider | Type | Auth |
|----------|------|------|
| **Claude Code** (default) | CLI integration | Uses your Claude Code subscription -- no API key needed |
| **OpenAI** | API | `OPENAI_API_KEY` environment variable or config |
| **OpenRouter** | API | `OPENROUTER_API_KEY` environment variable or config |
| **Ollama** | Local | No key needed -- runs on `localhost:11434` |
| **GitHub Copilot** | CLI integration | `gh` CLI with Copilot extension installed |
| **Custom** | OpenAI-compatible | Any base URL + API key via `config.toml` |

---

## Configuration

Configuration lives at `~/.config/nerve/config.toml` and is auto-generated on first run.

```toml
# Nerve - configuration file
#
# Paths:
#   Config  : ~/.config/nerve/config.toml
#   Prompts : ~/.config/nerve/prompts/   (custom .toml prompt files)
#   History : ~/.local/share/nerve/history/

default_model = "sonnet"
default_provider = "claude_code"

[theme]
user_color = "#89b4fa"
assistant_color = "#a6e3a1"
border_color = "#585b70"
accent_color = "#cba6f7"

[providers.claude_code]
enabled = true

[providers.openai]
api_key = ""
base_url = "https://api.openai.com/v1"
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openrouter]
api_key = ""
base_url = "https://openrouter.ai/api/v1"
enabled = false

# To add a custom OpenAI-compatible provider:
#
# [[providers.custom]]
# name = "my-provider"
# api_key = "sk-..."
# base_url = "https://api.example.com/v1"

[keybinds]
command_bar = "ctrl+k"
new_conversation = "ctrl+n"
prompt_picker = "ctrl+p"
model_select = "ctrl+m"
help = "f1"
copy_last = "ctrl+shift+c"
quit = "ctrl+q"
```

### Provider Setup

#### Claude Code (default)

No configuration needed. Nerve calls the `claude` CLI directly and uses your existing Claude Code subscription. Just make sure `claude` is in your PATH.

#### OpenAI

Set your API key in the config or via environment variable:

```bash
export OPENAI_API_KEY="sk-..."
nerve --provider openai
```

#### OpenRouter

```bash
export OPENROUTER_API_KEY="sk-or-..."
nerve --provider openrouter
```

#### Ollama

Start Ollama locally and point Nerve at it:

```bash
ollama serve
nerve --provider ollama --model llama3
```

#### GitHub Copilot

Requires the `gh` CLI with the Copilot extension:

```bash
gh extension install github/gh-copilot
nerve --provider copilot
```

#### Custom OpenAI-Compatible Provider

Add a section to your `config.toml`:

```toml
[[providers.custom]]
name = "my-provider"
api_key = "sk-..."
base_url = "https://api.example.com/v1"
```

Then use it with `nerve --provider my-provider`.

---

## Smart Prompts

130 built-in prompt templates across 20 categories:

| Category | Examples |
|----------|----------|
| **Writing** | Summarize, Expand, Rewrite, Fix Grammar, Improve Clarity, Proofread |
| **Coding** | Explain Code, Fix Bug, Refactor, Add Comments, Write Tests |
| **Engineering** | Code Review, Architecture Review, Performance Analysis, Tech Debt |
| **Analysis** | Sentiment, Key Points, SWOT, Fact Check, Root Cause |
| **Creative** | Brainstorm, Story, Metaphor, Names, Poetry, Slogan |
| **Productivity** | Action Items, Meeting Summary, Decision Matrix, Report |
| **Design** | UI Review, Accessibility Audit, Animation Spec |
| **Git** | Commit Message, PR Description, Changelog |
| **Rust** | Rust Code Review, Ownership Analysis, Idiomatic Rust |
| **DevOps** | Dockerfile Review, CI/CD Pipeline, Infrastructure |
| **Best Practices** | PR Review Checklist, API Design, Error Handling |
| **Translation** | English, Spanish, French, German, Japanese, Arabic |
| **Business** | Business Plan, Market Analysis, Competitive Analysis |
| **Communication** | Presentation, Technical Writing, Documentation |
| **Data** | Data Analysis, SQL Review, Schema Design |
| **Learning** | Explain Like I'm 5, Tutorial, Study Guide |
| **Legal** | Contract Review, Privacy Policy, Terms of Service |
| **Marketing** | Ad Copy, SEO, Social Media |
| **Personal** | Resume, Cover Letter, Career Advice |
| **Product** | PRD, User Stories, Feature Spec |

Browse prompts with the Prompt Picker (Ctrl+P) or search them through the Nerve Bar (Ctrl+K). Create your own reusable prompts as TOML files in `~/.config/nerve/prompts/`:

```toml
name = "My Custom Prompt"
description = "Does something useful"
template = "Please do the following with this input:\n\n{{input}}"
category = "Custom"
tags = ["custom", "utility"]
```

---

## Automations

Multi-step AI pipelines that chain prompts together, passing the output of each step as input to the next:

| Automation | Steps | Description |
|------------|-------|-------------|
| **Code Review Pipeline** | Analyze, Fix, Generate | Analyze code for bugs, suggest fixes, generate corrected code |
| **Content Optimizer** | Analyze, Rewrite, Summarize | Analyze content for clarity, rewrite, create summary and headlines |
| **Research Assistant** | Questions, Analysis, Synthesis | Break down topic, analyze each question, synthesize findings |
| **Email Drafter** | Context, Draft | Analyze context for tone, draft professional email |
| **Translate & Localize** | Translate, Localize | Translate text, review for cultural nuances |

Create custom automations as TOML files in `~/.config/nerve/automations/`.

---

## Project Templates

Scaffold new projects instantly with 8 built-in templates:

| Template | Language | Description |
|----------|----------|-------------|
| `rust-cli` | Rust | CLI application with clap, error handling, and tests |
| `rust-lib` | Rust | Library crate with public API and documentation |
| `rust-web` | Rust | Web server with axum, routes, and middleware |
| `node-api` | Node.js | Express REST API with middleware and error handling |
| `node-react` | Node.js | React application with modern tooling |
| `python-cli` | Python | CLI application with argument parsing |
| `python-api` | Python | FastAPI REST API with async support |
| `go-api` | Go | HTTP API with standard library |

```bash
# Use a built-in template
/template rust-cli myapp

# Or let AI generate a custom project
/scaffold a REST API in Rust with JWT auth and Postgres
```

---

## Architecture

```
nerve/
├── src/                          34 files, ~16K lines of Rust
│   ├── main.rs                   Entry point, event loop, slash commands
│   ├── app.rs                    Application state machine
│   ├── config.rs                 TOML configuration (load/save/defaults)
│   ├── daemon.rs                 Background daemon (Unix socket IPC)
│   ├── automation.rs             Multi-step AI pipelines
│   ├── history.rs                Conversation persistence
│   ├── clipboard.rs              System clipboard integration
│   ├── clipboard_manager.rs      Clipboard history with fuzzy search
│   ├── keybinds.rs               Keybind string parser
│   ├── files.rs                  File reading with line ranges
│   ├── shell.rs                  Shell command execution
│   ├── workspace.rs              Project type detection
│   ├── scaffold.rs               Project scaffolding (8 templates)
│   ├── ai/
│   │   ├── mod.rs                Module exports
│   │   ├── provider.rs           AiProvider trait + message types
│   │   ├── claude_code.rs        Claude Code CLI integration
│   │   ├── copilot.rs            GitHub Copilot CLI integration
│   │   └── openai.rs             OpenAI-compatible API client
│   ├── ui/
│   │   ├── mod.rs                Layout and rendering dispatch
│   │   ├── chat.rs               Syntax-highlighted chat view
│   │   ├── command_bar.rs        Nerve Bar (fuzzy command palette)
│   │   ├── prompt_picker.rs      SmartPrompt browser
│   │   ├── history_browser.rs    Conversation history viewer
│   │   ├── clipboard_manager.rs  Clipboard history overlay
│   │   ├── search.rs             In-conversation search
│   │   └── help.rs               Keybinding reference overlay
│   ├── prompts/
│   │   ├── mod.rs                Prompt loading and category listing
│   │   ├── builtin.rs            130 built-in prompt templates
│   │   └── custom.rs             User-defined prompt loading
│   ├── knowledge/
│   │   ├── mod.rs                Module exports
│   │   ├── store.rs              Knowledge base storage
│   │   ├── ingest.rs             Document ingestion and chunking
│   │   └── search.rs             Fuzzy search across knowledge base
│   └── scraper/
│       ├── mod.rs                Module exports
│       └── web.rs                URL fetching and content extraction
├── prompts/                      Example prompt templates
├── assets/                       ASCII art and screenshots
├── Cargo.toml
├── LICENSE
└── README.md
```

---

## Integration with Granit

Nerve is designed as a standalone tool that integrates with [Granit](https://github.com/Artaeon/granit), an open-source knowledge management system.

**Current integration:**

- Nerve can ingest Granit vault directories as knowledge bases (`/kb add ~/my-vault`)
- Both tools are terminal-native and share the same philosophy

**Planned integration:**

- Launch Nerve from within Granit's command palette
- Vault-aware AI context (Nerve reads your notes for RAG)
- HTTP/MCP API for tighter bidirectional communication

---

## Documentation

See the [User Guide](docs/GUIDE.md) for comprehensive documentation including:
- Provider setup for all 6 providers (Claude Code, OpenAI, Ollama, OpenRouter, GitHub Copilot, custom)
- Agent mode tutorial
- Context management for token efficiency
- Shell and file integration workflows
- 130 smart prompts reference
- Tips and tricks for power users

---

## Contributing

Contributions are welcome. Please open an issue first to discuss what you would like to change.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test
```

The test suite covers 370 tests across all 34 source files.

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">

Built by [Artaeon](https://github.com/Artaeon) -- [raphael.lugmayr@stoicera.com](mailto:raphael.lugmayr@stoicera.com)

</div>
