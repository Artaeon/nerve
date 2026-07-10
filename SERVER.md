# Nerve server & client — code 24/7

One `nerve` binary, two roles. Run it as a **server** on an always-on machine;
from your **laptop**, connect to it, hand off a project, and close the lid — the
server keeps working through its queue and commits results for you to review.

Everything here is built and verified end-to-end against a real server.

---

## The model

- **Server** — an always-on machine with `nerve`, an authenticated AI provider,
  and each project's toolchain. It runs `nerve --daemon`, which keeps a
  persistent **job queue** and a **worker** that executes jobs.
- **Client** — your laptop's `nerve` TUI (or the CLI). You connect to the
  server, watch its live queue, and schedule work on it. Closing the laptop
  doesn't stop anything.
- **Transport = SSH.** There is **no new network port**. The daemon listens only
  on a HOME-anchored Unix socket (`~/.nerve/nerve.sock`, dir `0700`, socket
  `0600` — owner-only). The client reaches it by running `nerve` on the server
  over the SSH you already trust. Nothing new is exposed to the internet.
- **Guardrail** — every job runs on its own `nerve/job-<id>` git branch and is
  committed there. Results never land on your working branch unreviewed.

---

## Server setup (once)

```bash
# on the server:
cargo build --release                       # or copy the nerve binary onto PATH
sudo ln -s $PWD/target/release/nerve /usr/local/bin/nerve

# authenticate an AI provider — pick ONE:
claude login                                # Claude Code CLI (interactive), or
export ANTHROPIC_API_KEY=sk-...             # non-interactive, or
#   configure an OpenAI/OpenRouter key in ~/.config (see "Providers" below)
```

Run the daemon 24/7 under systemd (`/etc/systemd/system/nerve.service`):

```ini
[Unit]
Description=Nerve coding server
After=network.target

[Service]
ExecStart=/usr/local/bin/nerve --daemon
Restart=always
Environment=HOME=/root
# Optional: Environment=ANTHROPIC_API_KEY=sk-...

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload && systemctl enable --now nerve
```

**Harden it** (recommended): SSH key auth only, `PasswordAuthentication no`, a
firewall allowing just SSH. The daemon needs no inbound ports of its own.

---

## Everyday use — from the TUI (recommended)

In your project directory, launch `nerve` and:

```
/server root@your-server        # connect (saved to config; reconnects next run)
/server                         # show the server's full queue
/server submit <what to do>     # sync THIS project to the server and queue it
/server status                  # refresh the indicator
/server off                     # disconnect
```

`/server submit` rsyncs your whole project to the server (including `.git` and
`.nerve/` project memory, excluding `node_modules`/`target`/`.next`…), then
queues a job against that synced copy. The status bar shows a live **`⛁`
indicator** — e.g. `⛁ 2 running · 5 queued` — so you can watch progress.

## …or from the command line

Because the client runs on the server over SSH, plain SSH works too:

```bash
ssh your-server 'cd /path/to/repo && nerve --submit "add rate limiting"'
ssh your-server 'nerve --jobs'                 # list the queue
ssh your-server 'nerve --jobs --json'          # machine-readable
ssh your-server 'nerve --cancel-job <id>'
ssh your-server 'nerve --query "STATUS <id>"'  # detail incl. error/branch
```

## Reviewing results

Each finished job leaves a `nerve/job-<id>` branch on the server's copy of the
repo (under `~/nerve-repos/<name>` when scheduled via `/server submit`). Review
and pull it:

```bash
ssh your-server 'cd ~/nerve-repos/<name> && git log --oneline nerve/job-<id>'
git fetch ssh://your-server/root/nerve-repos/<name> nerve/job-<id>
```

---

## Carrying your conversation (nothing lost)

`--submit --with-session` attaches your full last conversation to the job, so the
server has everything you had, not just a one-line prompt. It's stored durably
next to the job as `<id>.context.json`; `STATUS` shows `context: attached`.

## Providers

The worker builds its provider from the server's config on every run, so adding
credentials takes effect without a restart. Supported: **Claude Code** CLI
(`claude login` or `ANTHROPIC_API_KEY`), and any OpenAI-compatible endpoint —
**OpenAI**, **OpenRouter**, **Ollama**, or a custom base URL — by setting the
key in the server's nerve config. No code changes to switch.

## What is persisted, and where (durability)

Everything nerve writes is saved atomically (write-temp-then-rename):

- **Job queue** → `~/.nerve/queue/` (one JSON per job + optional
  `<id>.context.json`). Survives restarts and disconnects.
- **Conversations / sessions** → `~/Library/Application Support/nerve/sessions/`
  (Linux: `~/.local/share/nerve/sessions/`) — full history, auto-saved each turn.
- **Per-project memory** → each repo's `.nerve/`: `brief.md`, `decisions.jsonl`,
  `journal.jsonl` (every change), `activity.jsonl`, `memory.md`. This is your
  complete record of every decision and change, and it travels with the project
  on every `/server submit`.

---

## Status

**Works today, verified end-to-end:** the daemon + worker (drains the queue and
runs jobs to completion on isolated branches), `/server` connect + live queue
indicator, `/server submit` project sync, submit/list/cancel over SSH, durable
context bundles, and all providers above.

**Rougher edges / next:** an attached session isn't yet used to *resume* a job
(the worker runs the prompt against the synced project); pulling result branches
back to the laptop is manual (`git fetch` as above); the `⛁` indicator refreshes
on `/server` rather than polling in the background.
