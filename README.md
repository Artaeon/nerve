<div align="center">

# Nerve

**Open-source AI coding assistant for the terminal.**

[![License: MIT](https://img.shields.io/badge/License-MIT-2563EB.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-F74C00.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-820%20passing-22C55E.svg)](#quality)
[![Version](https://img.shields.io/badge/Version-0.1.0-7C3AED.svg)](CHANGELOG.md)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

Chat. Code. Ship. All from your terminal.

27K lines of Rust. 820 tests. 166 prompts. 9 agent tools. 6 providers. 7.3MB binary.

</div>

> **[Read the Manifesto](MANIFESTO.md)** -- Why we built Nerve and what makes it different.

---

## Installation

### Download (pre-built)

Grab the latest binary from [GitHub Releases](https://github.com/Artaeon/nerve/releases/latest):

| Platform | Download |
|----------|----------|
| Linux x86_64 | `nerve-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `nerve-macos-arm64.tar.gz` |
| macOS Intel | `nerve-macos-x86_64.tar.gz` |
| Windows x86_64 | `nerve-windows-x86_64.zip` |

```bash
# Example: Linux
curl -LO https://github.com/Artaeon/nerve/releases/latest/download/nerve-linux-x86_64.tar.gz
tar xzf nerve-linux-x86_64.tar.gz
mv nerve ~/.local/bin/
```

Each release includes SHA256 checksums for verification.

### Quick Install (from source)

```bash
# 1. Clone
git clone https://github.com/Artaeon/nerve.git
cd nerve

# 2. Build
cargo build --release

# 3. Install to your PATH
cp target/release/nerve ~/.local/bin/

# 4. Verify
nerve --help
```

Make sure `~/.local/bin` is in your PATH. If not, add to your shell rc:

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.local/bin:$PATH"
```

### Alternative: Symlink (easier updates)

```bash
ln -sf "$(pwd)/target/release/nerve" ~/.local/bin/nerve
# To update later: git pull && cargo build --release
```

### Requirements

- **Rust 1.85+** (edition 2024) -- install with [rustup](https://rustup.rs/)
- **One AI provider** (at least one):

| Provider | Setup | Cost |
|----------|-------|------|
| **Claude Code** | Install [Claude Code CLI](https://claude.ai/code) | Subscription |
| **Ollama** | `curl -fsSL https://ollama.ai/install.sh \| sh && ollama pull llama3` | Free (local) |
| **OpenAI** | `export OPENAI_API_KEY="sk-..."` | Pay per token |
| **OpenRouter** | `export OPENROUTER_API_KEY="sk-or-..."` | Pay per token |
| **GitHub Copilot** | Install [gh CLI](https://cli.github.com/) + Copilot extension | Subscription |

---

## Quick Start

```bash
# Launch the TUI (defaults to Claude Code)
nerve

# Use a different provider
nerve --provider ollama --model llama3

# One-shot question (prints answer and exits)
nerve -n "explain the difference between TCP and UDP"

# Pipe files for review
cat src/main.rs | nerve --stdin -n "review this code for bugs"

# Pipe git diff for commit messages
git diff | nerve --stdin -n "write a commit message"

# Resume your last session
nerve --continue
```

Once inside the TUI:

| Action | How |
|--------|-----|
| Send a message | Type + Enter |
| Open prompt library | Ctrl+K |
| Switch provider | Ctrl+T |
| Switch model | Ctrl+M |
| Enable coding agent | `/agent on` |
| Read a file into context | `/file src/main.rs` |
| Run tests | `/test` |
| See all commands | `/help` |

---

## Features

### AI Providers (6)

Switch instantly with **Ctrl+T** or `/provider <name>`:

- **Claude Code** -- uses your subscription, no API key needed
- **OpenAI** -- GPT-4o and GPT-4o-mini
- **OpenRouter** -- 100+ models through one API
- **Ollama** -- local models, free, works offline
- **GitHub Copilot** -- via gh CLI
- **Custom** -- any OpenAI-compatible endpoint

### Coding Agent (`/agent on`)

The AI gets 9 tools and works autonomously:

```
/agent on
/file src/auth.rs
"The login endpoint returns 500 for invalid credentials. Fix it and add tests."
```

The agent reads files, edits code, runs commands, and verifies -- with a git safety net (`/agent undo` to rollback).

### Smart Prompts (166 across 28 categories)

Press **Ctrl+K** to search. Categories include: Engineering, Rust, Python, TypeScript, Go, UI/UX, Testing, Security, API, Database, Cloud, DevOps, Business, and more. Each prompt is 5-15 lines of expert instructions.

### Developer Workflow

```bash
/file src/main.rs              # Add file as context
/file src/main.rs:10-50        # Specific lines
/test                          # Auto-detect and run tests
/build                         # Auto-detect and build
/diff                          # Git diff as context
/run cargo clippy              # Run any command
/map                           # Show project structure
```

### Productivity

- **Persistent sessions** -- auto-saves, resume with `nerve -c`
- **Conversation branching** -- `/branch save`, `/branch restore`
- **History browser** (Ctrl+O) -- search, sort, preview
- **Clipboard manager** (Ctrl+B) -- fuzzy search
- **In-chat search** (Ctrl+F)
- **Input history** -- Up/Down arrows cycle previous inputs
- **8 color themes** -- `/theme list`
- **Plugin system** -- scripts in `~/.config/nerve/plugins/`
- **Settings overlay** -- Ctrl+, for visual configuration

### Token Efficiency

- Provider-aware context limits (auto-compacts to save tokens)
- Usage tracking (`/usage`) and spending limits (`/limit set 5.00`)
- Cost estimates in the status bar for paid providers

---

## Keybindings

| Key | Action |
|-----|--------|
| **Ctrl+K** or **/** | Prompt library (166 prompts) |
| **Ctrl+T** | Switch provider |
| **Ctrl+M** | Switch model |
| **Ctrl+N** | New conversation |
| **Ctrl+O** | History browser |
| **Ctrl+F** | Search in conversation |
| **Ctrl+B** | Clipboard manager |
| **Ctrl+,** | Settings |
| **Ctrl+R** | Regenerate response |
| **Ctrl+E** | Edit last message |
| **Esc** | Stop streaming |
| **Tab/Shift+Tab** | Next/prev conversation |
| **Up/Down** (insert) | Input history |
| **i / Esc** | Insert / normal mode |
| **j / k** | Scroll (normal mode) |
| **1-9** | Copy message by number |

---

## All Commands

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/provider <name>` | Switch AI provider |
| `/model <name>` | Switch model |
| `/agent on\|off` | Toggle coding agent |
| `/agent undo` | Rollback agent changes |
| `/agent diff` | Show what agent changed |
| `/file <path>` | Read file as context |
| `/run <cmd>` | Execute command |
| `/test` | Run project tests |
| `/build` | Build project |
| `/diff` | Git diff as context |
| `/git <subcmd>` | Git operations |
| `/template <name>` | Create from template |
| `/scaffold <desc>` | AI-generated project |
| `/kb add <dir>` | Add to knowledge base |
| `/theme <name>` | Switch color theme |
| `/usage` | Show token usage and cost |
| `/limit set <$>` | Set spending limit |
| `/export` | Export as markdown |
| `/branch save` | Save conversation branch |
| `/session save` | Save session |
| `/map` | Show project file tree |
| `/alias <n> <cmd>` | Create command alias |
| `/status` | System overview |

See `/help` in the TUI for the complete list of 45+ commands.

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
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openrouter]
api_key = ""   # or set OPENROUTER_API_KEY env var
enabled = false
```

---

## Project Templates

```
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

- **[User Guide](docs/GUIDE.md)** -- comprehensive 2,200-line guide
- **[Changelog](CHANGELOG.md)** -- release notes

---

## Quality

| Check | Status |
|-------|--------|
| Tests | 820 passing |
| Clippy | 0 warnings (strict mode) |
| Format | 0 diffs |
| Unsafe code | 0 |
| Binary | 7.3 MB (LTO + stripped) |

---

## Contributing

Contributions welcome. Please open an issue first to discuss changes.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test          # 820 tests
cargo clippy        # lint
cargo fmt --check   # format check
```

---

## License

MIT -- see [LICENSE](LICENSE).

---

<div align="center">

Built by [Artaeon](https://github.com/Artaeon)

</div>
