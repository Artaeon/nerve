<div align="center">

# Nerve

**Raw AI power in your terminal.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-450%20passing-brightgreen.svg)](#)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

A blazing-fast terminal AI assistant and coding agent built in Rust.
6 providers, 130 smart prompts, agent mode with 7 tools, project scaffolding,
knowledge bases, and a beautiful TUI. 18K lines of Rust, 450 tests, ~7MB binary.

[Features](#features) | [Install](#installation) | [Quick Start](#quick-start) | [Agent Mode](#agent-mode) | [Docs](docs/GUIDE.md)

</div>

---

## Why Nerve?

Most AI tools are either cloud-only, Electron-bloated, or require complex setup. Nerve is different:

- **No API keys for Claude** -- Uses your Claude Code subscription directly. Just install Claude Code and go.
- **6 AI providers** -- Claude Code, OpenAI, OpenRouter, Ollama (local), GitHub Copilot, or any custom endpoint. Switch instantly with Ctrl+T.
- **Full coding agent** -- Not just chat. Nerve can read files, write code, run commands, search your codebase, and iterate automatically.
- **Token-efficient** -- Provider-aware context management. Auto-compacts conversations to minimize API costs on OpenAI/OpenRouter.
- **130 smart prompts** -- Professional templates for code review, architecture design, debugging, writing, marketing, and more. Each prompt is 5-15 lines of detailed instructions.
- **Terminal-native** -- ~7MB Rust binary. Instant startup. Vim keybindings. No browser, no Electron, no subscriptions beyond your AI provider.

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

### Nerve Bar -- 130 Smart Prompts with Categories

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

## Features

### AI Providers

| Provider | Auth | Models | Context | Best For |
|----------|------|--------|---------|----------|
| Claude Code | Subscription | opus, sonnet, haiku | 200K-1M | Heavy coding, large context |
| OpenAI | API key | gpt-4o, gpt-4o-mini | 128K | Reliable, widely supported |
| OpenRouter | API key | 100+ models | Varies | Model variety, cost control |
| Ollama | None (local) | llama3, mistral, etc. | 8K-128K | Privacy, offline, free |
| GitHub Copilot | gh CLI | copilot | 8K | Quick suggestions |
| Custom | API key + URL | Any OpenAI-compatible | Varies | Self-hosted, specialized |

Switch providers at any time:

```
Ctrl+T               # Visual provider picker
/provider ollama      # Command
nerve --provider openai --model gpt-4o  # CLI flag
```

### Coding Agent (`/agent on`)

Nerve becomes a full coding agent that can autonomously:

| Tool | What It Does |
|------|-------------|
| `read_file` | Read any file in the project |
| `write_file` | Create or overwrite files |
| `edit_file` | Find-and-replace within files |
| `run_command` | Execute shell commands |
| `list_files` | Browse directory contents |
| `search_code` | Grep across the codebase |
| `create_directory` | Create directory trees |

The agent runs a plan-execute-observe loop (max 10 iterations):

1. You describe the task
2. AI reads relevant files to understand the code
3. AI makes changes using tools
4. AI verifies changes (reads files, runs tests)
5. Repeats until complete

Works with ANY provider -- not just Claude Code. The tool format is prompt-based, so it works with OpenAI, Ollama, etc.

### Smart Prompts (130 across 22 categories)

Access via the Nerve Bar (Ctrl+K) with fuzzy search and category filtering:

| Category | Count | Examples |
|----------|-------|---------|
| Engineering | 20 | Full Code Review, Architect Solution, Debug Detective |
| Coding | 12 | Explain Code, Fix Bug, Refactor, Write Tests |
| Writing | 15 | Technical Blog Post, Professional Email, Long-Form Article |
| Design | 10 | UX Review, Design System, Accessibility Audit |
| Git | 8 | Commit Message, PR Description, Git Workflow |
| Best Practices | 10 | SOLID Principles, Clean Code, PR Review |
| Rust | 2 | Rust Code Review, Rust From Scratch |
| DevOps | 2 | Incident Postmortem, Infrastructure as Code |
| Business | 2 | Business Case, Meeting Notes to Actions |
| Data | 2 | SQL Query Builder, Data Pipeline Design |
| Marketing | 2 | Landing Page Copy, Content Calendar |
| Product | 2 | PRD, User Research Questions |
| [+ 10 more...] | | Translation, Analysis, Creative, Productivity, etc. |

Each prompt is a detailed 5-15 line system prompt -- not a simple one-liner.

### Developer Workflow

```bash
# Read files into context
/file src/main.rs              # Full file
/file src/main.rs:10-50        # Specific lines
/files src/lib.rs src/app.rs   # Multiple files
"review @src/main.rs"          # Inline @file reference

# Run commands and use output as context
/run cargo test                # Show output
/test                          # Auto-detect test runner
/build                         # Auto-detect build tool
/diff                          # Git diff as AI context
/pipe cargo clippy             # Command output as context

# Project scaffolding
/template list                 # 8 built-in templates
/template rust-web myapi       # Create from template
/scaffold a CLI tool in Go     # AI-generated project
```

### Workspace Awareness

Nerve auto-detects your project type on startup:

- Reads Cargo.toml, package.json, pyproject.toml, go.mod, etc.
- Extracts project name, description, and key dependencies
- Injects project context into the AI system prompt
- Supports: Rust, Node.js, Python, Go, Java, Ruby, Elixir, Zig, C#, C++

### Context Management

Token-efficient by design:

- Provider-aware limits (Claude 200K, OpenAI 60K, OpenRouter 30K, Ollama 8K)
- Auto-compacts old messages by summarizing at sentence boundaries
- Tool results from agent mode compacted aggressively
- `/tokens` shows usage breakdown, `/compact` for manual optimization

### Productivity

- **Nerve Bar** (Ctrl+K) -- Fuzzy command palette with category tabs and template preview
- **Conversation History** (Ctrl+O) -- Browse, search, reload past conversations
- **Clipboard Manager** (Ctrl+B) -- Full history with fuzzy search
- **In-Chat Search** (Ctrl+F) -- Find text across messages
- **Stop/Regen/Edit** -- Esc stops, Ctrl+R regenerates, Ctrl+E edits
- **Export** (`/export`) -- Save conversations as clean markdown
- **Knowledge Base** (`/kb`) -- RAG with document ingestion
- **Automations** (`/auto`) -- Multi-step AI pipelines

---

## Installation

### From Source

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

### Requirements

- Rust 2024 edition (1.85+)
- At least one AI provider:
  - **Claude Code** (recommended): [Install Claude Code](https://claude.ai/code)
  - **Ollama** (local, free): [Install Ollama](https://ollama.ai)
  - **OpenAI**: Get an API key from [platform.openai.com](https://platform.openai.com)
  - **OpenRouter**: Get a key from [openrouter.ai](https://openrouter.ai)
  - **GitHub Copilot**: Install [gh CLI](https://cli.github.com) with Copilot extension

### Build from Source -- Detailed Steps

```bash
# 1. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 2. Clone and build
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release

# 3. Install the binary
# Option A: Copy to a directory in your PATH
cp target/release/nerve ~/.local/bin/

# Option B: Symlink instead (easier to update with git pull + rebuild)
ln -sf "$(pwd)/target/release/nerve" ~/.local/bin/nerve

# 4. Verify
nerve --help
```

The release build uses LTO (link-time optimization) and symbol stripping to produce
a ~7MB binary with no runtime dependencies beyond libc.

---

## Quick Start

### Interactive TUI

```bash
# Default: Claude Code (no API key needed)
nerve

# With a specific provider
nerve --provider ollama --model llama3
nerve --provider openai --model gpt-4o
OPENROUTER_API_KEY="sk-or-..." nerve --provider openrouter
```

### CLI / Pipe Mode

```bash
# One-shot question
nerve -n "explain the difference between TCP and UDP"

# Pipe a file
cat src/main.rs | nerve --stdin -n "review this code for bugs"

# Pipe git diff
git diff | nerve --stdin -n "write a commit message for these changes"

# Pipe error logs
cargo test 2>&1 | nerve --stdin -n "explain why these tests failed"

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

### Inside the TUI

```
# Chat normally
"How do I implement a binary search tree in Rust?"

# Use file context
/file src/auth.rs
"Find the bug in this authentication handler"

# Run tests and ask about failures
/test
"Why did test_login fail?"

# Enable agent mode for autonomous coding
/agent on
"Add input validation to the user registration endpoint"

# Use smart prompts
Ctrl+K -> type "review" -> select "Full Code Review"
```

---

## Agent Mode

The most powerful feature. Enable with `/agent on`.

### Example: Fix a Bug

```
/agent on
/file src/auth.rs
"The login function returns 500 instead of 401 for invalid credentials. Fix it."
```

The agent will:

1. Read src/auth.rs to understand the code
2. Identify the bug (wrong error mapping)
3. Edit the file to fix it
4. Read the file again to verify
5. Run tests to confirm

### Example: Add a Feature

```
/agent on
"Add a /health endpoint to the API that returns system status, uptime, and version"
```

### Example: Scaffold and Code

```
/template rust-web myapi
/cd myapi
/agent on
"Add JWT authentication middleware and user registration/login endpoints"
```

### Agent vs Code Mode

| | Agent Mode (`/agent on`) | Code Mode (`/code on`) |
|-|--------------------------|------------------------|
| **Provider** | Any (OpenAI, Ollama, etc.) | Claude Code only |
| **How tools work** | AI outputs `<tool_call>` tags | Claude Code's native tools |
| **Token usage** | Your provider's tokens | Claude subscription |
| **Best for** | Any provider, token-efficient | Heavy coding tasks |

---

## Configuration

Config at `~/.config/nerve/config.toml` (auto-generated on first run):

```toml
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
api_key = ""  # or set OPENAI_API_KEY env var
base_url = "https://api.openai.com/v1"
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openrouter]
api_key = ""  # or set OPENROUTER_API_KEY env var
base_url = "https://openrouter.ai/api/v1"
enabled = false

# Add custom OpenAI-compatible endpoints:
# [[providers.custom]]
# name = "My Provider"
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

---

## Keybindings

### General

| Key | Action |
|-----|--------|
| Ctrl+C / Ctrl+D | Quit (saves state) |
| Ctrl+K or / | Open Nerve Bar |
| Ctrl+T | Switch provider |
| Ctrl+M | Switch model |
| Ctrl+N | New conversation |
| Tab / Shift+Tab | Next/previous conversation |
| Ctrl+O | History browser |
| Ctrl+P | Prompt picker |
| Ctrl+B | Clipboard manager |
| Ctrl+F | Search in conversation |
| Ctrl+H | Toggle help |

### Chat

| Key | Action |
|-----|--------|
| Enter | Send message |
| Shift+Enter | New line in input |
| Esc (streaming) | Stop generation |
| Ctrl+R | Regenerate last response |
| Ctrl+E | Edit last message |
| Ctrl+Y | Copy last AI response |
| Ctrl+L | Clear conversation |
| Ctrl+V | Paste from clipboard |
| Ctrl+W | Delete word before cursor |

### Vim Navigation (Normal mode)

| Key | Action |
|-----|--------|
| i | Enter insert mode |
| j / k | Scroll down / up |
| x | Delete last exchange |
| q | Quit |

---

## All Slash Commands

### Chat

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/clear` | Clear conversation |
| `/new` | New conversation |
| `/rename <title>` | Rename conversation |
| `/delete` | Delete conversation |
| `/delete all` | Delete all conversations |
| `/copy [all\|last]` | Copy to clipboard |
| `/export` | Export as markdown file |
| `/system <prompt>` | Set system prompt |
| `/system clear` | Remove system prompt |
| `/summary` | Conversation statistics |

### AI

| Command | Description |
|---------|-------------|
| `/provider <name>` | Switch provider |
| `/providers` | List providers |
| `/model <name>` | Switch model |
| `/models` | List models |
| `/code on\|off` | Toggle code mode (Claude only) |
| `/agent on\|off` | Toggle agent mode |

### Files and Context

| Command | Description |
|---------|-------------|
| `/file <path>` | Read file as context |
| `/file <path>:<start>-<end>` | Read line range |
| `/files <p1> <p2> ...` | Read multiple files |
| `/url <url> [question]` | Scrape webpage |
| `/workspace` | Show detected project |
| `/cd <dir>` | Change directory |
| `/cwd <dir>` | Set working directory for code mode |

### Shell

| Command | Description |
|---------|-------------|
| `/run <cmd>` | Execute command |
| `/pipe <cmd>` | Command output as context |
| `/test` | Auto-detect and run tests |
| `/build` | Auto-detect and build |
| `/diff [args]` | Git diff as context |
| `/git <subcmd>` | Git operations |

### Knowledge and Automation

| Command | Description |
|---------|-------------|
| `/kb add <dir>` | Ingest documents |
| `/kb search <query>` | Search knowledge base |
| `/kb list` | List knowledge bases |
| `/kb status` | Show KB stats |
| `/kb clear` | Clear knowledge base |
| `/auto list` | List automations |
| `/auto run <name>` | Run automation |
| `/auto info <name>` | Show automation details |
| `/auto create <name>` | Create custom automation |
| `/auto delete <name>` | Delete custom automation |
| `/template list` | List project templates |
| `/template <name>` | Create from template |
| `/scaffold <desc>` | AI-generated project |

### Token Management

| Command | Description |
|---------|-------------|
| `/tokens` | Token usage breakdown |
| `/compact` | Compact conversation |
| `/context` | Inspect context window |
| `/status` | System overview |

---

## Project Templates

8 built-in templates for instant project scaffolding:

| Template | Language | What You Get |
|----------|----------|-------------|
| `rust-cli` | Rust | CLI with clap, error handling, tests |
| `rust-lib` | Rust | Library crate with docs and tests |
| `rust-web` | Rust | Axum web API with handlers and routes |
| `node-api` | Node.js | Express + TypeScript REST API |
| `node-react` | React | Vite + TypeScript + React app |
| `python-cli` | Python | CLI with argparse and pyproject.toml |
| `python-api` | Python | FastAPI with routes and models |
| `go-api` | Go | HTTP API with net/http |

Usage:

```bash
/template rust-web myapi
/cd myapi
cargo build
```

---

## Architecture

```
nerve/
├── src/                          38 files, ~18K lines of Rust
│   ├── main.rs                   Entry point, event loop, 38 slash commands
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
│   ├── agent/
│   │   ├── mod.rs                Module exports
│   │   ├── tools.rs              7 agent tools
│   │   └── context.rs            Token management
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
├── docs/
│   └── GUIDE.md                  Comprehensive user guide
├── Cargo.toml
├── LICENSE
└── README.md
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| ratatui + crossterm | Terminal UI rendering |
| tokio | Async runtime for streaming and I/O |
| reqwest | HTTP client for OpenAI-compatible APIs |
| serde + toml + serde_json | Configuration and API serialization |
| syntect | Syntax highlighting in code blocks |
| pulldown-cmark | Markdown parsing for rich display |
| fuzzy-matcher | Fuzzy search in Nerve Bar and KB |
| clap | CLI argument parsing |
| arboard | System clipboard access |
| chrono | Timestamps for conversations and history |
| uuid | Unique conversation identifiers |
| anyhow + thiserror | Error handling |

---

## Documentation

See the [comprehensive User Guide](docs/GUIDE.md) for:

- Setup tutorials for all 6 providers
- Agent mode walkthrough with examples
- Context management for token efficiency
- 130 smart prompts reference
- Shell and file integration workflows
- Troubleshooting and FAQ
- Tips and tricks for power users

---

## Integration with Granit

Nerve integrates with [Granit](https://github.com/Artaeon/granit), an open-source knowledge management system:

- **Current:** Ingest Granit vaults as knowledge bases (`/kb add ~/my-vault`)
- **Planned:** Launch from Granit's command palette, vault-aware AI context, HTTP/MCP API

---

## Contributing

Contributions welcome. Please open an issue first to discuss changes.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test  # 450 tests across 38 source files
```

The test suite covers all modules: AI provider abstractions, agent tool parsing,
context management, prompt loading, configuration serialization, file reading with
line ranges, workspace detection, scaffold templates, knowledge base ingestion,
clipboard management, keybind parsing, and UI rendering logic.

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">

Built by [Artaeon](https://github.com/Artaeon) -- [raphael.lugmayr@stoicera.com](mailto:raphael.lugmayr@stoicera.com)

</div>
