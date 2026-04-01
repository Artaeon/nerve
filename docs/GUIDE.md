# Nerve User Guide

A comprehensive guide to using Nerve as your terminal AI assistant and coding agent.

---

## Table of Contents

1. [Getting Started](#getting-started)
   - [Installation](#installation)
   - [First Launch](#first-launch)
   - [Provider Setup](#provider-setup)
   - [Non-Interactive and Pipe Modes](#non-interactive-and-pipe-modes)
   - [Daemon Mode](#daemon-mode)
2. [Providers](#providers)
   - [Claude Code (Default)](#claude-code-default)
   - [OpenAI](#openai)
   - [OpenRouter](#openrouter)
   - [Ollama (Local)](#ollama-local)
   - [GitHub Copilot](#github-copilot)
   - [Custom OpenAI-Compatible Providers](#custom-openai-compatible-providers)
   - [Switching Providers at Runtime](#switching-providers-at-runtime)
   - [Provider Comparison Matrix](#provider-comparison-matrix)
3. [Chat Mode](#chat-mode)
   - [Basic Usage](#basic-usage)
   - [Multi-line Input](#multi-line-input)
   - [Message Actions](#message-actions)
   - [Conversations](#conversations)
   - [Search](#search)
   - [Exporting Conversations](#exporting-conversations)
4. [Agent Mode](#agent-mode)
   - [Enabling Agent Mode](#enabling-agent-mode)
   - [How It Works](#how-it-works)
   - [Available Tools](#available-tools)
   - [Agent Mode Walkthroughs](#agent-mode-walkthroughs)
   - [Agent Mode Tips](#agent-mode-tips)
   - [Agent vs Code Mode](#agent-vs-code-mode)
5. [Smart Prompts](#smart-prompts)
   - [Using Prompts](#using-prompts)
   - [Nerve Bar](#nerve-bar)
   - [Prompt Picker](#prompt-picker)
   - [Categories](#categories)
   - [Creating Custom Prompts](#creating-custom-prompts)
   - [Prompt Best Practices](#prompt-best-practices)
6. [File Context](#file-context)
   - [Single File](#single-file)
   - [Line Ranges](#line-ranges)
   - [Multiple Files](#multiple-files)
   - [Inline @file Syntax](#inline-file-syntax)
   - [Web Context](#web-context)
7. [Shell Integration](#shell-integration)
   - [Running Commands](#running-commands)
   - [Piping Output as Context](#piping-output-as-context)
   - [Auto-Detect Build and Test](#auto-detect-build-and-test)
   - [Git Integration](#git-integration)
   - [Practical Examples](#practical-examples)
8. [Knowledge Base](#knowledge-base)
   - [Ingesting Documents](#ingesting-documents)
   - [Searching the Knowledge Base](#searching-the-knowledge-base)
   - [How RAG Works in Nerve](#how-rag-works-in-nerve)
   - [Managing Knowledge Bases](#managing-knowledge-bases)
9. [Project Scaffolding](#project-scaffolding)
   - [Built-in Templates](#built-in-templates)
   - [AI-Generated Projects](#ai-generated-projects)
   - [Template Reference](#template-reference)
10. [Automations](#automations)
    - [Built-in Automations](#built-in-automations)
    - [Running Automations](#running-automations)
    - [Creating Custom Automations](#creating-custom-automations)
11. [Context Management](#context-management)
    - [How It Works](#how-context-management-works)
    - [Provider Token Limits](#provider-token-limits)
    - [Manual Controls](#manual-controls)
    - [Token Optimization Strategies](#token-optimization-strategies)
12. [Keyboard Reference](#keyboard-reference)
    - [Global](#global-any-mode)
    - [Normal Mode](#normal-mode-vim-style-navigation)
    - [Insert Mode](#insert-mode-typing)
    - [Overlays](#overlay-keybindings)
13. [Command Reference](#command-reference)
14. [Configuration](#configuration)
    - [Config File Location](#config-file-location)
    - [Full Configuration Reference](#full-configuration-reference)
    - [Theme Customization](#theme-customization)
    - [Keybind Customization](#keybind-customization)
    - [File Paths and Data Storage](#file-paths-and-data-storage)
15. [Workspace Awareness](#workspace-awareness)
16. [Tips and Tricks](#tips--tricks)
    - [Token Efficiency](#token-efficiency)
    - [Effective Agent Usage](#effective-agent-usage)
    - [Workflow Examples](#workflow-examples)
    - [Using Nerve in Scripts](#using-nerve-in-scripts)
    - [Power User Workflows](#power-user-workflows)
17. [Troubleshooting](#troubleshooting)
18. [FAQ](#faq)
19. [Migrating from Other AI Tools](#migrating-from-other-ai-tools)
20. [Performance Tuning](#performance-tuning)
21. [Architecture Overview](#architecture-overview)

---

## Getting Started

### Installation

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

Nerve compiles to a single ~7MB binary with no runtime dependencies beyond libc.
The release profile uses LTO (link-time optimization), maximum optimization, single
codegen unit, and symbol stripping to produce the smallest possible binary.

**Prerequisites:**

- Rust 2024 edition (1.85 or later). Install via [rustup](https://rustup.rs) if you do
  not have it.
- A C compiler (gcc or clang) for some dependency crates.
- At least one AI provider configured (see below).

**Verifying the build:**

```bash
# Check the binary size
ls -lh target/release/nerve

# Run the test suite (450 tests)
cargo test

# Verify it launches
./target/release/nerve --help
```

### First Launch

```bash
nerve
```

On first launch, Nerve:

1. Creates `~/.config/nerve/config.toml` with sensible defaults.
2. Creates `~/.config/nerve/prompts/` for custom prompt templates.
3. Creates `~/.local/share/nerve/history/` for conversation persistence.
4. Detects your project type if you run it inside a project directory (Rust, Node.js,
   Python, Go, and others).
5. Opens the TUI with the welcome screen showing quick-start keybindings.

The default provider is Claude Code. If you have the `claude` CLI installed and
authenticated, Nerve works immediately with no further configuration.

### Provider Setup

Nerve needs at least one AI provider. Here is the quickest path for each:

| Provider | Setup |
|----------|-------|
| Claude Code | Install `claude` CLI from [claude.ai/code](https://claude.ai/code), run `nerve` |
| Ollama | Install Ollama from [ollama.ai](https://ollama.ai), run `ollama serve`, then `nerve --provider ollama` |
| OpenAI | `export OPENAI_API_KEY="sk-..."`, then `nerve --provider openai` |
| OpenRouter | `export OPENROUTER_API_KEY="sk-or-..."`, then `nerve --provider openrouter` |
| GitHub Copilot | Install `gh` CLI with Copilot extension, then `nerve --provider copilot` |
| Custom | Add entry to `~/.config/nerve/config.toml` (see [Custom Providers](#custom-openai-compatible-providers)) |

### Non-Interactive and Pipe Modes

You do not have to use the full TUI. Nerve supports one-shot and pipe modes for
scripting, automation, and quick lookups:

```bash
# One-shot: get an answer and exit
nerve -n "explain the difference between TCP and UDP"

# Pipe mode: feed content via stdin
cat src/main.rs | nerve --stdin -n "review this code"
git diff | nerve --stdin -n "write a commit message for these changes"
cargo test 2>&1 | nerve --stdin -n "explain why these tests failed"

# List models from the current provider
nerve --list-models

# Specify provider and model on the command line
nerve --provider openai --model gpt-4o -n "what is a monad?"
```

**How to use pipe mode effectively:**

1. Pipe the content you want analyzed into Nerve via stdin.
2. Use the `-n` flag to provide a question or instruction.
3. Use `--stdin` to tell Nerve to read from stdin.
4. Nerve prints the AI response to stdout and exits.

This makes Nerve composable in shell pipelines and scripts.

Use `--no-splash` to skip both the startup and goodbye splash screens (useful in
scripts or when you want the fastest possible launch).

### Daemon Mode

For long-running workflows, keep Nerve resident in the background:

```bash
# Start the daemon (listens on /tmp/nerve.sock via Unix socket IPC)
nerve --daemon

# Send queries to the running daemon
nerve --query "explain monads"

# Stop the daemon
nerve --stop-daemon
```

The daemon maintains conversation state between queries, so follow-up questions
retain context from previous interactions. This is useful for scripting workflows
where you want to ask multiple related questions without re-launching Nerve each time.

---

## Providers

### Claude Code (Default)

Uses your Claude Code subscription directly. No API key needed -- Nerve calls the
`claude` CLI and uses your existing authentication.

- **Models:** `opus` (1M context), `sonnet` (200K), `haiku` (200K)
- **Switch models:** `/model opus` or `Ctrl+M`
- **Code mode:** `/code on` gives Claude full file and terminal access through
  Claude Code's native tool system
- **Why use it:** Largest context windows, excellent coding ability, zero extra
  cost beyond your subscription, no API key management.

**How to verify Claude Code is available:**

```bash
# Check that the claude CLI is installed and in PATH
which claude

# Test it
claude --version
```

**Selecting models:**

```
/model opus     # Best quality, 1M token context
/model sonnet   # Fast and capable, 200K context (default)
/model haiku    # Fastest, 200K context, best for quick tasks
```

### OpenAI

Standard OpenAI API. Requires an API key from [platform.openai.com](https://platform.openai.com).

- **Models:** `gpt-4o` (128K context), `gpt-4o-mini` (128K)
- **Max response tokens:** 4096 (configurable)
- Nerve auto-compacts context to stay within limits and save tokens.

**How to set up:**

```bash
# Option 1: Environment variable
export OPENAI_API_KEY="sk-..."
nerve --provider openai --model gpt-4o

# Option 2: Config file
# Edit ~/.config/nerve/config.toml:
# [providers.openai]
# api_key = "sk-..."
# enabled = true
```

**Cost awareness:** OpenAI charges per token. Use `/tokens` to monitor usage and
`/compact` to reduce context size before expensive operations. Nerve's auto-compaction
helps, but manual compaction before long agent sessions can save significant cost.

### OpenRouter

Access 100+ models from many providers through a single API at
[openrouter.ai](https://openrouter.ai).

- Any model listed on OpenRouter works with Nerve.
- Set the model using its full OpenRouter identifier: `/model anthropic/claude-3.5-sonnet`
- Conservative context management for cost control.

**How to set up:**

```bash
# Option 1: Environment variable
export OPENROUTER_API_KEY="sk-or-..."
nerve --provider openrouter

# Option 2: Config file
# Edit ~/.config/nerve/config.toml:
# [providers.openrouter]
# api_key = "sk-or-..."
# enabled = true
```

**How to use with specific models:**

```
/model anthropic/claude-3.5-sonnet
/model openai/gpt-4o
/model meta-llama/llama-3-70b-instruct
/model mistralai/mistral-large-latest
```

**Cost tip:** OpenRouter shows per-token pricing on their website. Use `/models` to
see available models, then check OpenRouter's pricing page to find the best
price/performance ratio for your task.

### Ollama (Local)

Run models locally with zero cost and full privacy. Nothing leaves your machine.

- **Models:** whatever you have pulled (`llama3`, `mistral`, `codellama`, `phi`, etc.)
- No API key, no internet required after model download.
- Smaller context windows -- Nerve auto-compacts to fit.

**How to set up:**

```bash
# 1. Install Ollama (https://ollama.ai)
# 2. Start the server
ollama serve

# 3. Pull a model (one-time download)
ollama pull llama3

# 4. Launch Nerve
nerve --provider ollama --model llama3
```

**Which model to choose:**

| Model | Size | Best For |
|-------|------|----------|
| `llama3` (8B) | ~4.7GB | General chat, good quality/speed balance |
| `llama3:70b` | ~40GB | Best quality, needs beefy GPU |
| `mistral` (7B) | ~4.1GB | Fast, good at instruction following |
| `codellama` (7B) | ~3.8GB | Code generation and review |
| `phi` (2.7B) | ~1.6GB | Fastest, lightweight tasks |

**Important:** Ollama has smaller context windows (typically 4K-8K tokens by default).
Nerve handles this automatically, but you may need to `/compact` more frequently.
You can increase Ollama's context window by setting `num_ctx` in your Ollama modelfile.

### GitHub Copilot

Uses GitHub Copilot through the `gh` CLI. Best for quick code suggestions and shell
command help.

- **Requires:** `gh` CLI with the Copilot extension installed and authenticated.
- **Limited context** (approximately 8K tokens) compared to full providers.
- Aliases: `copilot` or `gh`.

**How to set up:**

```bash
# 1. Install the GitHub CLI
# See: https://cli.github.com

# 2. Authenticate
gh auth login

# 3. Install the Copilot extension
gh extension install github/gh-copilot

# 4. Verify
gh copilot --help

# 5. Launch Nerve with Copilot
nerve --provider copilot
```

**When to use Copilot:** Copilot is best for quick, focused questions about code
or shell commands. For longer conversations, multi-file analysis, or agent mode,
use a provider with a larger context window.

### Custom OpenAI-Compatible Providers

Any endpoint that implements the OpenAI chat-completions API can be added. This
includes LM Studio, vLLM, text-generation-webui, LocalAI, and many others.

**How to set up:**

Add a section to `~/.config/nerve/config.toml`:

```toml
[[providers.custom]]
name = "my-provider"
api_key = "sk-..."
base_url = "https://api.example.com/v1"
```

Then use it:

```bash
nerve --provider my-provider
```

**Example: LM Studio:**

```toml
[[providers.custom]]
name = "lm-studio"
api_key = "not-needed"
base_url = "http://localhost:1234/v1"
```

**Example: Self-hosted vLLM:**

```toml
[[providers.custom]]
name = "vllm"
api_key = "token-if-needed"
base_url = "http://gpu-server:8000/v1"
```

You can add multiple custom providers. Each appears in the provider picker (Ctrl+T)
with the name you specify.

### Switching Providers at Runtime

You never need to restart Nerve to change providers:

```
# Visual picker
Ctrl+T                          # Opens the provider selector overlay

# Slash command
/provider ollama                # Switch to Ollama
/provider openai                # Switch to OpenAI
/providers                      # List all providers with status

# Model switching
Ctrl+M                          # Opens the model selector overlay
/model gpt-4o                   # Switch to specific model
/models                         # List available models
```

Your conversation history is preserved when switching providers. The context is
automatically re-sent to the new provider on the next message.

### Provider Comparison Matrix

| Feature | Claude Code | OpenAI | OpenRouter | Ollama | Copilot | Custom |
|---------|-------------|--------|------------|--------|---------|--------|
| Auth | Subscription | API key | API key | None | gh CLI | API key |
| Max context | 1M (opus) | 128K | Varies | 8K-128K | ~8K | Varies |
| Streaming | Yes | Yes | Yes | Yes | No | Yes |
| Agent mode | Yes | Yes | Yes | Yes | Yes | Yes |
| Code mode | Yes | No | No | No | No | No |
| Cost | Subscription | Per-token | Per-token | Free | Subscription | Varies |
| Privacy | Cloud | Cloud | Cloud | Local | Cloud | Your choice |
| Offline | No | No | No | Yes | No | Depends |

---

## Chat Mode

Chat mode is the default. Type messages, get AI responses with real-time streaming
and syntax highlighting.

### Basic Usage

1. Press `i` to enter insert mode (or just start typing -- defaults to insert on
   first launch).
2. Type your message.
3. Press `Enter` to send.
4. The AI response streams in token-by-token with full syntax highlighting.

The chat view renders Markdown with syntax-highlighted code blocks (powered by
syntect), bold/italic formatting, and proper list indentation.

### Multi-line Input

- `Shift+Enter` or `Alt+Enter` inserts a newline in the input area.
- The input area grows dynamically, up to 40% of the screen height.
- This is useful for pasting code snippets or writing detailed prompts.

### Message Actions

| Key | Action |
|-----|--------|
| `Esc` (while streaming) | Stop the AI mid-generation |
| `Ctrl+R` | Regenerate the last AI response (re-sends your last message) |
| `Ctrl+E` | Edit your last message (loads it back into the input area) |
| `Ctrl+Y` | Copy the last AI response to system clipboard |
| `x` (Normal mode) | Delete the last user/AI exchange pair |

**How to stop and regenerate:**

If the AI is going in the wrong direction, press `Esc` to stop it mid-stream. Then
either edit your prompt with `Ctrl+E` or regenerate with `Ctrl+R`. Regenerating
sends the same message again, which may produce a different response (AI responses
are non-deterministic by default).

### Conversations

Nerve supports multiple parallel conversations, each with its own history:

| Key / Command | Action |
|---------------|--------|
| `Ctrl+N` | Create a new conversation |
| `Tab` | Switch to the next conversation |
| `Shift+Tab` | Switch to the previous conversation |
| `/rename <title>` | Give the current conversation a descriptive name |
| `/clear` | Clear all messages in the current conversation |
| `/delete` | Delete the current conversation entirely |
| `/delete all` | Delete all conversations (asks for confirmation) |

Conversations are automatically saved to `~/.local/share/nerve/history/` as JSON files.
They persist across Nerve restarts.

**How to manage conversations effectively:**

- Use `/rename` to give conversations meaningful names -- this makes them easy to
  find later in the history browser (Ctrl+O).
- Start a new conversation (`Ctrl+N`) when switching topics. Mixing unrelated topics
  wastes context tokens.
- Use `Tab`/`Shift+Tab` to quickly flip between ongoing conversations.

### Search

Press `Ctrl+F` to open the in-conversation search overlay:

1. Type your search query.
2. Matching messages are highlighted.
3. Use `Up`/`Down` or `n`/`N` to jump between matches.
4. Press `Esc` to close search.

Search looks through both user messages and AI responses in the current conversation.

### Exporting Conversations

Use `/export` to save the current conversation as a clean Markdown file:

```
/export
```

The file is saved to the current working directory with a filename based on the
conversation title and timestamp. Exported files include all messages with proper
Markdown formatting, code blocks, and role labels.

---

## Agent Mode

Nerve's most powerful feature. The AI becomes a coding agent that can read and write
files, run shell commands, search your codebase, and iterate until the task is done.

### Enabling Agent Mode

```
/agent on
```

To turn it off:

```
/agent off
```

### How It Works

Agent mode adds a set of 7 tools to the AI's system prompt. The AI can call these
tools by outputting structured `<tool_call>` tags in its response. Nerve intercepts
these calls, executes them, and feeds the results back to the AI.

The execution loop:

1. **You describe a task:** "Add error handling to the auth module"
2. **AI plans its approach:** The model thinks about what files to read and what changes
   to make.
3. **AI uses tools:** It reads files to understand the code, then makes edits or creates
   new files.
4. **AI verifies:** It re-reads changed files or runs tests to confirm correctness.
5. **AI iterates:** If something is wrong, it fixes the issue. Maximum 10 iterations
   per turn to prevent runaway loops.

**Important:** Agent mode works with ANY provider, not just Claude Code. The tool
format is prompt-based (not native function calling), so even Ollama and Copilot
can use it. Quality varies by model -- larger models handle complex multi-step tasks
better.

### Available Tools

| Tool | Description | Example |
|------|-------------|---------|
| `read_file` | Read the contents of any file | Reads src/main.rs to understand the code |
| `write_file` | Create a new file or overwrite an existing one | Creates a new test file |
| `edit_file` | Find-and-replace text within a file | Changes a function implementation |
| `run_command` | Execute a shell command and return output | Runs `cargo test` to verify changes |
| `list_files` | List the contents of a directory | Explores project structure |
| `search_code` | Search for patterns across the codebase (grep) | Finds all usages of a function |
| `create_directory` | Create one or more directories | Sets up a new module directory |

### Agent Mode Walkthroughs

**Walkthrough 1: Fix a Bug**

```
/agent on
/file src/auth.rs
"The login function returns 500 instead of 401 for invalid credentials. Fix it and
make sure all tests pass."
```

What the agent does:

1. Reads src/auth.rs (it was already loaded via `/file`, but the agent may re-read it).
2. Identifies the error mapping logic and finds the bug.
3. Uses `edit_file` to change the error status from 500 to 401.
4. Uses `read_file` to verify the edit was applied correctly.
5. Uses `run_command` to execute `cargo test` and confirm tests pass.
6. Reports the fix.

**Walkthrough 2: Add a New Feature**

```
/agent on
"Add a /health endpoint to the API that returns JSON with:
- status: 'ok'
- uptime: seconds since server start
- version: read from Cargo.toml"
```

What the agent does:

1. Uses `list_files` to explore the project structure.
2. Uses `read_file` on the main router file to understand how routes are defined.
3. Uses `read_file` on Cargo.toml to find the version string.
4. Uses `edit_file` to add the health route to the router.
5. Uses `write_file` to create a new handler file (or edits an existing one).
6. Uses `run_command` to compile and run tests.

**Walkthrough 3: Refactor Across Multiple Files**

```
/agent on
"Rename the User struct to Account everywhere in the codebase, including all
references, imports, and test assertions."
```

What the agent does:

1. Uses `search_code` to find every file that references "User".
2. Uses `read_file` on each relevant file.
3. Uses `edit_file` on each file to rename User to Account.
4. Uses `run_command` with `cargo build` to check compilation.
5. Fixes any remaining issues found during compilation.

**Walkthrough 4: Generate Tests**

```
/agent on
/file src/auth.rs
"Write comprehensive unit tests for every public function in this file. Put them
in a #[cfg(test)] module at the bottom of the file."
```

### Agent Mode Tips

- **Be specific.** "Add input validation to the login handler in src/auth.rs" is much
  better than "improve the code".
- **Pre-load context.** Use `/file` to load relevant files before giving the agent its
  task. This saves iterations the agent would spend discovering files.
- **Use `/cd` first.** Navigate to the right project directory so the agent's relative
  paths resolve correctly.
- **Press Esc to stop.** If the agent is going in a bad direction, stop it, provide
  correction, and let it continue.
- **Check with `/context`.** Use `/context` to see what the agent currently knows.
  If context is getting large, use `/compact` before the next agent task.
- **Start small.** For large changes, break the work into smaller tasks and handle
  them one at a time.
- **Run tests after.** Always verify agent changes. Use `/test` or `/run cargo test`
  after an agent session.

### Agent vs Code Mode

| Feature | Agent Mode (`/agent on`) | Code Mode (`/code on`) |
|---------|--------------------------|------------------------|
| Provider | Any (OpenAI, Ollama, Copilot, etc.) | Claude Code only |
| How tools work | AI outputs `<tool_call>` tags, Nerve executes | Claude Code's native tool system |
| Token usage | Uses your provider's tokens | Uses Claude Code subscription |
| Tool quality | Depends on model capability | Best-in-class (Claude's native tools) |
| Max iterations | 10 per turn | Controlled by Claude Code |
| Best for | Any provider, token-efficient tasks | Heavy coding with Claude Code |

**When to use which:**

- Use **Agent Mode** when you want to use a non-Claude provider (OpenAI, Ollama, etc.)
  or when you want explicit control over the tool loop.
- Use **Code Mode** when you have a Claude Code subscription and want the most capable
  coding experience. Code mode gives Claude full access to your file system and terminal
  through its native tool system, which generally produces better results for complex
  coding tasks.

---

## Smart Prompts

130 built-in prompts across 22 categories, instantly accessible via the Nerve Bar
or the Prompt Picker. Each prompt is a detailed 5-15 line system prompt with specific
instructions, not a simple one-liner.

### Using Prompts

**Method 1: Nerve Bar (Ctrl+K)**

1. Press `Ctrl+K` to open the Nerve Bar.
2. Type to fuzzy-search (searches names, descriptions, and template content).
3. Use `Tab` / `Shift+Tab` to filter by category.
4. The preview panel shows the full prompt template as you navigate.
5. Press `Enter` to load the selected prompt.
6. The template appears in your input -- edit placeholders or send directly.

**Method 2: Prompt Picker (Ctrl+P)**

1. Press `Ctrl+P` to open the full-screen Prompt Picker.
2. The left panel shows categories; the right panel shows prompts.
3. Use `Tab` to toggle focus between categories and prompts.
4. Use `j`/`k` or arrow keys to navigate.
5. Press `Enter` to use the selected prompt.

**Method 3: Nerve Bar via /**

In normal mode, press `/` to open the Nerve Bar. This is the same as Ctrl+K.

### Nerve Bar

The Nerve Bar is a fuzzy command palette inspired by VS Code's command palette and
Obsidian's command palette. It provides:

- **Fuzzy search** across prompt names, descriptions, and template content
- **Category tabs** along the top for quick filtering
- **Preview panel** showing the full template of the currently highlighted prompt
- **Match count** showing how many prompts match your query

The Nerve Bar also handles slash commands. If you type `/` followed by a command name,
it will execute the command instead of loading a prompt.

### Prompt Picker

The Prompt Picker is a full-screen overlay with a two-panel layout:

- **Left panel:** Category list with counts (e.g., "Engineering (20)")
- **Right panel:** Prompts in the selected category with descriptions

This is better for browsing when you are not sure which prompt you want. The Nerve
Bar is better when you know roughly what you are looking for.

### Categories

| Category | Count | Example Prompts |
|----------|-------|-----------------|
| Engineering | 20 | Full Code Review, Architect Solution, Debug Detective, Performance Analysis, Tech Debt Audit, Security Review, API Design Review |
| Writing | 15 | Summarize, Expand, Rewrite, Fix Grammar, Improve Clarity, Proofread, Technical Blog Post, Professional Email, Long-Form Article |
| Coding | 12 | Explain Code, Fix Bug, Refactor, Add Comments, Write Tests, Optimize, Convert Language, Code Documentation |
| Design | 10 | UX Review, Design System, Accessibility Audit, Animation Spec, Wireframe Description, Color Palette, Typography |
| Best Practices | 10 | PR Review Checklist, SOLID Principles, Clean Code, API Design, Error Handling Patterns |
| Git | 8 | Commit Message, PR Description, Changelog, Git Workflow, Branch Strategy, Release Notes |
| Analysis | 7 | Sentiment Analysis, Key Points Extraction, SWOT, Fact Check, Root Cause Analysis, Compare and Contrast |
| Creative | 7 | Brainstorm, Story, Metaphor, Names, Poetry, Slogan, Dialogue |
| Productivity | 5 | Action Items, Meeting Summary, Decision Matrix, Report, Status Update |
| Translation | 5 | English, Spanish, French, German, Japanese |
| Learning | 4 | Explain Like I'm 5, Tutorial, Study Guide, Flashcards |
| Communication | 3 | Presentation, Technical Writing, Documentation |
| Business | 3 | Business Plan, Business Case, Meeting Notes to Actions |
| Marketing | 3 | Landing Page Copy, Content Calendar, Ad Copy |
| Data | 3 | SQL Query Builder, Data Pipeline Design, Data Analysis |
| Product | 3 | PRD, User Stories, User Research Questions |
| DevOps | 2 | Incident Postmortem, Infrastructure as Code |
| Rust | 2 | Rust Code Review, Rust From Scratch |
| Legal | 2 | Contract Review, Privacy Policy |
| Personal | 2 | Resume, Cover Letter |

### Creating Custom Prompts

Create `.toml` files in `~/.config/nerve/prompts/`:

```toml
name = "My Custom Prompt"
description = "What it does in one sentence"
template = """You are an expert at X.

Analyze the following input and provide:
1. A summary of the key points
2. Potential issues or risks
3. Specific, actionable recommendations

Be thorough and cite specific lines or sections when relevant.

Input:
{{input}}"""
category = "Custom"
tags = ["custom", "review"]
```

**Template variables:**

- `{{input}}` -- Replaced by whatever the user types after selecting the prompt. This
  is the primary way to inject user content into the template.

**Tips for writing good prompts:**

- Be specific about the role you want the AI to assume.
- List exactly what output you expect (numbered items, sections, format).
- Include constraints ("be concise", "under 200 words", "use bullet points").
- Tag prompts with relevant keywords so they are easy to find in fuzzy search.

Custom prompts appear alongside built-in ones in the Nerve Bar and Prompt Picker,
sorted alphabetically within their category.

### Prompt Best Practices

1. **Use the right prompt for the job.** The Engineering category has 20 specialized
   prompts for different aspects of code review. "Full Code Review" gives a broad
   review; "Security Review" focuses on vulnerabilities; "Performance Analysis"
   focuses on bottlenecks.

2. **Layer prompts with file context.** Load a file first (`/file src/auth.rs`),
   then select a prompt. The AI sees both the prompt instructions and the file content.

3. **Edit before sending.** After selecting a prompt, the template appears in your
   input. You can edit it to add specific instructions or remove irrelevant sections
   before sending.

4. **Create project-specific prompts.** If your project has specific coding standards,
   create a custom prompt that encodes those standards. For example, a "Project Style
   Check" prompt that references your team's conventions.

---

## File Context

Include files in your conversation so the AI can see your code. File content is
injected as a system message with the file path and content clearly labeled.

### Single File

```
/file src/main.rs
```

This reads the entire file and adds it to the conversation context. The AI sees the
full content with the file path as a header.

### Line Ranges

```
/file src/main.rs:10-50
```

This reads only lines 10 through 50. Use line ranges for large files to save context
tokens and focus the AI's attention on the relevant section.

**When to use line ranges:**

- The file is more than a few hundred lines.
- You know exactly which function or section is relevant.
- You are on a provider with a small context window (Ollama, Copilot).

### Multiple Files

```
/files src/lib.rs src/app.rs src/config.rs
```

Loads all three files in a single command. Useful for cross-file analysis, such as
understanding how modules interact or reviewing an entire feature across files.

### Inline @file Syntax

Reference files directly in your message text:

```
"Review @src/main.rs for bugs and compare with @src/main_old.rs"
```

Nerve detects the `@path` syntax, reads the referenced files, and includes their
content alongside your message. This is a convenient shorthand that avoids a
separate `/file` command.

### Web Context

Fetch a URL and inject its content as AI context:

```
/url https://docs.rs/ratatui What are the main widgets?
```

Nerve fetches the URL, extracts the text content (stripping HTML), and adds it as
context. The optional question after the URL is sent as your message.

**Use cases for web context:**

- Reading API documentation to help the AI generate correct API calls.
- Fetching a GitHub issue or PR description for analysis.
- Loading a Stack Overflow answer to discuss or adapt.

---

## Shell Integration

Run commands without leaving Nerve. Output is displayed in the chat and can be
automatically added as AI context.

### Running Commands

```
/run cargo test
```

Executes the command in your shell and displays the output in the chat. The output
is shown but not automatically added as AI context (use `/pipe` for that).

### Piping Output as Context

```
/pipe cargo clippy 2>&1
```

Runs the command and adds the output as AI context. The AI can then analyze or
respond to the output. This is the preferred way to get AI help with compiler errors,
linter warnings, or test failures.

### Auto-Detect Build and Test

```
/test     # Detects and runs the project's test command
/build    # Detects and runs the project's build command
```

Nerve uses workspace detection to determine the correct command:

| Project Type | `/test` Command | `/build` Command |
|-------------|----------------|-----------------|
| Rust (Cargo.toml) | `cargo test` | `cargo build` |
| Node.js (package.json) | `npm test` | `npm run build` |
| Python (pyproject.toml) | `pytest` | `python -m build` |
| Go (go.mod) | `go test ./...` | `go build ./...` |

### Git Integration

```
/diff                    # Show unstaged git diff as AI context
/diff --staged           # Show staged changes only
/diff HEAD~3             # Diff against 3 commits ago
/git status              # Quick git status
/git log 20              # Last 20 commits
/git branch              # List branches
```

All git output is displayed in the chat and can be used as context for AI analysis.

### Practical Examples

**Run tests, then ask about failures:**

```
/test
"Why did test_auth fail? Show me exactly what assertion failed and suggest a fix."
```

**Review staged changes and draft a commit message:**

```
/diff --staged
"Write a conventional-commits style commit message for these changes. Include
a short body explaining the motivation."
```

**Pipe linter output for analysis:**

```
/pipe cargo clippy 2>&1
"Fix all of these clippy warnings. Show me the corrected code for each."
```

**Analyze recent git history:**

```
/git log 10
"Summarize the last 10 commits for a changelog entry. Group by feature area."
```

**Debug a build failure:**

```
/pipe cargo build 2>&1
"Explain these compilation errors and how to fix each one."
```

---

## Knowledge Base

Build a personal knowledge repository that Nerve searches automatically when you
ask questions. This implements RAG (Retrieval-Augmented Generation): your documents
are chunked, indexed, and searched on every query, with the most relevant chunks
injected into the AI context.

### Ingesting Documents

```
/kb add ~/docs/api-specs
```

This recursively scans the directory, reads all text-based files (Markdown, plain
text, TOML, YAML, JSON, source code), splits them into chunks, and stores them in
the knowledge base.

**Supported file types:** Markdown (.md), plain text (.txt), TOML, YAML, JSON, and
common source code files (.rs, .py, .js, .ts, .go, .java, etc.).

### Searching the Knowledge Base

```
/kb search "authentication"
```

Performs a fuzzy search across all knowledge base chunks and displays the most
relevant results. You can also just ask a question -- if a knowledge base is active,
Nerve automatically searches it and injects relevant context into every AI query.

### How RAG Works in Nerve

1. **Ingestion:** Documents are split into chunks at paragraph boundaries.
2. **Storage:** Chunks are stored locally with metadata (source file, position).
3. **Search:** On every user message, Nerve performs a fuzzy search across chunks.
4. **Injection:** The top-matching chunks are prepended to the AI context.
5. **Transparency:** The AI sees the chunks as "[Knowledge Base Context]" in the
   system prompt.

### Managing Knowledge Bases

```
/kb status     # Show stats: document count, chunk count, total size
/kb list       # List all ingested sources
/kb clear      # Delete the knowledge base and all chunks
```

**Tips:**

- Ingest API specifications, project documentation, or design documents before
  asking the AI architecture or design questions.
- Ingest Granit vault directories for seamless knowledge management integration.
- Keep the knowledge base focused. Ingesting too many unrelated documents reduces
  search quality.
- Use `/kb status` to check the size. Large knowledge bases can add latency to
  queries due to search time.

---

## Project Scaffolding

### Built-in Templates

8 templates for instant project creation:

```
/template list                  # Show all templates with descriptions
/template rust-cli myapp        # Create a Rust CLI project in ./myapp
/template python-api backend    # Create a FastAPI project in ./backend
```

### AI-Generated Projects

For projects not covered by templates, use AI scaffolding:

```
/agent on
/scaffold a REST API in Go with JWT auth, PostgreSQL, and Docker
```

The AI generates the full project structure, writes source files, configuration,
Dockerfile, and README. This uses agent mode tools (write_file, create_directory)
to build the project.

### Template Reference

| Template | Language | What You Get |
|----------|----------|-------------|
| `rust-cli` | Rust | Cargo.toml, src/main.rs with clap argument parsing, src/lib.rs, comprehensive error handling with thiserror, unit tests, .gitignore |
| `rust-lib` | Rust | Library crate with public API surface, documentation comments, unit tests, integration test stubs, examples directory |
| `rust-web` | Rust | Axum web server with router, handler modules, middleware, JSON request/response types, health endpoint, basic tests |
| `node-api` | Node.js | Express + TypeScript, tsconfig.json, package.json with scripts, routes directory, middleware, error handling, basic tests |
| `node-react` | React | Vite + TypeScript + React, component structure, CSS modules, routing setup, basic tests with Vitest |
| `python-cli` | Python | pyproject.toml, src/ layout, argparse-based CLI, __main__.py entry point, tests directory |
| `python-api` | Python | FastAPI with async, Pydantic models, route modules, error handlers, test stubs with pytest |
| `go-api` | Go | go.mod, main.go, handler package, router with net/http, middleware, basic tests |

**After scaffolding:**

```
/template rust-web myapi
/cd myapi
/run cargo build             # Verify the template builds
/agent on
"Add user authentication with JWT tokens"
```

Templates are designed to compile/run out of the box, giving you a solid starting
point to build on with agent mode.

---

## Automations

Multi-step AI pipelines that chain prompts together. Each step's output becomes the
input for the next step.

### Built-in Automations

| Automation | Steps | Description |
|------------|-------|-------------|
| **Code Review Pipeline** | Analyze, Fix, Generate | Analyzes code for bugs, suggests fixes, generates corrected code |
| **Content Optimizer** | Analyze, Rewrite, Summarize | Analyzes content for clarity, rewrites it, creates summary and headlines |
| **Research Assistant** | Questions, Analysis, Synthesis | Breaks down a topic into research questions, analyzes each, synthesizes findings |
| **Email Drafter** | Context, Draft | Analyzes context for tone and audience, drafts a professional email |
| **Translate and Localize** | Translate, Localize | Translates text, then reviews for cultural nuances and localizes idioms |

### Running Automations

```
/auto list                  # List all available automations
/auto info code-review      # Show steps and descriptions
/auto run code-review       # Run the automation
```

**How to use with file context:**

```
/file src/auth.rs
/auto run code-review
```

The file content becomes the input to the first step of the automation. Each
subsequent step receives the output of the previous step.

### Creating Custom Automations

Create `.toml` files in `~/.config/nerve/automations/`:

```toml
name = "my-pipeline"
description = "Describe what this automation does"

[[steps]]
name = "Step 1: Analyze"
prompt = """Analyze the following input and identify the key issues.
List them in order of severity.

Input:
{{input}}"""

[[steps]]
name = "Step 2: Fix"
prompt = """Given the following analysis, provide concrete fixes for each issue.
Include code examples where applicable.

Analysis:
{{input}}"""

[[steps]]
name = "Step 3: Summary"
prompt = """Summarize the analysis and fixes into a brief report suitable for
a pull request description.

Details:
{{input}}"""
```

Each step's `{{input}}` placeholder is replaced by the output of the previous step
(or the user's initial input for the first step).

---

## Context Management

Nerve automatically manages context to stay within each provider's token limits.
This is critical for keeping conversations flowing without errors and for minimizing
costs on per-token providers.

### How Context Management Works

- Token usage is estimated at approximately 4 characters per token. This is a rough
  heuristic that works well across most models.
- When the estimated token count approaches the provider's limit, Nerve compacts
  older messages by summarizing them.
- Summarization preserves the key information while reducing token count.
- It cuts at sentence boundaries to avoid mid-sentence truncation.
- Tool results from agent mode are compacted more aggressively than regular messages,
  since the AI typically only needs the outcome, not the full raw output.
- System messages (workspace context, knowledge base context, file context) are
  preserved and not compacted.

### Provider Token Limits

| Provider | Default Limit | Notes |
|----------|--------------|-------|
| Claude Code (opus) | 200,000 | Can handle extremely long conversations |
| Claude Code (sonnet/haiku) | 200,000 | Same limit, faster response |
| OpenAI (gpt-4o) | 60,000 | Conservative to manage costs |
| OpenRouter | 30,000 | Conservative for cost control |
| Ollama | 8,000 | Limited by local model context window |
| GitHub Copilot | 8,000 | Limited by Copilot's API |

### Manual Controls

```
/tokens    # Show token usage breakdown by message type
/compact   # Manually compact the conversation (summarize old messages)
/context   # Show exactly what the AI currently sees (full context dump)
/summary   # Conversation statistics (message count, word count, token estimate)
```

### Token Optimization Strategies

1. **Use line ranges for large files.** `/file src/main.rs:10-50` instead of
   `/file src/main.rs` for a 2000-line file.

2. **Compact before agent sessions.** Agent mode generates many tool calls and results.
   Start with a clean context: `/compact` before `/agent on`.

3. **Choose the right model.** Use `haiku` or `gpt-4o-mini` for simple tasks. Save
   `opus` or `gpt-4o` for complex multi-file reasoning.

4. **Start new conversations for new topics.** Do not reuse a conversation about
   authentication to discuss database migrations. Start fresh with `Ctrl+N`.

5. **Monitor on paid providers.** Watch `/tokens` on OpenAI and OpenRouter. If token
   count is climbing, compact or start a new conversation.

6. **Use `/pipe` sparingly on verbose commands.** `cargo build 2>&1` can produce
   hundreds of lines. Consider `cargo build 2>&1 | head -50` if you only need the
   first errors.

---

## Keyboard Reference

### Global (Any Mode)

| Key | Action |
|-----|--------|
| `Ctrl+C` / `Ctrl+D` | Quit (conversation state is saved) |
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

### Overlay Keybindings

**Nerve Bar:**

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Use selected prompt / execute command |
| `Tab` | Next category filter |
| `Shift+Tab` | Previous category filter |
| `Up` / `Down` | Navigate results |
| Type | Fuzzy search |

**Prompt Picker:**

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Use selected prompt |
| `Tab` | Toggle focus between category list and prompt list |
| `j` / `Down` | Move down in focused list |
| `k` / `Up` | Move up in focused list |
| Type | Filter prompts |

**Clipboard Manager:**

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Paste selected entry into input |
| `d` | Delete selected entry |
| `Up` / `Down` | Navigate |
| Type | Fuzzy search |

**History Browser:**

| Key | Action |
|-----|--------|
| `Esc` | Close |
| `Enter` | Load selected conversation |
| `d` | Delete selected entry |
| `j` / `k` or `Up` / `Down` | Navigate |
| Type | Search conversations |

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

### Knowledge and Context

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

### Shell and Git

| Command | Description |
|---------|-------------|
| `/run <command>` | Run shell command and show output |
| `/pipe <command>` | Run command and add output as AI context |
| `/diff [args]` | Show git diff (adds as context) |
| `/test` | Auto-detect and run project tests |
| `/build` | Auto-detect and build project |
| `/git [subcommand]` | Quick git operations (`status`, `log`, `diff`, `branch`) |
| `/cd <dir>` | Change working directory |

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

### Workspace and Status

| Command | Description |
|---------|-------------|
| `/workspace` | Show detected project info |
| `/status` | Show system status (provider, model, tokens) |
| `/tokens` | Show token usage breakdown |
| `/compact` | Manually compact conversation |
| `/context` | Show what the AI currently sees |
| `/summary` | Conversation statistics |

---

## Configuration

### Config File Location

```
~/.config/nerve/config.toml    # Main configuration
~/.config/nerve/prompts/       # Custom prompt templates (.toml)
~/.config/nerve/automations/   # Custom automation pipelines (.toml)
~/.local/share/nerve/history/  # Conversation history (JSON)
```

The config file is auto-generated on first run with sensible defaults.

### Full Configuration Reference

```toml
# Nerve - configuration file

# Default model and provider used on startup
default_model = "sonnet"
default_provider = "claude_code"

# Theme colors (hex codes)
[theme]
user_color = "#89b4fa"         # User message headers and text accents
assistant_color = "#a6e3a1"    # AI response headers
border_color = "#585b70"       # Panel borders and dividers
accent_color = "#cba6f7"       # Status bar, selections, search matches

# --- Providers ---

[providers.claude_code]
enabled = true                 # No API key needed; uses claude CLI

[providers.openai]
api_key = ""                   # Or set OPENAI_API_KEY env var
base_url = "https://api.openai.com/v1"
enabled = false

[providers.ollama]
base_url = "http://localhost:11434/v1"
enabled = true                 # No API key needed; local only

[providers.openrouter]
api_key = ""                   # Or set OPENROUTER_API_KEY env var
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

# --- Keybindings ---

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

All colors are hex codes. The four theme colors control different parts of the UI:

| Key | Controls |
|-----|----------|
| `user_color` | User message headers and text accents |
| `assistant_color` | AI response headers |
| `border_color` | Panel borders and dividers |
| `accent_color` | Status bar highlights, selections, matched search terms |

**Example: Dark blue theme:**

```toml
[theme]
user_color = "#7aa2f7"
assistant_color = "#9ece6a"
border_color = "#3b4261"
accent_color = "#bb9af7"
```

**Example: Solarized-inspired:**

```toml
[theme]
user_color = "#268bd2"
assistant_color = "#859900"
border_color = "#586e75"
accent_color = "#d33682"
```

### Keybind Customization

Override keybindings in the `[keybinds]` section. Values use the format:

- `ctrl+<key>` -- Control + key
- `shift+<key>` -- Shift + key
- `alt+<key>` -- Alt + key
- `ctrl+shift+<key>` -- Control + Shift + key
- `f1` through `f12` -- Function keys
- Plain characters for normal-mode bindings

**Example: Remap command bar to Ctrl+Space:**

```toml
[keybinds]
command_bar = "ctrl+space"
```

### File Paths and Data Storage

| Path | Purpose | Format |
|------|---------|--------|
| `~/.config/nerve/config.toml` | Main configuration | TOML |
| `~/.config/nerve/prompts/*.toml` | Custom prompt templates | TOML |
| `~/.config/nerve/automations/*.toml` | Custom automation pipelines | TOML |
| `~/.local/share/nerve/history/*.json` | Conversation history | JSON |
| `/tmp/nerve.sock` | Daemon socket (when running) | Unix socket |

Nerve follows the XDG Base Directory Specification for config and data paths. If you
have custom `XDG_CONFIG_HOME` or `XDG_DATA_HOME` set, Nerve respects those.

---

## Workspace Awareness

Nerve auto-detects your project type when launched from a project directory. This
information is injected into the AI system prompt, giving the model useful context
about your project without you having to explain it.

**What Nerve detects:**

| File | Project Type | Extracted Info |
|------|-------------|----------------|
| `Cargo.toml` | Rust | Name, version, description, dependencies |
| `package.json` | Node.js | Name, version, description, scripts, dependencies |
| `pyproject.toml` | Python | Name, version, description, dependencies |
| `go.mod` | Go | Module name, Go version, dependencies |
| `pom.xml` | Java (Maven) | Group ID, artifact ID, version |
| `build.gradle` | Java (Gradle) | Project detection |
| `Gemfile` | Ruby | Project detection |
| `mix.exs` | Elixir | Project detection |
| `build.zig` | Zig | Project detection |
| `*.csproj` | C# | Project detection |
| `CMakeLists.txt` | C/C++ | Project detection |

**How to see what Nerve detected:**

```
/workspace
```

This shows the detected project type, name, description, and key metadata.

**Why this matters:** When the AI knows you are working on a Rust project with axum
and serde as dependencies, it can give more relevant advice without you having to
explain your tech stack every time.

---

## Tips and Tricks

### Token Efficiency

- Use `/compact` before complex agent tasks to free up context space.
- Prefer `/file path:10-50` over `/file path` for large files -- only load what
  you need.
- Use the right model for the job: `haiku` or `gpt-4o-mini` for quick tasks, `opus`
  or `gpt-4o` for complex multi-file reasoning.
- Check `/tokens` regularly on paid providers to stay within budget.
- On Ollama or Copilot (small context windows), keep conversations short and
  compact often.
- Start new conversations (`Ctrl+N`) when switching topics to avoid wasting tokens
  on irrelevant context.

### Effective Agent Usage

- **Be specific:** "Add input validation to the login handler in src/auth.rs" is
  much better than "improve the code".
- **Pre-load context:** Use `/file` to give the agent relevant files before describing
  the task. This saves tool-call iterations.
- **Use `/cd` first:** Navigate to the right directory so relative paths resolve correctly.
- **Run tests after:** Always verify agent changes with `/test` or `/run cargo test`.
- **Break up big tasks:** Instead of "rewrite the entire auth system", try
  "add password hashing to the registration handler" followed by "add JWT token
  generation to the login handler".
- **Chain agent + template:** Scaffold a project with `/template`, then use agent mode
  to flesh it out.

### Workflow Examples

**Code Review:**

```
/file src/main.rs
Ctrl+K -> "Full Code Review"
```

**Bug Fix with Agent:**

```
/test
"The test_auth test is failing. What is the assertion error?"
/agent on
"Fix the bug that causes test_auth to fail."
/test
```

**New Feature with Agent:**

```
/agent on
"Add a /weather command that shows weather for a given city using the wttr.in API.
Include error handling for network failures and invalid city names."
```

**Documentation Generation:**

```
/files src/lib.rs src/api.rs
"Write comprehensive API documentation for these modules. Include function
descriptions, parameter explanations, return types, and usage examples."
```

**Git Workflow:**

```
/diff --staged
"Write a conventional-commits style commit message for these changes."
/git log 5
"Summarize the recent changes for a changelog entry."
```

**Quick Shell Help (with Copilot):**

```
/provider copilot
"how do I find all files larger than 100MB and delete them"
```

**Translate Content:**

```
cat README.md | nerve --stdin -n "Translate to Japanese"
```

### Using Nerve in Scripts

```bash
# Generate a commit message from staged changes
git diff --staged | nerve --stdin -n "Write a conventional-commits message"

# Summarize a log file
tail -100 /var/log/app.log | nerve --stdin -n "What errors occurred?"

# Code review in CI
cat src/main.rs | nerve --stdin --provider openai -n "Review for security issues"

# Translate a file
cat README.md | nerve --stdin -n "Translate to Spanish" > README.es.md

# Generate documentation
cat src/lib.rs | nerve --stdin -n "Write rustdoc comments for all public items"
```

### Power User Workflows

**1. Multi-provider comparison:**

Ask the same question to different providers to compare answers:

```
/provider claude_code
"Explain the tradeoffs between async and sync I/O in Rust"
Ctrl+N
/provider openai
"Explain the tradeoffs between async and sync I/O in Rust"
```

**2. Knowledge-augmented coding:**

Ingest your project docs, then use agent mode. The AI has access to both your code
(via tools) and your documentation (via knowledge base):

```
/kb add ~/docs/api-specs
/agent on
"Implement the user registration endpoint according to the API spec"
```

**3. Prompt chaining with automations:**

Load a file, run the Code Review Pipeline automation, then use agent mode to fix
the issues it found:

```
/file src/auth.rs
/auto run code-review
/agent on
"Fix all the issues identified in the code review above."
```

**4. Exploration sessions:**

Use Nerve to explore an unfamiliar codebase:

```
/workspace
"What is this project? Describe the architecture."
/file src/main.rs:1-30
"What does the entry point do?"
/run find src -name "*.rs" | head -20
"List the main modules and their purposes."
```

---

## Troubleshooting

### "Claude CLI not found" / Provider not available

**Symptom:** Nerve starts but shows an error when you try to send a message with
Claude Code.

**Fix:**

1. Verify the `claude` CLI is installed: `which claude`
2. If not found, install it from [claude.ai/code](https://claude.ai/code).
3. Ensure it is in your PATH. Try running `claude --version` in your terminal.
4. If installed via npm, ensure the npm bin directory is in PATH:
   `export PATH="$HOME/.npm-global/bin:$PATH"`

### "Connection refused" with Ollama

**Symptom:** Nerve shows a connection error when using the Ollama provider.

**Fix:**

1. Ensure Ollama is running: `ollama serve` (or check if the service is active).
2. Verify Ollama is listening on the expected port: `curl http://localhost:11434/api/tags`
3. If using a non-default port or host, update `config.toml`:
   ```toml
   [providers.ollama]
   base_url = "http://your-host:your-port/v1"
   ```
4. Pull a model if you have not already: `ollama pull llama3`

### "API key invalid" with OpenAI or OpenRouter

**Symptom:** 401 Unauthorized error when using OpenAI or OpenRouter.

**Fix:**

1. Verify your API key is set correctly:
   ```bash
   echo $OPENAI_API_KEY        # Should print your key
   echo $OPENROUTER_API_KEY    # Should print your key
   ```
2. If using the config file, ensure the key is correct in `~/.config/nerve/config.toml`.
3. Check that the key has not expired or been revoked on the provider's dashboard.
4. Ensure there are no trailing spaces or newlines in the key.

### Agent mode produces garbled output

**Symptom:** The agent's tool calls are not being parsed correctly, or tool results
appear as raw text.

**Fix:**

1. Use a capable model. Agent mode works best with larger models (sonnet, gpt-4o,
   llama3-70b). Smaller models may struggle with the tool-call format.
2. Start a new conversation (`Ctrl+N`) to clear any confused state.
3. Reduce context pressure: `/compact` before using agent mode.

### Conversation history is missing

**Symptom:** Previous conversations do not appear when you relaunch Nerve.

**Fix:**

1. Check that the history directory exists: `ls ~/.local/share/nerve/history/`
2. Ensure the directory is writable: `touch ~/.local/share/nerve/history/test && rm ~/.local/share/nerve/history/test`
3. If using a custom `XDG_DATA_HOME`, ensure it is set consistently.

### Nerve crashes on startup

**Symptom:** Nerve exits immediately with an error.

**Fix:**

1. Check for error output: `nerve 2>&1 | head -20`
2. Verify your terminal supports the required capabilities (256 color, mouse, alternate
   screen). Most modern terminals (Alacritty, Kitty, WezTerm, iTerm2, GNOME Terminal)
   work fine. Very old terminals or bare TTYs may not.
3. Try rebuilding: `cargo build --release`
4. Delete the config and let it regenerate: `rm ~/.config/nerve/config.toml && nerve`

### Syntax highlighting is wrong or missing

**Symptom:** Code blocks do not have proper syntax highlighting.

**Fix:** Nerve uses syntect for syntax highlighting, which supports most common
languages. If a specific language is not highlighted:

1. Ensure the code block has a language tag: \`\`\`rust, \`\`\`python, etc.
2. Syntect recognizes most common language identifiers. Try the full name if an
   abbreviation does not work.

### Ctrl+K does not open the Nerve Bar

**Symptom:** Pressing Ctrl+K does nothing or inserts a character.

**Fix:**

1. Make sure you are in normal mode (press `Esc` first).
2. Some terminal emulators intercept Ctrl+K for their own purposes. Check your
   terminal's keybinding settings.
3. Try the alternative: type `/` in normal mode to open the Nerve Bar.
4. Remap the keybinding in config.toml if needed:
   ```toml
   [keybinds]
   command_bar = "ctrl+space"
   ```

### High memory usage

**Symptom:** Nerve is using more memory than expected.

**Fix:**

1. Long conversations with many messages accumulate memory. Start a new conversation
   (`Ctrl+N`) periodically.
2. Large knowledge bases consume memory for chunk storage. Use `/kb clear` if you
   no longer need the KB.
3. Run `/compact` to reduce in-memory conversation size.

---

## FAQ

**Q: Does Nerve store my conversations on any server?**

A: No. Conversations are stored locally in `~/.local/share/nerve/history/` as JSON
files. Nerve never sends conversation history to any server other than the AI provider
you are actively using. Your conversation data is yours.

**Q: Can I use Nerve completely offline?**

A: Yes, with Ollama or another locally-hosted provider. Once you have pulled a model,
Nerve works without any internet connection.

**Q: How does Nerve compare to ChatGPT / Claude.ai web apps?**

A: Nerve is designed for developers who work in the terminal. Key advantages over
web apps: file context injection, shell integration, agent mode for autonomous coding,
130 specialized prompts, multi-provider support, keyboard-driven workflow, pipe mode
for scripting. The tradeoff is no GUI, no image generation, and no web browsing (beyond
URL scraping).

**Q: Can I use Nerve with models behind a corporate proxy?**

A: Yes. Set the `HTTPS_PROXY` or `HTTP_PROXY` environment variable, and reqwest
(Nerve's HTTP client) will use it. For custom endpoints behind a VPN, configure
the base_url in the custom provider config.

**Q: How do I add my own models to the model list?**

A: For Ollama, pull the model (`ollama pull modelname`) and it will appear in `/models`.
For OpenAI and OpenRouter, models are fetched from the API. For custom providers,
the model list is queried from the base_url endpoint.

**Q: Is there a way to set a default system prompt for all conversations?**

A: Currently, system prompts are per-conversation (`/system <prompt>`). To use the
same system prompt every time, create a custom prompt template and select it at the
start of each conversation. A global default system prompt configuration is planned.

**Q: Can I use Nerve in tmux / screen?**

A: Yes. Nerve works correctly in tmux and screen. Ensure your tmux configuration
supports 256 colors (`set -g default-terminal "tmux-256color"`).

**Q: How do I update Nerve?**

A: Pull the latest changes and rebuild:

```bash
cd /path/to/nerve
git pull
cargo build --release
cp target/release/nerve ~/.local/bin/
```

If you used a symlink during installation, just `git pull && cargo build --release`.

**Q: What is the difference between `/run` and `/pipe`?**

A: `/run` executes a command and displays the output in the chat, but does not add
it as AI context. `/pipe` executes the command and adds the output as context, so
the AI can analyze it. Use `/run` when you just want to see output; use `/pipe` when
you want the AI to reason about the output.

**Q: Can I use multiple providers in the same conversation?**

A: Yes. Switch providers at any time with `Ctrl+T` or `/provider`. The conversation
history is preserved, and Nerve re-sends the context to the new provider. Note that
token estimates may differ between providers.

**Q: Does Nerve support image input or output?**

A: No. Nerve is a text-based terminal application. It does not support image input
(multimodal prompts) or image generation. For those use cases, use the web interface
of your AI provider.

**Q: Why is agent mode limited to 10 iterations?**

A: The 10-iteration limit prevents runaway loops where the AI repeatedly calls tools
without making progress. If the agent has not completed the task in 10 iterations,
it stops and reports its progress. You can then give further instructions and it will
continue.

---

## Migrating from Other AI Tools

### From ChatGPT / Claude.ai Web Interface

**What you gain:**
- File context injection (no more copy-pasting code)
- Shell integration (run tests, build, diff -- all from the AI conversation)
- Agent mode for autonomous coding
- 130 specialized prompts
- Pipe mode for scripting
- Keyboard-driven workflow

**Getting started:**
1. Install Nerve and a provider.
2. Navigate to your project directory: `cd ~/my-project && nerve`
3. Instead of pasting code, use `/file src/main.rs`.
4. Instead of describing test output, use `/test` or `/pipe cargo test`.

### From GitHub Copilot (VS Code)

**What you gain:**
- Full conversation context (not just cursor-adjacent code)
- Multi-file analysis
- Agent mode with file/command tools
- Multiple AI providers
- Works outside VS Code

**What you lose:**
- Inline code suggestions as you type
- IDE integration (diagnostics, refactoring)

**Recommendation:** Use both. Copilot in VS Code for inline suggestions; Nerve in
a terminal for deeper analysis, code review, and agent-driven changes.

### From Cursor / Continue

**What you gain:**
- No Electron overhead (~7MB vs hundreds of MB)
- Works with any terminal, any editor
- True provider independence (6 providers, not locked to one)
- Open source, MIT licensed
- Pipe mode for shell scripting

**What you lose:**
- IDE integration (inline diffs, code actions)
- Visual diff preview

**Recommendation:** Nerve is best for developers who prefer a terminal-centric
workflow. If you need tight IDE integration, Cursor/Continue may be better for
real-time editing. Nerve excels at code review, architecture discussions, debugging,
and agent-driven multi-file changes.

### From Aider

**What you gain:**
- Beautiful TUI with syntax highlighting
- Nerve Bar with 130 smart prompts
- 6 providers (not just OpenAI)
- Knowledge base (RAG)
- Project scaffolding
- Conversation history with browser
- Clipboard manager

**What you lose:**
- Aider's git-aware editing (automatic commits)
- Aider's repository map feature

**Key difference:** Aider focuses on git-integrated code editing. Nerve is a broader
AI assistant that includes agent mode for coding but also handles writing, analysis,
design, and general-purpose AI tasks.

---

## Performance Tuning

### Startup Time

Nerve starts in under 100ms. If startup seems slow:

1. Check if the config file is very large or malformed.
2. If you have many conversation history files (thousands), startup may be slightly
   slower due to directory scanning. Archive old history files periodically.
3. The `--provider` flag avoids loading unused provider configs.

### Streaming Latency

Streaming performance depends on the provider:

- **Claude Code:** Streams via the claude CLI. Latency depends on Claude's servers
  and the CLI's buffering.
- **OpenAI / OpenRouter:** Streams via SSE (Server-Sent Events) over HTTP. Very
  responsive. First-token latency depends on the provider and model.
- **Ollama:** Local streaming. Latency depends entirely on your hardware. GPU
  acceleration (CUDA, Metal) makes a large difference.

**Tips for faster responses:**
- Use smaller models for simple tasks (haiku, gpt-4o-mini, llama3-8b).
- Keep context small. Smaller context = faster inference, especially on Ollama.
- For Ollama, ensure GPU acceleration is working: `ollama run llama3 "test"` should
  produce tokens quickly.

### Memory Usage

Nerve's base memory footprint is small (~20-30MB). Memory grows with:

1. **Conversation length:** Each message is stored in memory. Long conversations with
   many file contents can grow to 50-100MB. Use `/compact` or start new conversations.
2. **Knowledge base:** Chunks are stored in memory. A large KB (thousands of documents)
   can use significant memory. Use `/kb status` to check.
3. **Clipboard history:** Stored in memory for the session. Very large clipboard
   entries (entire files) can add up.

### Binary Size

The release binary is ~7MB thanks to:

- LTO (link-time optimization)
- Single codegen unit
- Symbol stripping
- Opt-level 3

If you need an even smaller binary, consider using `opt-level = "z"` in `Cargo.toml`
(optimizes for size instead of speed), though the performance difference is minimal.

### Ollama-Specific Tuning

For the best local experience with Ollama:

1. **GPU acceleration:** Ensure your GPU drivers are installed and Ollama detects
   your GPU (`ollama run llama3 "test"` -- check tokens/second).
2. **Context window:** Increase `num_ctx` in your modelfile for longer conversations.
   Default is often 2048 or 4096. Nerve's default limit for Ollama is 8000 tokens.
3. **Model selection:** For coding tasks, `codellama` or `deepseek-coder` often
   outperform general-purpose models. For general chat, `llama3` or `mistral` are
   good choices.
4. **Quantization:** If memory is limited, use quantized models (Q4_K_M, Q5_K_M).
   These are smaller and faster with minimal quality loss.

---

## Architecture Overview

For contributors and curious users, here is the high-level project layout:

```
nerve/
├── src/                          38 files, ~18K lines of Rust
│   ├── main.rs                   Entry point, event loop, 38 slash commands
│   ├── app.rs                    Application state machine
│   ├── config.rs                 TOML configuration (load/save/defaults)
│   ├── daemon.rs                 Background daemon (Unix socket IPC)
│   ├── automation.rs             Multi-step AI pipelines
│   ├── history.rs                Conversation persistence (JSON)
│   ├── clipboard.rs              System clipboard integration (arboard)
│   ├── clipboard_manager.rs      Clipboard history with fuzzy search
│   ├── keybinds.rs               Keybind string parser
│   ├── files.rs                  File reading with line ranges
│   ├── shell.rs                  Shell command execution
│   ├── workspace.rs              Project type detection (10+ languages)
│   ├── scaffold.rs               Project scaffolding (8 templates)
│   ├── ai/
│   │   ├── mod.rs                Module exports
│   │   ├── provider.rs           AiProvider trait + message types
│   │   ├── claude_code.rs        Claude Code CLI integration
│   │   ├── copilot.rs            GitHub Copilot CLI integration
│   │   └── openai.rs             OpenAI-compatible API client (SSE streaming)
│   ├── agent/
│   │   ├── mod.rs                Module exports
│   │   ├── tools.rs              7 agent tools (read/write/edit/run/list/search/mkdir)
│   │   └── context.rs            Token management and auto-compaction
│   ├── ui/
│   │   ├── mod.rs                Layout and rendering dispatch (ratatui)
│   │   ├── chat.rs               Syntax-highlighted chat view (syntect)
│   │   ├── command_bar.rs        Nerve Bar (fuzzy command palette)
│   │   ├── prompt_picker.rs      SmartPrompt browser (two-panel layout)
│   │   ├── history_browser.rs    Conversation history viewer
│   │   ├── clipboard_manager.rs  Clipboard history overlay
│   │   ├── search.rs             In-conversation search
│   │   └── help.rs               Keybinding reference overlay
│   ├── prompts/
│   │   ├── mod.rs                Prompt loading and category listing
│   │   ├── builtin.rs            130 built-in prompt templates
│   │   └── custom.rs             User-defined prompt loading (.toml)
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
│   └── GUIDE.md                  This file
├── Cargo.toml                    Dependencies and build config
├── LICENSE                       MIT License
└── README.md                     Project overview and quick start
```

### Key Abstractions

**AiProvider trait** (`src/ai/provider.rs`): The interface every backend implements.
Methods: `chat_stream` (streaming response), `chat` (single response), `list_models`,
and `name`. Adding a new provider means implementing this trait and wiring it into
`create_provider` in `main.rs`.

**AgentTool** (`src/agent/tools.rs`): Defines the 7 tools available in agent mode.
Each tool has a name, description, parameter schema, and an execution function. The
tool descriptions are included in the system prompt so the AI knows how to call them.

**SmartPrompt** (`src/prompts/builtin.rs`): A struct with name, description, template,
category, and tags. The 130 built-in prompts are defined as constants in this file.

**App state machine** (`src/app.rs`): Manages the application state including input
mode (normal/insert), active overlay (None, NerveBar, PromptPicker, etc.), conversation
list, and provider state.

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| ratatui + crossterm | Terminal UI framework and backend |
| tokio | Async runtime for streaming and I/O |
| reqwest | HTTP client for OpenAI-compatible APIs |
| serde + toml + serde_json | Configuration and API serialization |
| syntect | Syntax highlighting in code blocks |
| pulldown-cmark | Markdown parsing for rich text display |
| fuzzy-matcher | Fuzzy search in Nerve Bar, KB, clipboard manager |
| clap | CLI argument parsing |
| arboard | System clipboard access |
| chrono | Timestamps for conversations and history |
| uuid | Unique conversation identifiers |
| anyhow + thiserror | Error handling (anyhow for application, thiserror for library) |
| dirs | XDG Base Directory paths |
| tracing + tracing-subscriber | Structured logging |

### Test Suite

The test suite contains 450 tests across all 38 source files. Tests cover:

- AI provider abstractions (mock providers, message formatting)
- Agent tool parsing (tool call extraction, parameter validation)
- Context management (compaction logic, token estimation, boundary detection)
- Prompt loading (built-in count verification, custom TOML parsing)
- Configuration serialization (load, save, defaults, migration)
- File reading with line ranges (edge cases, out-of-bounds handling)
- Workspace detection (each project type, missing files, malformed manifests)
- Scaffold templates (file generation, directory structure)
- Knowledge base (ingestion, chunking, search ranking)
- Clipboard management (history ordering, deduplication)
- Keybind parsing (key combinations, modifier keys, edge cases)
- UI rendering logic (layout calculations, text wrapping)
- Automation pipeline execution (step chaining, error handling)

Run the full suite:

```bash
cargo test
```

Run tests for a specific module:

```bash
cargo test --lib config     # Config tests only
cargo test --lib agent      # Agent tests only
cargo test --lib prompts    # Prompt tests only
```

---

*Built by [Artaeon](https://github.com/Artaeon) -- [raphael.lugmayr@stoicera.com](mailto:raphael.lugmayr@stoicera.com)*
