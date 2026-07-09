# Nerve server & client — 24/7 coding

One `nerve` binary, two roles. Run it as a **server** on an always-on machine and
submit coding jobs to it from your **laptop**; close the laptop and the server
keeps its queue. This document is the honest state of the feature — what works
today and what is still being built.

## The model

- **Server**: a machine that has `nerve`, the Claude Code CLI (logged in), your
  repos, and each project's environment set up. It runs `nerve --daemon` and
  keeps a persistent job queue in `~/.nerve/queue/`.
- **Client**: your laptop. You submit jobs to the server, list them, and cancel
  them. When you close the laptop, the server's queue is untouched.
- **Transport = SSH.** There is **no new network port**. The daemon listens only
  on a HOME-anchored Unix socket (`~/.nerve/nerve.sock`, dir `0700`, socket
  `0600` — owner-only). You reach it by running the client *on the server over
  SSH*, which you already trust. Simple, and nothing new is exposed to the
  internet.

## Server setup

```bash
# on the server, as your user:
cargo install --path .          # or copy the built `nerve` binary onto PATH
claude login                    # the Claude Code CLI must be authenticated
nerve --daemon                  # start the server (see "keep it running" below)
```

Keep it running 24/7 with your init system. Example systemd unit
(`~/.config/systemd/user/nerve.service`, then `systemctl --user enable --now nerve`):

```ini
[Unit]
Description=Nerve coding server
[Service]
ExecStart=%h/.cargo/bin/nerve --daemon
Restart=always
[Install]
WantedBy=default.target
```

macOS servers: enable **Remote Login** (System Settings → General → Sharing) so
the client can SSH in, and run the daemon from a `launchd` agent.

## Using it from your laptop

Because the socket is owner-only, the client runs *on the server* over SSH — same
user, same HOME, same socket:

```bash
# submit a job; the repo is the directory you're in ON THE SERVER
ssh myserver 'cd /srv/repos/api && nerve --submit "add rate limiting to the login route"'

ssh myserver 'nerve --jobs'                 # list the queue
ssh myserver 'nerve --cancel-job <id>'      # cancel a queued job
```

Each job runs on its own branch `nerve/job-<id>` and is committed there for you
to review — never straight to `main`. That is the guardrail: you pull the branch
and merge if you like it.

## Carrying your context (nothing lost)

`--with-session` attaches your full last conversation (the session JSON) to the
job, so the server resumes with everything you had rather than a bare prompt:

```bash
nerve --submit "continue the auth refactor" --with-session
```

The session is stored durably next to the job as `<id>.context.json`, and
`nerve --query "STATUS <id>"` shows `context: attached`. Note: `--with-session`
reads the session of *whichever machine runs the client*. To hand off your
laptop's exact context you currently run the client against a forwarded socket
(`ssh -L`) or sync your session file — first-class laptop→server context sync is
the next increment (see below).

## What is persisted, and where (durability)

Everything nerve writes is saved atomically (write-temp-then-rename), so a crash
never leaves a half-written file:

- **Conversations / sessions** → `~/Library/Application Support/nerve/sessions/`
  (Linux: `~/.local/share/nerve/sessions/`). Full message history, auto-saved
  every turn, last-session + 25 named snapshots.
- **The job queue** → `~/.nerve/queue/` (one JSON per job + optional
  `<id>.context.json`). Survives restarts.
- **Per-project memory** → each repo's `.nerve/`: `brief.md`, `decisions.jsonl`,
  `journal.jsonl` (every change), `activity.jsonl`, `memory.md`. This is your
  "complete database of every decision and change." **Commit `.nerve/` to git**
  and it travels with the repo — git is the durable, synced history that both
  the laptop and server share (push on one, pull on the other).

## Status — what works today vs. next

**Works now:** the server (`--daemon`), the persistent queue, submit / list /
cancel over SSH, job branches, and durable context bundles attached to jobs.
Verified end-to-end (submit + attach a real session + status + persistence).

**Next increment (in progress):** the **worker** that drains the queue and
actually *runs* each job (checkout branch → run the agent to completion → verify
→ commit). It needs a headless agent runner extracted from the TUI event loop.
After that: first-class laptop→server sync of session + `.nerve/` state so the
handoff is fully automatic, and optional live attach to watch a running job.
