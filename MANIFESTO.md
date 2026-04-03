# The Nerve Manifesto

## Why Nerve Exists

We built Nerve because every AI coding tool we tried hit a nerve.

**Claude Code** is brilliant but locks you into one provider. Your workflow shouldn't depend on a single company's pricing decisions.

**Cursor** is powerful but it's a 500MB Electron app that consumes half your RAM before you write a line of code. Developers who live in the terminal shouldn't need a GUI IDE.

**ChatGPT and Claude.ai** are great for questions but terrible for coding. Copy-pasting between a browser and your terminal breaks flow. Context is lost. Files aren't read. Commands aren't run.

**Aider** gets it right with terminal-first design, but it's Python-based, slow to start, and focuses narrowly on git-integrated editing.

None of them give you everything: terminal-native speed, multi-provider freedom, an autonomous coding agent, AND a rich library of expert prompts. So we built Nerve.

## Design Principles

### 1. Terminal-First

The terminal is where developers work. Not a browser tab. Not an Electron wrapper. Nerve is a 7.7MB Rust binary that starts in 400ms and uses 20MB of memory. It works over SSH, in tmux, on a Raspberry Pi, on a server with no GUI.

### 2. Provider-Agnostic

Your AI tool should not be your AI vendor. Nerve works with Claude Code, OpenAI, OpenRouter, Ollama, GitHub Copilot, and any OpenAI-compatible endpoint. Switch with Ctrl+T. No lock-in. No migration. No vendor anxiety.

### 3. Token-Efficient

AI tokens cost money. Nerve tracks every token, auto-compacts conversations when they get long, and lets you set spending limits. It doesn't waste context on verbose system prompts or redundant file re-reads. Every token counts.

### 4. Agent, Not Autocomplete

Nerve doesn't just suggest the next line of code. It reads your files, understands your project, makes changes, runs tests, and iterates. With a git safety net so you can always roll back. The agent works with ANY provider, not just expensive cloud APIs.

### 5. Batteries Included

166 expert prompts. 8 project templates. 9 agent tools. 10 color themes. 70+ commands. Plugin system. Knowledge base. Conversation branching. Clipboard manager. All in one binary. No extensions to install, no marketplace to browse, no configuration hell.

### 6. Open and Honest

Nerve is MIT-licensed. The code is readable. The tests are comprehensive (1,345 of them). We don't hide pricing behind "contact sales." We don't collect telemetry. We don't phone home. Your conversations stay on your machine.

## What Nerve Is Not

- **Not an IDE.** Nerve doesn't replace your editor. It works alongside vim, neovim, VS Code, or whatever you use. It's the AI brain, not the code editor.
- **Not a chatbot.** Every feature is designed for developers building real software. No small talk, no creative writing games. Code, ship, repeat.
- **Not finished.** This is v0.1.0. MCP support, LSP integration, and repository intelligence are coming. Contributions welcome.

## The Name

"Nerve" because:
- It's the **nerve center** of your development workflow
- It takes **nerve** to build an open-source competitor to well-funded tools
- Other tools **hit a nerve** -- Nerve is the cure

---

*Built in Rust. Tested obsessively. Designed for developers who value their terminal, their time, and their token budget.*
