# Nerve

**Raw AI power in your terminal.**

Nerve is a blazing-fast, terminal-native AI assistant built in Rust. It brings multi-model AI, smart prompts, knowledge bases, and a beautiful TUI to your command line -- no browser, no Electron, no subscriptions required.

---

## Features

- **Multi-Provider AI** -- Claude (via Claude Code), OpenAI, OpenRouter, Ollama (local), any OpenAI-compatible endpoint
- **Smart Prompts** -- 60+ built-in prompt templates across 6 categories (Writing, Coding, Translation, Analysis, Creative, Productivity)
- **Custom Prompts** -- Create your own reusable prompts with `{{input}}` variables
- **Nerve Bar** (Ctrl+K) -- Spotlight-style fuzzy command palette
- **Streaming Responses** -- Tokens render as they arrive
- **Syntax-Highlighted Code** -- Code blocks rendered with full syntax highlighting
- **Markdown Rendering** -- Bold, italic, headers, lists in the terminal
- **Knowledge Base (RAG)** -- Ingest documents, search by relevance, auto-inject context
- **Conversation History** -- Browse, search, and reload past conversations
- **Clipboard Manager** -- Full clipboard history with fuzzy search
- **Web Scraping** -- Fetch URL content as AI context (`/url`)
- **Automations** -- Multi-step AI pipelines (Code Review, Research, Content Optimization)
- **Slash Commands** -- `/url`, `/model`, `/kb`, `/auto`, `/clear`, `/help`
- **Vim Keybindings** -- Navigate with j/k, insert with i, command with /
- **Pipe-Friendly** -- `echo "text" | nerve --stdin` and `nerve -n "prompt"`
- **Daemon Mode** -- Background process with socket IPC
- **TOML Configuration** -- Human-editable config at `~/.config/nerve/`
- **Tiny Binary** -- Stripped, LTO-optimized, instant startup

---

## Installation

### From Source

```bash
git clone https://github.com/yourusername/nerve.git
cd nerve
cargo build --release
sudo cp target/release/nerve /usr/local/bin/
```

### Requirements

- Rust 2024 edition (1.85+)
- One of:
  - Claude Code installed (free with Claude subscription)
  - OpenAI API key
  - OpenRouter API key
  - Ollama running locally

---

## Quick Start

```bash
# Interactive TUI (default: Claude via Claude Code)
nerve

# With a specific provider
nerve --provider openai
nerve --provider ollama --model llama3

# One-shot mode
nerve -n "explain quantum computing in simple terms"

# Pipe mode
cat error.log | nerve --stdin -n "explain this error"
echo "translate to French" | nerve --stdin

# Direct prompt (non-interactive)
nerve "what is the capital of France"

# List available models
nerve --list-models
```

---

## Configuration

Nerve creates a config file at `~/.config/nerve/config.toml` on first run:

```toml
default_model = "sonnet"
default_provider = "claude_code"

[theme]
user_color = "#89b4fa"
assistant_color = "#a6e3a1"
border_color = "#585b70"
accent_color = "#cba6f7"

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

**Claude Code (default, no API key needed):**
Just install Claude Code -- Nerve uses your existing subscription. Make sure `claude` is in your PATH.

**OpenAI:**
```bash
export OPENAI_API_KEY="sk-..."
nerve --provider openai
```

**OpenRouter:**
```bash
export OPENROUTER_API_KEY="sk-or-..."
nerve --provider openrouter
```

**Ollama (local):**
```bash
ollama serve
nerve --provider ollama --model llama3
```

**Custom OpenAI-compatible provider:**
Add to your `config.toml`:
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
| `Ctrl+K` or `/` | Open Nerve Bar (command palette) |
| `Ctrl+N` | New conversation |
| `Ctrl+L` | Clear conversation |
| `Ctrl+C` | Quit |
| `Esc` | Close overlay / cancel |

### Navigation

| Key | Action |
|-----|--------|
| `Tab` | Switch conversations |
| `j` / `Down` | Scroll down / next item |
| `k` / `Up` | Scroll up / previous item |

### Overlays

| Key | Action |
|-----|--------|
| `Ctrl+P` | Prompt picker |
| `Ctrl+M` | Model selector |
| `Ctrl+H` | Help screen |
| `Ctrl+B` | Clipboard manager |
| `Ctrl+O` | History browser |

### Editing

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `Esc` | Return to normal mode |
| `Enter` | Send message |
| `Ctrl+V` | Paste from clipboard |

### Clipboard

| Key | Action |
|-----|--------|
| `Ctrl+Y` | Copy last AI response |
| `Ctrl+B` | Open clipboard manager |

---

## Slash Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/clear` | Clear current conversation |
| `/new` | Start new conversation |
| `/model <name>` | Switch AI model |
| `/models` | List available models |
| `/url <url> [question]` | Scrape URL and ask about it |
| `/kb add <dir>` | Add directory to knowledge base |
| `/kb list` | List knowledge bases |
| `/kb search <query>` | Search knowledge base |
| `/kb status` | Show KB statistics |
| `/auto list` | List automations |
| `/auto run <name>` | Run an automation |
| `/auto info <name>` | Show automation details |

---

## Smart Prompts

Nerve includes 60+ built-in prompts. Access them via the Nerve Bar (`Ctrl+K`) or the Prompt Picker (`Ctrl+P`).

### Categories

- **Writing** -- Summarize, Expand, Rewrite, Fix Grammar, Improve Clarity, and more
- **Coding** -- Explain Code, Fix Bug, Refactor, Add Comments, Write Tests, and more
- **Translation** -- Translate to English, Spanish, French, German, Japanese, Arabic, and more
- **Analysis** -- Sentiment, Key Points, SWOT, Fact Check, Root Cause, and more
- **Creative** -- Brainstorm, Story, Metaphor, Names, Poetry, Slogan, and more
- **Productivity** -- Action Items, Meeting Summary, Decision Matrix, Report, and more

### Custom Prompts

Custom prompts can be added as TOML files in `~/.config/nerve/prompts/`:

```toml
name = "My Custom Prompt"
description = "Does something useful"
template = "Please do the following with this input:\n\n{{input}}"
category = "Custom"
tags = ["custom", "utility"]
```

---

## Knowledge Base

Build a personal knowledge repository for RAG-enhanced AI responses:

```
# Add documents from a directory
/kb add ~/Documents/project-docs

# Search your knowledge base
/kb search "authentication flow"

# View statistics
/kb status

# List all knowledge bases
/kb list
```

Nerve automatically searches your knowledge base and injects relevant context into AI queries. Supported file types include plain text, Markdown, and source code files.

---

## Automations

Multi-step AI pipelines that chain prompts together, passing the output of each step as input to the next.

```
/auto list                           # See available automations
/auto run "Code Review Pipeline"     # Run with current input
/auto info "Research Assistant"       # View automation steps
```

### Built-in Automations

| Automation | Steps | Description |
|------------|-------|-------------|
| **Code Review Pipeline** | Analyze, Fix, Generate | Analyze code for bugs, suggest fixes, generate corrected code |
| **Content Optimizer** | Analyze, Rewrite, Summarize | Analyze content for clarity, rewrite, create summary and headlines |
| **Research Assistant** | Questions, Analysis, Synthesis | Break down topic, analyze each question, synthesize findings |
| **Email Drafter** | Context, Draft | Analyze context for tone, draft professional email |
| **Translate & Localize** | Translate, Localize | Translate text, review for cultural nuances |

Custom automations can be saved as TOML files in `~/.config/nerve/automations/`.

---

## Web Scraping

Fetch web content directly into your conversation:

```
/url https://example.com
/url https://example.com/docs what does this API do?
```

Nerve fetches the page, extracts readable content, and injects it as context for the AI. You can optionally include a question after the URL.

---

## Daemon Mode

Run Nerve as a background process that stays alive between invocations:

```bash
# Start the daemon (listens on /tmp/nerve.sock)
nerve --daemon

# Send a query to the running daemon
nerve --query "explain monads"

# Stop the daemon
nerve --stop-daemon
```

The daemon uses Unix domain sockets for IPC, keeping state alive between invocations without the overhead of restarting the process each time.

---

## Architecture

```
src/
├── main.rs                # CLI parsing, event loop, key handling
├── app.rs                 # Application state machine
├── config.rs              # TOML configuration (load/save/defaults)
├── daemon.rs              # Background daemon (Unix socket IPC)
├── automation.rs          # Multi-step AI pipelines
├── history.rs             # Conversation persistence
├── clipboard.rs           # System clipboard integration
├── clipboard_manager.rs   # Clipboard history with fuzzy search
├── keybinds.rs            # Keybind string parser
├── ai/
│   ├── mod.rs             # Module exports
│   ├── provider.rs        # AiProvider trait + message types
│   ├── claude_code.rs     # Claude Code CLI integration
│   └── openai.rs          # OpenAI-compatible API (OpenAI/Ollama/OpenRouter)
├── ui/
│   ├── mod.rs             # Layout and rendering dispatch
│   ├── chat.rs            # Syntax-highlighted chat view
│   ├── command_bar.rs     # Nerve Bar (fuzzy command palette)
│   ├── prompt_picker.rs   # SmartPrompt browser
│   ├── history_browser.rs # Conversation history viewer
│   ├── clipboard_manager.rs # Clipboard history overlay
│   └── help.rs            # Keybinding reference overlay
├── prompts/
│   ├── mod.rs             # Prompt loading and category listing
│   ├── builtin.rs         # 60+ built-in prompt templates
│   └── custom.rs          # User-defined prompt loading
├── knowledge/
│   ├── mod.rs             # Module exports
│   ├── store.rs           # Knowledge base storage
│   └── ingest.rs          # Document ingestion and chunking
└── scraper/
    ├── mod.rs             # Module exports
    └── web.rs             # URL fetching and content extraction
```

---

## Integration with Granit

Nerve is designed to integrate with [Granit](https://github.com/yourusername/granit), a knowledge management system. Planned integration includes:

- Vault-aware AI context (Nerve reads your Granit vault for RAG)
- Command palette integration (launch Nerve from Granit)
- Shared knowledge base between tools

---

## License

MIT

## Author

Artaeon
