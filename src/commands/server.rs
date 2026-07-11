//! `/server` — connect the TUI to a remote nerve server (24/7 daemon) over SSH
//! and show its live job queue. The transport is plain `ssh` (see
//! [`crate::remote`]): no new port, uses the user's existing SSH trust.

use crate::app::App;

/// Handle the `/server` command family. Returns `true` (always recognised).
///
/// - `/server` — show the connected server's full queue.
/// - `/server <ssh-host>` — connect to that host (persisted to config).
/// - `/server status` — refresh the status indicator.
/// - `/server off` — disconnect.
pub fn handle(app: &mut App, args: &str) -> bool {
    let args = args.trim();
    // `/server submit <prompt>` — sync this project to the server and queue it.
    if args == "submit" {
        app.set_status("Usage: /server submit <prompt>");
        return true;
    }
    if let Some(prompt) = args.strip_prefix("submit ") {
        submit_to_server(app, prompt.trim());
        return true;
    }
    match args {
        "" => show_queue(app),
        "off" | "disconnect" | "none" => {
            app.remote_server = None;
            app.remote_status = None;
            persist(app);
            app.set_status("Disconnected from the remote server");
        }
        "status" | "refresh" => refresh(app),
        host => {
            app.remote_server = Some(host.to_string());
            persist(app);
            app.set_status(format!("Connecting to {host}\u{2026}"));
            refresh(app);
        }
    }
    true
}

/// Sync the current project to the connected server and queue a job for it —
/// the "schedule it on the server" path. Blocking (rsync + ssh), user-invoked.
///
/// A leading `--workflow` runs the multi-agent pipeline (planner → coder →
/// reviewer) instead of a single agent: `/server submit --workflow <prompt>`.
fn submit_to_server(app: &mut App, args: &str) {
    let Some(host) = app.remote_server.clone() else {
        app.add_assistant_message(
            "No remote server connected. Use `/server <ssh-host>` first, then \
             `/server submit [--workflow] <prompt>`."
                .to_string(),
        );
        return;
    };
    let (workflow, prompt) = match args.strip_prefix("--workflow") {
        Some(rest) => (true, rest.trim()),
        None => (false, args),
    };
    if prompt.is_empty() {
        app.set_status("Usage: /server submit [--workflow] <prompt>");
        return;
    }
    let repo = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            app.set_status(format!("Could not read the current directory: {e}"));
            return;
        }
    };
    match crate::remote::sync_and_submit(&host, &repo, prompt, workflow) {
        Ok(msg) => {
            app.add_assistant_message(msg);
            refresh(app);
        }
        Err(e) => app.add_assistant_message(format!("Could not schedule on {host}: {e}")),
    }
}

/// Refresh the cached queue status shown in the status bar. Blocking (a single
/// user-invoked SSH round-trip); shows a clear message on failure.
fn refresh(app: &mut App) {
    let Some(host) = app.remote_server.clone() else {
        app.set_status("No remote server set \u{2014} use /server <ssh-host>");
        return;
    };
    match crate::remote::fetch_status(&host) {
        Ok(status) => {
            let badge = status.badge();
            app.remote_status = Some(status);
            app.set_status(format!("{host}: {badge}"));
        }
        Err(e) => {
            app.remote_status = None;
            app.set_status(format!("Server '{host}' unreachable \u{2014} {e}"));
        }
    }
}

/// Print the remote server's full queue as an assistant message.
fn show_queue(app: &mut App) {
    let Some(host) = app.remote_server.clone() else {
        app.add_assistant_message(
            "No remote server connected.\n\n\
             Connect one with `/server <ssh-host>` (e.g. `/server nerve-server` or \
             `/server root@1.2.3.4`). The host must be reachable over SSH and have \
             `nerve` installed. Once connected, the status bar shows its live queue \
             and you can watch jobs run 24/7."
                .to_string(),
        );
        return;
    };
    match crate::remote::fetch_jobs(&host) {
        Ok(jobs) => {
            let status = crate::remote::summarize(&jobs);
            let mut msg = format!("Remote server \u{2014} {host}\n  {}\n\n", status.badge());
            if jobs.is_empty() {
                msg.push_str("The queue is empty.");
            } else {
                for job in &jobs {
                    msg.push_str(&job.summary_line());
                    msg.push('\n');
                }
            }
            app.remote_status = Some(status);
            app.add_assistant_message(msg);
        }
        Err(e) => {
            app.remote_status = None;
            app.add_assistant_message(format!("Remote server '{host}' unreachable \u{2014} {e}"));
        }
    }
}

/// Persist the connected server to the on-disk config so it reconnects next run.
/// Best-effort: a save failure is surfaced but never crashes the command.
fn persist(app: &App) {
    match crate::config::Config::load() {
        Ok(mut config) => {
            config.remote_server = app.remote_server.clone();
            if let Err(e) = config.save() {
                tracing::warn!("could not persist remote_server: {e}");
            }
        }
        Err(e) => tracing::warn!("could not load config to persist remote_server: {e}"),
    }
}
