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

**37K lines of Rust | 1,361 tests | 166 prompts | 10 agent tools | 6 providers | 7.7 MB binary**

[Install](#install) | [Quick Start](#quick-start) | [Features](#features) | [Agent Mode](#agent-mode) | [Manifesto](MANIFESTO.md)

</div>

---

Nerve is a fast, open-source AI coding assistant built entirely in Rust. It runs in your terminal, connects to 6 AI providers, ships 166 expert prompts, and includes a full coding agent with 10 tools -- all in a single 7.7 MB binary with zero runtime dependencies.

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

### Agent Mode

Type `/agent on` and let Nerve work autonomously. The agent has 10 tools:

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

Every agent action is tracked. Roll back with `/agent undo`. Inspect changes with `/agent diff`.

```
/agent on
/file src/auth.rs
> "The login endpoint returns 500 for invalid credentials. Fix it and add tests."
```

The agent reads the code, writes a fix, runs the tests, and reports back.

### 166 Smart Prompts

Press `Ctrl+K` to open the prompt library. 28 categories of expert-level prompts:

Engineering, Rust, Python, TypeScript, Go, Testing, Security, API Design, Database, Cloud, DevOps, UI/UX, Business, and more.

Each prompt is 5-15 lines of carefully crafted instructions -- not vague suggestions, but real engineering guidance.

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
| Agent tools | 10 | Yes | Yes | Yes |
| Built-in prompts | 166 | No | No | No |
| Cost | Free + API | $20/mo | $20/mo | Free + API |
| Binary size | 7.7 MB | ~150 MB | ~500 MB | Python |
| Vim keybindings | Native | No | Plugin | No |

---

## Project Quality

| Metric | Value |
|--------|-------|
| Language | 100% Rust |
| Lines of code | 37,600+ |
| Tests | 1,361 passing |
| Clippy | 0 warnings |
| Unsafe code | 0 blocks |
| Binary (release) | 7.7 MB (LTO + stripped) |

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
- **[Manifesto](MANIFESTO.md)** -- why Nerve exists
- **[Changelog](CHANGELOG.md)** -- release history

---

## Contributing

Contributions welcome. Please open an issue first to discuss changes.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo test          # 1,361 tests
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
