# Nerve User Guide

A comprehensive guide to using Nerve as your terminal AI assistant and coding agent.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Providers](#providers)
3. [Chat Mode](#chat-mode)
4. [Agent Mode](#agent-mode)
5. [Smart Prompts](#smart-prompts)
6. [File Context](#file-context)
7. [Shell Integration](#shell-integration)
8. [Knowledge Base](#knowledge-base)
9. [Project Scaffolding](#project-scaffolding)
10. [Context Management](#context-management)
11. [Keyboard Reference](#keyboard-reference)
12. [Command Reference](#command-reference)
13. [Configuration](#configuration)
14. [Tips & Tricks](#tips--tricks)

---

## Getting Started

### Installation

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

Nerve compiles to a single ~7 MB binary with no runtime dependencies beyond the
AI provider you choose.

### First Launch

```bash
nerve
```

On first launch, Nerve:
1. Creates `~/.config/nerve/config.toml` with sensible defaults
2. Detects your project type if you run it inside a project directory
3. Opens the TUI with the welcome screen showing quick-start keybindings

### Provider Setup

Nerve needs at least one AI provider. The default is **Claude Code** (no API
key needed if you have a Claude Code subscription).

**Quick setup for each provider:**

| Provider | Setup |
|----------|-------|
| Claude Code | Install `claude` CLI, run `nerve` |
| Ollama | Install Ollama, run `ollama serve`, then `nerve --provider ollama` |
| OpenAI | `export OPENAI_API_KEY="sk-..."`, then `nerve --provider openai` |
| OpenRouter | `export OPENROUTER_API_KEY="sk-or-..."`, then `nerve --provider openrouter` |
| GitHub Copilot | Install `gh` CLI with Copilot extension, then `nerve --provider copilot` |

### Non-Interactive and Pipe Modes

You do not have to use the full TUI. Nerve supports one-shot and pipe modes for
scripting and quick lookups:

```bash
# One-shot: get an answer and exit
nerve -n "explain the difference between TCP and UDP"

# Pipe mode: feed content via stdin
cat src/main.rs | nerve --stdin -n "review this code"
git diff | nerve --stdin -n "write a commit message for these changes"

# List models from the current provider
nerve --list-models
```

### Daemon Mode

For long-running workflows you can keep Nerve resident in the background:

```bash
# Start the daemon (listens on /tmp/nerve.sock)
nerve --daemon

# Send queries to the running daemon
nerve --query "explain monads"

# Stop the daemon
nerve --stop-daemon
```

---

## Providers

### Claude Code (Default)

Uses your Claude Code subscription directly. No API key needed.

- **Models:** `opus` (1 M context), `sonnet` (200 K), `haiku` (200 K)
- **Switch models:** `/model opus` or `Ctrl+M`
- **Code mode:** `/code on` gives Claude full file and terminal access
- **Why use it:** Largest context windows, excellent coding ability, zero extra
  cost beyond your subscription.

### OpenAI

Standard OpenAI API. Requires an API key.

- **Models:** `gpt-4o` (128 K context), `gpt-4o-mini` (128 K)
- **Max response tokens:** 4096 (configurable)
- Nerve auto-compacts context to stay within limits and save tokens.

```bash
export OPENAI_API_KEY="sk-..."
nerve --provider openai --model gpt-4o
```

### OpenRouter

Access 100+ models through a single API.

- Any model listed on OpenRouter works.
- Set the model with: `/model anthropic/claude-3.5-sonnet`
- Conservative context management for cost control.

```bash
export OPENROUTER_API_KEY="sk-or-..."
nerve --provider openrouter
```

### Ollama (Local)

Run models locally with zero cost and full privacy.

- **Models:** whatever you have pulled (`llama3`, `mistral`, etc.)
- No API key, no internet required.
- Smaller context windows -- Nerve auto-compacts to fit.

```bash
ollama serve
nerve --provider ollama --model llama3
```

### GitHub Copilot

Uses GitHub Copilot through the `gh` CLI.

- **Requires:** `gh` CLI with the Copilot extension installed and authenticated.
- **Best for:** quick code suggestions and shell commands.
- **Limited context** compared to full providers (approximately 8 K tokens).
- Aliases: `copilot` or `gh`.

```bash
# Verify Copilot is available
gh copilot --help

# Launch Nerve with Copilot
nerve --provider copilot
```

### Custom OpenAI-Compatible Providers

Any endpoint that speaks the OpenAI chat-completions protocol can be added in
`config.toml`:

```toml
[[providers.custom]]
name = "my-provider"
api_key = "sk-..."
base_url = "https://api.example.com/v1"
```

Then use it: `nerve --provider my-provider`.

### Switching Providers at Runtime

```
# In the TUI:
Ctrl+T                # Provider selector overlay
/provider ollama      # Switch via command
/providers            # List all with status

# From the CLI:
nerve --provider openai --model gpt-4o
```

---

## Chat Mode

Chat mode is the default. Type messages, get AI responses with real-time
streaming and syntax highlighting.

### Basic Usage

1. Press `i` to enter insert mode (or just start typing -- defaults to insert).
2. Type your message.
3. Press `Enter` to send.
4. The AI response streams in token-by-token with syntax highlighting.

### Multi-line Input

- `Shift+Enter` or `Alt+Enter` inserts a newline.
- The input area grows dynamically (up to 40 % of the screen).

### Message Actions

| Key | Action |
|-----|--------|
| `Esc` (while streaming) | Stop generation |
| `Ctrl+R` | Regenerate last response |
| `Ctrl+E` | Edit last message |
| `Ctrl+Y` | Copy last AI response |
| `x` (Normal mode) | Delete last exchange |

### Conversations

Nerve supports multiple parallel conversations:

| Key / Command | Action |
|---------------|--------|
| `Ctrl+N` | New conversation |
| `Tab` | Next conversation |
| `Shift+Tab` | Previous conversation |
| `/rename <title>` | Rename the current conversation |
| `/clear` | Clear the current conversation |
| `/delete` | Delete the current conversation |
| `/export` | Export conversation as Markdown |

### Search

Press `Ctrl+F` to open the in-conversation search overlay. Matching messages
are highlighted and you can jump between results.

---

## Agent Mode

Nerve's most powerful feature. The AI becomes a coding agent that can read and
write files, run shell commands, and iterate until the task is done.

### Enabling Agent Mode

```
/agent on
```

### How It Works

1. You describe a task: *"Add error handling to the auth module"*
2. The AI plans its approach.
3. It uses tools to read files, understand code, and make changes.
4. It verifies by reading the changed files or running tests.
5. It repeats until the task is complete (max 10 iterations per turn).

### Available Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_file` | Create or overwrite files |
| `edit_file` | Find-and-replace in files |
| `run_command` | Execute shell commands |
| `list_files` | List directory contents |
| `search_code` | Grep across the codebase |
| `create_directory` | Create directories |

### Agent Mode Tips

- Start with `/agent on` and a clear, scoped task description.
- Use `/cd <dir>` to navigate to the right directory first.
- Press `Esc` at any time to stop the agent mid-loop.
- Use `/context` to inspect what the agent currently knows.
- The agent auto-compacts context to stay within the provider's token limit.

### Agent vs Code Mode

| Feature | Agent Mode (`/agent on`) | Code Mode (`/code on`) |
|---------|--------------------------|------------------------|
| Provider | Any (OpenAI, Ollama, Copilot, etc.) | Claude Code only |
| How tools work | Prompt-based (AI outputs tool calls) | Claude Code's native tools |
| Token usage | Uses your tokens | Uses Claude subscription |
| Best for | Any provider, token-efficient | Heavy coding with Claude |

---

## Smart Prompts

130 built-in prompts across 22 categories, instantly accessible via the Nerve
Bar or the Prompt Picker.

### Using Prompts

1. Press `Ctrl+K` to open the Nerve Bar.
2. Type to fuzzy-search (searches names, descriptions, *and* templates).
3. Use `Tab` / `Shift+Tab` to filter by category.
4. Press `Enter` to load the prompt.
5. The template appears in your input -- edit or send directly.

Alternatively, press `Ctrl+P` for the full-screen Prompt Picker with a
left-panel category browser and right-panel prompt list.

### Categories

| Category | Example Prompts |
|----------|-----------------|
| Writing | Summarize, Expand, Rewrite, Fix Grammar, Improve Clarity, Proofread |
| Coding | Explain Code, Fix Bug, Refactor, Add Comments, Write Tests |
| Engineering | Code Review, Architecture Review, Performance Analysis, Tech Debt |
| Analysis | Sentiment, Key Points, SWOT, Fact Check, Root Cause, Compare |
| Creative | Brainstorm, Story, Metaphor, Names, Poetry, Slogan |
| Productivity | Action Items, Meeting Summary, Decision Matrix, Report |
| Design | UI Review, Accessibility Audit, Animation Spec |
| Git | Commit Message, PR Description, Changelog |
| Rust | Rust Code Review, Ownership Analysis, Idiomatic Rust |
| DevOps | Dockerfile Review, CI/CD Pipeline, Infrastructure |
| Best Practices | PR Review Checklist, API Design, Error Handling |
| Translation | English, Spanish, French, German, Japanese, Arabic |
| Business | Business Plan, Market Analysis, Competitive Analysis |
| Communication | Presentation, Technical Writing, Documentation |
| Data | Data Analysis, SQL Review, Schema Design |
| Learning | Explain Like I'm 5, Tutorial, Study Guide |
| Legal | Contract Review, Privacy Policy, Terms of Service |
| Marketing | Ad Copy, SEO, Social Media |
| Personal | Resume, Cover Letter, Career Advice |
| Product | PRD, User Stories, Feature Spec |

### Creating Custom Prompts

Create `.toml` files in `~/.config/nerve/prompts/`:

```toml
name = "My Custom Prompt"
description = "What it does"
template = "Detailed instructions for the AI.\n\n{{input}}"
category = "Custom"
tags = ["my", "custom"]
```

The `{{input}}` placeholder is replaced by whatever you type after selecting the
prompt. Custom prompts appear alongside built-in ones in the Nerve Bar.

---

## File Context

Include files in your conversation so the AI can see your code.

### Single File

```
/file src/main.rs              # Full file
/file src/main.rs:10-50        # Lines 10-50 only
```

### Multiple Files

```
/files src/lib.rs src/app.rs   # Load several files at once
```

### Inline @file Syntax

You can reference files inline in your message:

```
"review @src/main.rs for bugs"
```

### Web Context

Fetch a URL and inject its content as AI context:

```
/url https://docs.rs/ratatui What are the main widgets?
```

---

## Shell Integration

Run commands without leaving Nerve. Output is displayed in the chat and can be
added as AI context automatically.

```
/run cargo test          # Run and show output
/test                    # Auto-detect and run project tests
/build                   # Auto-detect and build
/diff                    # Git diff as AI context
/diff --staged           # Staged changes only
/git status              # Quick git operations
/git log 20              # Recent commits
/pipe cargo clippy       # Run command, add output as AI context
```

### Practical Examples

**Run tests, then ask about failures:**
```
/test
"Why did test_auth fail?"
```

**Review staged changes and draft a commit message:**
```
/diff --staged
"Write a commit message for these changes"
```

**Pipe linter output for analysis:**
```
/pipe cargo clippy 2>&1
"Fix these clippy warnings"
```

---

## Knowledge Base

Build a personal knowledge repository that Nerve searches automatically when you
ask questions.

```
/kb add ~/docs/api-specs       # Ingest documents (recursively)
/kb status                     # Show stats (document count, chunk count)
/kb search "authentication"    # Search manually
/kb list                       # List knowledge bases
/kb clear                      # Clear knowledge base
```

When a knowledge base exists, Nerve automatically performs a fuzzy search
against it and injects the most relevant chunks into your AI queries. This gives
the model access to your private documentation, notes, or specifications without
manually pasting them.

---

## Project Scaffolding

### From Built-in Templates

```
/template list                  # Show 8 built-in templates
/template rust-cli myapp        # Create Rust CLI project
/template python-api backend    # Create FastAPI project
```

Available templates:

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

### AI-Generated Projects

Enable agent or code mode and describe what you want:

```
/agent on
/scaffold a REST API in Go with JWT auth, PostgreSQL, and Docker
```

The AI will create the full project structure, write source files, and generate
configuration.

---

## Context Management

Nerve automatically manages context to stay within each provider's token limits.

### How It Works

- Token usage is estimated at approximately 4 characters per token.
- Provider limits: Claude (200 K), OpenAI (60 K), OpenRouter (30 K), Ollama (8 K), Copilot (8 K).
- When approaching the limit, older messages are summarized.
- Tool results from agent mode are compacted aggressively.
- System messages (workspace context, file context) are preserved.

### Manual Controls

```
/tokens    # Show token usage breakdown
/compact   # Manually compact the conversation
/context   # Show what the AI currently sees
/summary   # Conversation statistics
```

### Token Tracking

The status bar always shows estimated token usage. Watch it on paid providers to
stay within budget.

---

## Keyboard Reference

### Global (Any Mode)

| Key | Action |
|-----|--------|
| `Ctrl+C` / `Ctrl+D` | Quit |
| `Ctrl+H` | Toggle help overlay |
| `Ctrl+K` | Open Nerve Bar (command palette) |
| `Ctrl+T` | Switch provider |
| `Ctrl+M` | Switch model |
| `Ctrl+N` | New conversation |
| `Ctrl+P` | Prompt picker (full-screen) |
| `Ctrl+B` | Clipboard manager |
| `Ctrl+O` | History browser |
| `Ctrl+F` | Search in conversation |
| `Ctrl+R` | Regenerate last response |
| `Ctrl+E` | Edit last message |
| `Ctrl+Y` | Copy last AI response |
| `Ctrl+L` | Clear conversation |
| `Esc` (while streaming) | Stop generation |

### Normal Mode (Vim-style Navigation)

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `/` | Open Nerve Bar |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `x` | Delete last exchange |
| `q` | Quit |
| `Tab` | Next conversation |
| `Shift+Tab` | Previous conversation |

### Insert Mode (Typing)

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` / `Alt+Enter` | Insert newline |
| `Esc` | Return to normal mode |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+W` | Delete word backward |
| `Backspace` | Delete character |
| `Left` / `Right` | Move cursor |
| `Tab` | Indent (in multi-line) |

### Nerve Bar

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Use selected prompt |
| `Tab` | Next category filter |
| `Shift+Tab` | Previous category filter |
| `Up` / `Down` | Navigate results |
| Type | Fuzzy search |

### Prompt Picker

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Use selected prompt |
| `Tab` | Toggle focus between category list and prompt list |
| `j` / `Down` | Move down in focused list |
| `k` / `Up` | Move up in focused list |
| Type | Filter prompts |

### Clipboard Manager

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Paste selected entry |
| `d` | Delete selected entry |
| `Up` / `Down` | Navigate |
| Type | Fuzzy search |

### History Browser

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Load selected conversation |
| `d` | Delete selected entry |
| `j` / `k` or `Up` / `Down` | Navigate |
| Type | Search |

---

## Command Reference

### Chat

| Command | Description |
|---------|-------------|
| `/help` | Show all available commands |
| `/clear` | Clear current conversation |
| `/new` | Start new conversation |
| `/delete` | Delete current conversation |
| `/delete all` | Delete all conversations |
| `/rename <title>` | Rename current conversation |
| `/export` | Export conversation to Markdown |
| `/copy` | Copy last AI response to clipboard |
| `/copy all` | Copy entire conversation |
| `/copy last` | Copy last message (any role) |
| `/system <prompt>` | Set system prompt for conversation |
| `/system clear` | Remove system prompt |

### AI Provider

| Command | Description |
|---------|-------------|
| `/provider <name>` | Switch provider (`claude_code`, `ollama`, `openai`, `openrouter`, `copilot`) |
| `/providers` | List available providers with status |
| `/model <name>` | Switch model |
| `/models` | List available models |
| `/code on\|off` | Toggle code mode (Claude Code only) |
| `/cwd <dir>` | Set working directory for code mode |
| `/agent on\|off` | Toggle agent mode (any provider) |

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
| `/build` | Auto-detect and build project |
| `/git [subcommand]` | Quick git operations (`status`, `log`, `diff`, `branch`) |

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
| `/tokens` | Show token usage breakdown |
| `/compact` | Manually compact conversation |
| `/context` | Show what the AI currently sees |
| `/summary` | Conversation statistics |

---

## Configuration

Configuration lives at `~/.config/nerve/config.toml` and is auto-generated on
first run.

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
api_key = ""                          # or set OPENAI_API_KEY env var
base_url = "https://api.openai.com/v1"
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true

[providers.openrouter]
api_key = ""                          # or set OPENROUTER_API_KEY env var
base_url = "https://openrouter.ai/api/v1"
enabled = false

# GitHub Copilot needs no config -- just ensure `gh` CLI with Copilot
# extension is installed and authenticated.

# Custom OpenAI-compatible provider example:
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

### Theme Customization

All colors are specified as hex codes. The four theme colors control:

| Key | Controls |
|-----|----------|
| `user_color` | User message headers and text accents |
| `assistant_color` | AI response headers |
| `border_color` | Panel borders and dividers |
| `accent_color` | Status bar highlights, selections, matched search terms |

### Keybind Customization

Override keybindings in the `[keybinds]` section. Values use the format
`ctrl+<key>`, `shift+<key>`, `alt+<key>`, or plain keys like `f1`.

---

## Tips & Tricks

### Token Efficiency

- Use `/compact` before complex agent tasks to free up context space.
- Prefer `/file path:10-50` over `/file path` for large files -- only load what
  you need.
- Use the right model for the job: `haiku` for quick tasks, `opus` for complex
  multi-file reasoning.
- Check `/tokens` regularly on paid providers to stay within budget.
- On Ollama or Copilot (small context windows), keep conversations short and
  compact often.

### Effective Agent Usage

- Be specific: *"Add input validation to the login handler in src/auth.rs"* is
  much better than *"improve the code"*.
- Start with `/file` to give the agent context, then describe the task.
- Use `/test` after agent changes to verify correctness.
- Chain commands: `/diff` then *"write a commit message for these changes"*.

### Workflow Examples

**Code Review:**
```
/file src/main.rs
"Review this code for bugs, security issues, and performance"
```

**Bug Fix:**
```
/test
"The test_auth test is failing. Fix the bug."
/agent on
```

**New Feature:**
```
/agent on
"Add a /weather command that shows weather for a given city using the wttr.in API"
```

**Documentation:**
```
/files src/lib.rs src/api.rs
"Write API documentation for these modules"
```

**Git Workflow:**
```
/diff --staged
"Write a conventional-commits message for these changes"
/git log 5
"Summarize the recent changes for a changelog entry"
```

**Quick Shell Help (with Copilot):**
```
/provider copilot
"how do I find all files larger than 100MB in this repo"
```

### Prompt Chaining with Automations

Automations let you build multi-step AI pipelines. For example, the built-in
"Code Review Pipeline" will:

1. Analyze code for bugs and issues.
2. Suggest fixes for each finding.
3. Generate corrected code.

Run it with `/auto run code-review-pipeline` after loading a file with `/file`.

### Using Nerve in Scripts

```bash
# Generate a commit message from staged changes
git diff --staged | nerve --stdin -n "Write a conventional-commits message"

# Summarize a log file
tail -100 /var/log/app.log | nerve --stdin -n "What errors occurred?"

# Translate a file
cat README.md | nerve --stdin -n "Translate to Japanese"
```

---

## Architecture Overview

For contributors and curious users, here is the high-level project layout:

```
nerve/
├── src/
│   ├── main.rs               Entry point, event loop, slash commands
│   ├── app.rs                Application state machine
│   ├── config.rs             TOML configuration (load/save/defaults)
│   ├── daemon.rs             Background daemon (Unix socket IPC)
│   ├── ai/
│   │   ├── provider.rs       AiProvider trait + message types
│   │   ├── claude_code.rs    Claude Code CLI integration
│   │   ├── copilot.rs        GitHub Copilot CLI integration
│   │   └── openai.rs         OpenAI-compatible API client
│   ├── ui/                   TUI rendering (ratatui)
│   ├── prompts/              130 built-in + custom prompt templates
│   ├── knowledge/            RAG: ingestion, chunking, fuzzy search
│   └── scraper/              URL fetching and content extraction
├── docs/
│   └── GUIDE.md              This file
├── Cargo.toml
├── LICENSE
└── README.md
```

The `AiProvider` trait (`src/ai/provider.rs`) defines the interface that every
backend implements: `chat_stream`, `chat`, `list_models`, and `name`. Adding a
new provider means implementing this trait and wiring it into `create_provider`
in `main.rs`.

---

*Built by [Artaeon](https://github.com/Artaeon) -- [raphael.lugmayr@stoicera.com](mailto:raphael.lugmayr@stoicera.com)*
