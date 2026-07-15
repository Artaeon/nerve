# Nerve — Plugins

A plugin adds a **slash command** to nerve that runs a script you control. It's
the escape hatch for "I want nerve to be able to do *this specific thing* in *my*
environment" without touching nerve's source.

**The point: plugins are token-free.** A plugin is not a tool the model calls and
not context that rides along on every turn — it costs **zero tokens until you
invoke it**, and then only its (bounded) output enters the conversation. Anything
deterministic and local — scanning the repo, querying your DB, hitting an
internal API, checking deploy status — is cheaper and more reliable as a plugin
than as a pile of model tool-calls.

---

## Quick start

```bash
mkdir -p ~/.config/nerve/plugins/todos
cp -r examples/plugins/todos/* ~/.config/nerve/plugins/todos/
chmod +x ~/.config/nerve/plugins/todos/run.sh
```

Then in the TUI:

```
/plugin reload    # pick it up without restarting
/todos            # all TODO/FIXME/HACK/XXX markers, grouped by file
/todos src/       # scoped to a path
```

### The three built-in commands

| Command | What it does |
|---|---|
| `/plugin list` | Every installed plugin: its slash command, version, enabled/disabled, description |
| `/plugin reload` | Re-read `~/.config/nerve/plugins/` — use after adding or editing a plugin |
| `/plugin init` | Scaffold a starter plugin you can edit |

Plugins are loaded from `~/.config/nerve/plugins/` at startup, and `/plugin
reload` re-reads them live.

---

## Anatomy

One directory per plugin, containing a manifest and a script:

```
~/.config/nerve/plugins/todos/
├── plugin.toml     # the manifest
└── run.sh          # the script (any executable: bash, python, a binary…)
```

### `plugin.toml`

```toml
name = "TODO scanner"                  # human name, shown in /plugins
description = "List TODO/FIXME markers. Usage: /todos [path]"
version = "1.0.0"
author = "You"                         # optional
command = "todos"                      # invoked as /todos
run = "run.sh"                         # script path, relative to this dir
enabled = true                         # omit to default to true
```

A directory without a `plugin.toml` is skipped. A manifest that fails to parse is
skipped with a warning (it never breaks startup). `enabled = false` keeps a
plugin installed but unloaded.

### The script contract

| | |
|---|---|
| **Arguments** | Everything after the command, split on whitespace, as `$1..$n` |
| **`$NERVE_ARGS`** | The same arguments, unsplit, as one string |
| **`$NERVE_PLUGIN_DIR`** | Absolute path to the plugin's own directory (for bundled data/config) |
| **Working directory** | The plugin's own directory — `cd` yourself if you need the repo |
| **stdin** | Closed (EOF) — a script that reads stdin won't hang |
| **Result** (exit 0) | Whatever you print to **stdout**. If stdout is empty, **stderr** is used instead |
| **Exit code** | **Matters.** Non-zero = nerve reports the plugin as *failed* and shows your **stderr** instead of the result. Exit `0` unless you genuinely want an error |

### Limits (enforced by nerve)

- **30 second timeout** — the process is killed past it.
- **1 MB output cap** — nerve keeps draining the pipe past the cap (so your
  script never blocks on a full pipe) but retains only the first 1 MB.
- **ANSI and control characters are stripped** from the output (newlines and
  tabs survive), so colored CLI output won't corrupt the chat view.
- The script is `chmod +x`'d automatically on Unix.
- It runs on a blocking thread pool, so a slow plugin never freezes the TUI.

---

## Writing a good plugin

**Do the work locally; return a small answer.** The whole value is spending your
CPU instead of the model's context. Aim for a compact, already-summarized result.

**Bound your own output.** The 1 MB cap is a backstop, not a design. Cap rows,
truncate long lines, and say what you dropped — see how the `todos` example
prints `— showing first 200 of 412 markers`.

**Make it deterministic.** The same input should give the same output. Plugins
are most useful precisely where you *don't* want a model guessing.

**Mind the exit code.** nerve treats a non-zero exit as a plugin *failure* and
surfaces stderr instead of your output — so end with `exit 0` on the normal path.
"Found nothing" is a normal result, not an error: print `No matches.` and exit 0.
Avoid bare `set -e` in a script whose commands legitimately return non-zero
(`grep` with no match is the classic trap).

**Respect the repo.** Prefer `git grep` over `grep -r`: it honors `.gitignore`
for free, so you don't scan `node_modules/` or `target/`.

### A minimal example

```bash
#!/usr/bin/env bash
set -uo pipefail
echo "args: $NERVE_ARGS"
echo "plugin dir: $NERVE_PLUGIN_DIR"
```

```toml
name = "Hello"
description = "Minimal plugin"
version = "1.0.0"
command = "hello"
run = "run.sh"
```

Then `/hello world`.

### A real example — `examples/plugins/todos/`

Scans the repo for `TODO|FIXME|HACK|XXX`, groups by file, trims comment leaders,
caps at 200 rows, and prefers `git grep` so ignored files are skipped. Read it as
the reference implementation: it shows arg handling, the git fallback, output
bounding, and an honest truncation note.

---

## Security

Plugins are **your own code, run with your own privileges** — nerve does not
sandbox them. Only install plugins you wrote or have read. nerve does protect the
conversation from a plugin's output (control-character stripping, size cap,
timeout), but it cannot protect your machine from a script you chose to install.

Because the model never invokes plugins — **you** do, via a slash command — a
prompt-injected model cannot trigger one.

---

## Troubleshooting

| Symptom | Cause |
|---|---|
| Command not found | Run `/plugin reload` (nerve only auto-loads at startup). Confirm with `/plugin list`. |
| Nothing happens | `enabled = false`, or `command` in the manifest doesn't match what you typed. |
| Plugin missing from `/plugin list` | Manifest failed to parse — check nerve's log for the warning, and validate the TOML. |
| "Plugin script not found" | `run` must be relative to the plugin directory (e.g. `run.sh`, not `./run.sh` or an absolute path). |
| Output cut off | You hit the 1 MB cap — summarize in the script. |
| Killed after ~30s | The timeout. Do less work, or cache into `$NERVE_PLUGIN_DIR`. |
| Wrong directory | The script starts in the **plugin's** directory, not your repo. |
