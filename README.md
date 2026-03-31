<div align="center">

# Nerve

**Raw AI power in your terminal.**

A blazing-fast, terminal-native AI assistant built in Rust.
Multi-model, multi-provider, with smart prompts, knowledge bases, and a beautiful TUI.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

```
    _   __
   / | / /__  ______   _____
  /  |/ / _ \/ ___/ | / / _ \
 / /|  /  __/ /   | |/ /  __/
/_/ |_/\___/_/    |___/\___/
```

</div>

---

## Why Nerve?

- **No API keys required** -- Uses your Claude Code subscription natively. Just install Claude Code and go.
- **Blazing fast** -- Rust + ratatui, LTO-stripped binary, instant startup.
- **Multi-provider** -- Claude Code, OpenAI, Ollama (local), OpenRouter, any OpenAI-compatible endpoint. Switch on the fly with Ctrl+T.
- **Terminal-native** -- No browser, no Electron. Designed for developers who live in the terminal.
- **Smart, not bloated** -- 60+ built-in prompts, knowledge base RAG, automations. All in a single binary.
- **Open source** -- MIT licensed. Free forever.

---

## Features

### AI Providers

Nerve supports multiple AI backends and lets you switch between them at runtime with Ctrl+T:

| Provider | Type | Auth |
|----------|------|------|
| **Claude Code** (default) | CLI integration | Uses your Claude Code subscription -- no API key needed |
| **OpenAI** | API | `OPENAI_API_KEY` environment variable or config |
| **OpenRouter** | API | `OPENROUTER_API_KEY` environment variable or config |
| **Ollama** | Local | No key needed -- runs on `localhost:11434` |
| **Custom** | OpenAI-compatible | Any base URL + API key via `config.toml` |

### Nerve Bar (Ctrl+K)

A Spotlight-style fuzzy command palette that gives you instant access to every command, prompt, and action in Nerve. Type to filter, arrow keys to navigate, Enter to execute. Also accessible with `/` in normal mode.

### Smart Prompts

60+ built-in prompt templates organized across 6 categories:

- **Writing** -- Summarize, Expand, Rewrite, Fix Grammar, Improve Clarity, and more
- **Coding** -- Explain Code, Fix Bug, Refactor, Add Comments, Write Tests, and more
- **Translation** -- Translate to English, Spanish, French, German, Japanese, Arabic, and more
- **Analysis** -- Sentiment, Key Points, SWOT, Fact Check, Root Cause, and more
- **Creative** -- Brainstorm, Story, Metaphor, Names, Poetry, Slogan, and more
- **Productivity** -- Action Items, Meeting Summary, Decision Matrix, Report, and more

Browse prompts with the Prompt Picker (Ctrl+P) or search them through the Nerve Bar. Create your own reusable prompts as TOML files in `~/.config/nerve/prompts/` using `{{input}}` template variables.

### Streaming Chat

Responses stream token-by-token as they arrive from the provider. Code blocks are rendered with full syntax highlighting via syntect. Markdown formatting -- bold, italic, headers, lists -- renders directly in the terminal.

### Knowledge Base (RAG)

Build a personal knowledge repository that enhances every AI response:

- Ingest entire directories of documents (`/kb add ~/project-docs`)
- Documents are chunked and indexed for efficient retrieval
- Fuzzy search across your knowledge base (`/kb search "auth flow"`)
- Relevant context is automatically injected into AI queries
- Supports plain text, Markdown, and source code files

### Conversation History

Every conversation is persisted to disk automatically. Browse past conversations with Ctrl+O, grouped by date with preview. Switch between active conversations with Tab. Reload and continue any previous session.

### Automations

Multi-step AI pipelines that chain prompts together, passing the output of each step as input to the next:

| Automation | Steps | Description |
|------------|-------|-------------|
| **Code Review Pipeline** | Analyze, Fix, Generate | Analyze code for bugs, suggest fixes, generate corrected code |
| **Content Optimizer** | Analyze, Rewrite, Summarize | Analyze content for clarity, rewrite, create summary and headlines |
| **Research Assistant** | Questions, Analysis, Synthesis | Break down topic, analyze each question, synthesize findings |
| **Email Drafter** | Context, Draft | Analyze context for tone, draft professional email |
| **Translate & Localize** | Translate, Localize | Translate text, review for cultural nuances |

Create custom automations as TOML files in `~/.config/nerve/automations/`.

### Clipboard Manager

Full clipboard history with fuzzy search, source tracking, and instant paste. Open it with Ctrl+B to browse everything you have copied during your session. Copy the last AI response with Ctrl+Y.

### Web Scraping

Fetch any URL directly into your conversation with the `/url` command. Nerve extracts readable content from the page and injects it as context for the AI. Optionally include a question after the URL to ask about the content immediately.

### Vim Keybindings

Nerve uses a modal editing model. Press `i` to enter insert mode and type messages. Press `Esc` to return to normal mode where `j`/`k` scroll the conversation, `/` opens the command palette, and `q` quits. Designed to feel natural for Vim users.

### Pipe & CLI Mode

Use Nerve in scripts and pipelines without the TUI:

```bash
# One-shot mode: get an answer and exit
nerve -n "explain the difference between TCP and UDP"

# Pipe mode: feed content via stdin
cat src/main.rs | nerve --stdin -n "review this code"
git diff | nerve --stdin -n "write a commit message for these changes"

# Direct prompt (non-interactive)
nerve "what is the capital of France"

# List available models
nerve --list-models
```

### Daemon Mode

Run Nerve as a persistent background process using Unix domain sockets for IPC:

```bash
# Start the daemon (listens on /tmp/nerve.sock)
nerve --daemon

# Send queries to the running daemon
nerve --query "explain monads"

# Stop the daemon
nerve --stop-daemon
```

The daemon keeps state alive between invocations, eliminating startup overhead for frequent use.

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
# Launch the TUI (defaults to Claude Code)
nerve

# Switch provider from CLI
nerve --provider ollama --model llama3
nerve --provider openai --model gpt-4o

# One-shot mode
nerve -n "explain the difference between TCP and UDP"

# Pipe mode
cat src/main.rs | nerve --stdin -n "review this code"
git diff | nerve --stdin -n "write a commit message for these changes"

# Web context (inside the TUI)
/url https://docs.rs/ratatui summarize the ratatui docs

# Knowledge base (inside the TUI)
/kb add ~/Documents/project-specs
/kb search "authentication flow"
```

---

## Configuration

Configuration lives at `~/.config/nerve/config.toml` and is auto-generated on first run.

```toml
# ───────────────────────────────────────────────────────────────────────
# Nerve - configuration file
# ───────────────────────────────────────────────────────────────────────
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
| `Ctrl+O` | History browser |
| `Ctrl+P` | Prompt picker |
| `Ctrl+B` | Clipboard manager |

### Navigation (Normal Mode)

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `j` / `k` | Scroll down / up |
| `q` | Quit |

### Editing (Insert Mode)

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Esc` | Normal mode |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+Y` | Copy last AI response |
| `Ctrl+L` | Clear conversation |

---

## Slash Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/clear` | Clear current conversation |
| `/new` | Start new conversation |
| `/model <name>` | Switch AI model |
| `/models` | List available models |
| `/provider <name>` | Switch AI provider |
| `/providers` | List available providers |
| `/url <url> [question]` | Scrape URL and ask about it |
| `/kb add <dir>` | Add directory to knowledge base |
| `/kb list` | List knowledge bases |
| `/kb search <query>` | Search knowledge base |
| `/kb status` | Show KB statistics |
| `/kb clear` | Clear knowledge base |
| `/auto list` | List automations |
| `/auto run <name>` | Run an automation |
| `/auto info <name>` | Show automation details |

---

## Custom Prompts

Create reusable prompt templates as TOML files in `~/.config/nerve/prompts/`:

```toml
name = "My Custom Prompt"
description = "Does something useful"
template = "Please do the following with this input:\n\n{{input}}"
category = "Custom"
tags = ["custom", "utility"]
```

Prompts appear automatically in the Prompt Picker (Ctrl+P) and Nerve Bar (Ctrl+K).

---

## Architecture

```
nerve/
├── src/
│   ├── main.rs                  # Entry point, event loop, key handling
│   ├── app.rs                   # Application state machine
│   ├── config.rs                # TOML configuration (load/save/defaults)
│   ├── daemon.rs                # Background daemon (Unix socket IPC)
│   ├── automation.rs            # Multi-step AI pipelines
│   ├── history.rs               # Conversation persistence
│   ├── clipboard.rs             # System clipboard integration
│   ├── clipboard_manager.rs     # Clipboard history with fuzzy search
│   ├── keybinds.rs              # Keybind string parser
│   ├── ai/
│   │   ├── mod.rs               # Module exports
│   │   ├── provider.rs          # AiProvider trait + message types
│   │   ├── claude_code.rs       # Claude Code CLI integration
│   │   └── openai.rs            # OpenAI-compatible API (OpenAI/Ollama/OpenRouter)
│   ├── ui/
│   │   ├── mod.rs               # Layout and rendering dispatch
│   │   ├── chat.rs              # Syntax-highlighted chat view
│   │   ├── command_bar.rs       # Nerve Bar (fuzzy command palette)
│   │   ├── prompt_picker.rs     # SmartPrompt browser
│   │   ├── history_browser.rs   # Conversation history viewer
│   │   ├── clipboard_manager.rs # Clipboard history overlay
│   │   └── help.rs              # Keybinding reference overlay
│   ├── prompts/
│   │   ├── mod.rs               # Prompt loading and category listing
│   │   ├── builtin.rs           # 60+ built-in prompt templates
│   │   └── custom.rs            # User-defined prompt loading
│   ├── knowledge/
│   │   ├── mod.rs               # Module exports
│   │   ├── store.rs             # Knowledge base storage
│   │   └── ingest.rs            # Document ingestion and chunking
│   └── scraper/
│       ├── mod.rs               # Module exports
│       └── web.rs               # URL fetching and content extraction
├── prompts/                     # Example prompt templates
├── assets/
│   └── banner.txt               # ASCII art banner
├── Cargo.toml
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

## Contributing

Contributions are welcome. Please open an issue first to discuss what you would like to change.

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build
cargo test
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">

Built by [Artaeon](https://github.com/Artaeon) -- [raphael.lugmayr@stoicera.com](mailto:raphael.lugmayr@stoicera.com)

</div>
