# Changelog

All notable changes to Nerve will be documented in this file.

## [0.1.0] - 2025-04-01

### Initial Release

Nerve is a terminal-native AI assistant and coding agent built in Rust.

#### AI Providers
- Claude Code (no API key needed — uses subscription)
- OpenAI (API key)
- OpenRouter (API key, 100+ models)
- Ollama (local, free, offline)
- GitHub Copilot (gh CLI)
- Custom OpenAI-compatible endpoints

#### Chat
- Streaming responses with syntax-highlighted code blocks
- Markdown rendering (bold, italic, headers, lists, blockquotes, links)
- Line numbers in code blocks
- Animated thinking spinner and streaming progress bar
- Message number badges (1-9) for quick copy
- Multi-line input with Shift+Enter
- Dynamic input area that grows with content
- Input history with Up/Down arrows
- Vim keybindings (i/Esc, j/k scroll)
- Mouse scroll support

#### Agent Mode
- 9 tools: read_file, write_file, edit_file, run_command, list_files, search_code, create_directory, find_files, read_lines
- Plan-execute-observe loop (max 10 iterations)
- Robust tool call parsing (handles XML tags, JSON, markdown fences, missing closing tags)
- Auto-verify file syntax after edits (Rust, Python, JS, JSON, YAML, TOML)
- Git safety net (auto-stash on start, /agent undo to rollback)
- Project map injection (file tree + key symbols)
- Provider-aware context compaction
- Works with any provider (not just Claude Code)

#### Smart Prompts
- 166 expert prompts across 28 categories
- Categories: Engineering, Coding, Writing, Design, Git, Rust, Python, TypeScript, Go, UI/UX, Testing, Security, API, Database, Cloud, DevOps, Business, Marketing, and more
- Each prompt is 5-15 lines of detailed system instructions
- Nerve Bar (Ctrl+K) with fuzzy search, category tabs, and template preview
- Custom prompts via TOML files

#### Developer Workflow
- File context: /file, /files, @file inline references
- Shell integration: /run, /test, /build, /diff, /pipe, /git
- Workspace detection (Rust, Node, Python, Go, Java, Ruby, Elixir, Zig, C#, C++)
- Project scaffolding: 8 templates + AI-generated /scaffold
- Tab completion for file paths and slash commands

#### Productivity
- Persistent sessions (auto-save, resume with --continue)
- Conversation branching (/branch save, restore, diff)
- Conversation history browser (Ctrl+O) with search, sort, delete confirmation
- Clipboard manager (Ctrl+B) with fuzzy search
- In-chat search (Ctrl+F)
- Stop (Esc), regenerate (Ctrl+R), edit (Ctrl+E) responses
- Export conversations as markdown
- Knowledge base (RAG) with document ingestion
- Automations (5 built-in multi-step pipelines)
- Command aliases (/alias)

#### Settings & Customization
- Interactive settings overlay (Ctrl+,) with 4 tabs
- 8 color themes (Catppuccin, Tokyo Night, Gruvbox, Nord, Solarized, Dracula, One Dark, Rose Pine)
- TOML configuration at ~/.config/nerve/config.toml
- Plugin system (executable scripts in ~/.config/nerve/plugins/)

#### Token Management
- Provider-aware context limits (Claude 200K, OpenAI 60K, OpenRouter 30K, Ollama 8K)
- Auto-compaction with smart summarization
- Tool result compaction in agent mode
- Usage tracking (/usage) and spending limits (/limit)
- max_tokens on OpenAI/OpenRouter requests

#### Security
- Shell injection blocking (20+ dangerous patterns)
- Protected system paths (agent can't write to /etc, /usr, /bin, etc.)
- Sensitive file blocking (.env, SSH keys, .aws/credentials)
- Tool execution rate limiting (100/session)
- Config file permissions (0600 on Unix)
- API key masking in display

#### CLI
- Non-interactive mode: nerve -n "prompt"
- Pipe mode: cat file | nerve --stdin -n "review"
- Daemon mode: nerve --daemon / --query / --stop-daemon
- Resume sessions: nerve --continue
- Provider/model override: nerve --provider ollama --model llama3

#### Quality
- 820 tests across 42 source files
- 0 clippy warnings
- 0 unsafe code
- 0 panics in production code
- Graceful error handling with helpful messages
- CI/CD with GitHub Actions (build, test, clippy, auto-release)
