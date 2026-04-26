# Nerve — 10-minute walkthrough

A hands-on tour of Nerve's most useful features, in the order you'll
actually run into them. For the full reference, see [GUIDE.md](GUIDE.md).

This document assumes you have `nerve` on your PATH. If not:

```bash
git clone https://github.com/Artaeon/nerve.git
cd nerve
cargo build --release
cp target/release/nerve ~/.local/bin/
```

---

## 1. First launch (30 seconds)

```bash
nerve
```

You'll see a splash, then drop into the chat TUI. The bottom-left of
the status bar shows the active provider and model. The default is
**Claude Code** with the **sonnet** model — works out of the box if
you already have the `claude` CLI installed and signed in.

If you don't have Claude Code, switch to a provider you do have:

```
Ctrl+T          # opens the provider picker
```

Pick one (Ollama for local, OpenAI / OpenRouter for API keys, GitHub
Copilot if you have a Copilot subscription). For OpenAI / OpenRouter,
set the relevant env var before launch:

```bash
export OPENAI_API_KEY=sk-...
nerve
```

---

## 2. Send your first message

Type something and hit Enter:

```
> explain what TCP slow start does in one paragraph
```

Tokens stream live as the model produces them. At any point, **press
Esc to stop generation** — the spawned task is killed instantly, no
trailing tokens.

---

## 3. Send a file as context

Two ways. The first uses the `/file` command:

```
> /file src/main.rs
> what does this file do?
```

The contents of `src/main.rs` are added to the conversation. Subsequent
messages reference them.

The second uses inline `@` syntax:

```
> review @src/main.rs:1-50 — focus on the parser
```

Only lines 1 to 50 are included.

---

## 4. Pick a smart prompt

Press `Ctrl+K`. The prompt picker opens with 186 built-in prompts in
30 categories. Filter by typing — it's fuzzy-matched.

Useful ones to know:

- **Fix Bug** — give it code, get back a diagnosis and fix
- **Refactor** — applies SOLID / clean-code rules
- **Review for Race Conditions** — concurrency audit
- **Diagnose a Slow Database Query** — EXPLAIN-driven workflow
- **Plan a Major Framework Upgrade** — phased upgrade plan with rollback points

Hit Enter on one — it fills the input box with the prompt template.
Replace `{{input}}` with your code, then Enter to send.

---

## 5. Don't know which prompt to use? Ask `/suggest`

```
> /suggest my login endpoint returns 500 sometimes
```

Returns the top 5 prompts ranked by relevance, with scores. Pure
local, ~1ms — no API call. Open `Ctrl+K` to use one.

---

## 6. Agent mode — let Nerve do the work

Toggle agent mode on:

```
> /agent on
```

Now Nerve has 10 tools: read_file, write_file, edit_file, run_command,
list_files, search_code, find_files, create_directory, read_lines,
web_search.

Try:

```
> the test in tests/auth.rs is failing intermittently. read the file, run the test, then fix it.
```

Nerve reads the file, runs the test, sees the failure, edits the
fix, re-runs the test, and reports back. Each tool call is shown
in the status bar in real time.

To roll back everything the agent did:

```
> /agent undo
```

To see only what changed:

```
> /agent diff
```

---

## 7. Multi-agent workflow — `/workflow`

For non-trivial tasks, run the 3-role pipeline:

```
> /workflow add a --json output flag to the CLI
```

Three roles execute in sequence on a fresh conversation:

1. **Planner** — produces a numbered plan (no tools)
2. **Coder** — executes the plan (full tools)
3. **Reviewer** — inspects the result (read-only tools)

The status bar shows which role is active. The reviewer ends with one
of three verdicts:

- `VERDICT: APPROVED` — ready to use
- `VERDICT: NEEDS FIXES` — minor issues to address
- `VERDICT: REJECTED` — fundamental problems

The reviewer is enforced read-only at the tool layer — even if the
LLM ignores its system prompt and tries `write_file`, the call is
refused before execution.

To stop the workflow at any time, press **Esc**. To abandon a paused
workflow and start fresh, type `/new`.

---

## 8. Pipe input from the shell

```bash
git diff | nerve --stdin -n "write me a commit message"
cat src/main.rs | nerve --stdin -n "list the public API"
nerve -n "explain TCP slow start" > tcp.txt
```

`-n` is one-shot mode (prints and exits), `--stdin` reads piped input.

---

## 9. Sessions and history

Every conversation is auto-saved.

```bash
nerve --continue        # resume the last session on launch
```

Inside the TUI:

```
Ctrl+O                  # browse all past conversations
/session save           # explicitly snapshot current session
/branch save            # checkpoint inside a conversation
/branch restore <name>  # roll back to a checkpoint
```

History is stored in `~/.local/share/nerve/`. To wipe it: delete the
directory. To export the current conversation as markdown:

```
/export
```

---

## 10. Switching models / providers mid-conversation

```
Ctrl+M                  # model picker for the current provider
Ctrl+T                  # provider picker
```

Switching providers mid-chat doesn't break context — the conversation
history is sent in the next message regardless of which provider
serves it.

---

## Common gotchas

- **Provider hangs forever**: shouldn't happen — every provider has a
  read timeout. If it does, check `RUST_LOG=nerve=warn nerve` for
  diagnostics. The retry log scrubs Bearer tokens / API keys from
  upstream errors automatically, so it's safe to share.
- **Long-running agent tool blocks the UI**: it doesn't anymore. Tool
  execution runs on a separate thread; the UI keeps rendering during
  `npm install`, `cargo test`, etc. The status bar shows progress.
- **Workflow gets stuck in a "paused" state**: type `/new` to clear
  the workflow and start a fresh conversation.
- **Esc didn't fully cancel**: the task is killed, but if it had
  already spawned a subprocess (a tool's `run_command`), that
  subprocess is killed via SIGKILL when the future is dropped.

---

## What to read next

- [GUIDE.md](GUIDE.md) — the comprehensive reference (2300+ lines)
- [MANIFESTO.md](../MANIFESTO.md) — the design philosophy
- `nerve --help` — full CLI flag list
- `/help` inside the TUI — full slash-command reference

---

## Useful one-liners cheat sheet

```bash
# Quick code review of staged changes
git diff --cached | nerve --stdin -n "code review this diff"

# Generate a commit message
git diff --cached | nerve --stdin -n "write a conventional commit message" | git commit -F -

# Pipe in a stack trace, get an explanation
some_command 2>&1 | nerve --stdin -n "explain this error"

# Use as a smart sed for natural-language refactors
nerve -n "in src/main.rs, rename `handle_input` to `dispatch_input` and update all callers"
# (then run with /agent on inside the TUI for the actual changes)

# One-off Q&A without entering the TUI
nerve -n "regex for ISO 8601 dates that allows offsets"

# Explain a config / cron / systemd unit you don't recognize
cat /etc/systemd/system/foo.service | nerve --stdin -n "what does this unit do?"
```

```text
# Inside the TUI: keystrokes that pay for themselves
Ctrl+K          prompt library (fuzzy filter)
Ctrl+T          switch provider
Ctrl+M          switch model
Ctrl+N          new conversation (cancels in-flight stream)
Ctrl+O          history browser
Ctrl+R          regenerate last response
Ctrl+E          edit last message and resend
Ctrl+F          search in current conversation
Esc             stop generation / cancel workflow
```

That's enough to be productive. Read the rest as you need it.
