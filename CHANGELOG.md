# Changelog

All notable changes to Nerve will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1](https://github.com/Artaeon/nerve/compare/nerve-v0.2.0...nerve-v0.2.1) (2026-04-12)


### Bug Fixes

* let / insert character in Insert mode instead of opening Nerve Bar ([a45d149](https://github.com/Artaeon/nerve/commit/a45d14924a2d8f7e5364fae55d9dc92675b5bd8d))

## [Unreleased]

### Added

#### Developer Workflow
- `/lint` command -- auto-detect and run project linter (clippy, eslint, ruff, golangci-lint, rubocop, credo)
- `/format` (`/fmt`) command -- auto-detect and run code formatter (cargo fmt, prettier, ruff, gofmt, rubocop, mix format)
- `/search <pattern>` command -- search codebase with ripgrep, results added as AI context
- `/commit` now uses `--author` from configured git_user_name/email
- AI-generated commit messages via `/commit` (no message argument)

#### AI & Prompt Engineering
- `temperature` config option (0.0-2.0) for controlling response creativity
- `top_p` config option (0.0-1.0) for nucleus sampling
- `context_limit` config option to override provider default context window size
- Mode-specific system prompts: Efficient (concise), Thorough (detailed), Agent (workflow), Learning (Socratic)
- Ollama default context raised from 8K to 32K tokens

#### UI/UX
- Color-coded token usage percentage in status bar (green/yellow/red)
- Vim `G`/`g` keys to jump to bottom/top of conversation
- `PageUp`/`PageDown` for fast scrolling (30 lines)
- Context-aware status bar hints (different hints per mode)
- Working directory display in status bar when code mode is active
- Auto-agent mode: automatically enables tools when message needs them

### Fixed

#### Security Hardening
- **SSRF**: Proper URL parsing replaces string-matching blocklist; blocks IPv6 loopback, link-local, ULA (fc00::/7), multicast (ff00::/8), IPv4-mapped IPv6, IPv4-compatible IPv6, CGN range
- **SSRF**: Final URL re-validated after redirect chain to prevent redirect-based SSRF
- **Command injection**: All agent tool shell commands use `shell_escape()` (verify_file_syntax, search_code, find_files, git_diff)
- **Path traversal**: `normalize_path()` resolves `..` segments without filesystem access; blocks `/tmp/x/../../etc/passwd`
- **Path traversal**: History conversation IDs sanitized to alphanumeric+hyphens
- **Symlink attacks**: `validate_write_path()` canonicalizes paths before protection check
- **Device file DoS**: Block char/block devices, FIFOs, sockets, symlinks from file reads
- **Command filter**: Block poweroff, halt, systemctl reboot, shred, wipe, chpasswd; block pipe to zsh, python, perl, ruby, ksh, dash
- **ANSI injection**: Strip ANSI escape sequences (CSI, OSC, SS2/SS3) from plugin output
- **Clipboard**: Propagate permission-setting errors instead of silently ignoring
- **Knowledge search**: Fix ln(0) edge case when scoring empty chunks
- **Memory**: `truncate_output()` avoids collecting all lines before truncating

### Changed
- Removed dead `/commit` handler from shell.rs (git.rs handles it)
- Moved `build_git_author_flag` to git.rs where it's used

### Quality
- Tests: 820 -> 1,345 (+525 tests)
- 0 clippy warnings
- 0 formatting issues
- 11 security vulnerabilities fixed

## [0.1.0] - 2025-04-01

### Added

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
- 10 color themes (Catppuccin, Tokyo Night, Gruvbox, Nord, Solarized, Dracula, One Dark, Rose Pine, High Contrast, Monochrome)
- TOML configuration at ~/.config/nerve/config.toml
- Plugin system (executable scripts in ~/.config/nerve/plugins/)

#### Token Management
- Provider-aware context limits (Claude 200K, OpenAI 60K, OpenRouter 30K, Ollama 32K)
- Auto-compaction with smart summarization
- Tool result compaction in agent mode
- Usage tracking (/usage) and spending limits (/limit)
- max_tokens on OpenAI/OpenRouter requests

#### Security
- Shell injection blocking (30+ dangerous patterns)
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
- 820 tests across 42 source files (now 1,345 tests across 55 files)
- 0 clippy warnings
- 0 unsafe code
- 0 panics in production code
- Graceful error handling with helpful messages
- CI/CD with GitHub Actions (build, test, clippy, auto-release)

[Unreleased]: https://github.com/Artaeon/nerve/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Artaeon/nerve/releases/tag/v0.1.0
