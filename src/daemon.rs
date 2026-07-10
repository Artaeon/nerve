use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Path to the daemon's control socket.
///
/// Two requirements: (1) it must NOT live in a world-writable shared root like
/// `/tmp` (any local user could then connect and send `__SHUTDOWN__`), and
/// (2) the daemon and client must compute the SAME path regardless of how each
/// was launched. `$XDG_RUNTIME_DIR` and `$TMPDIR` satisfy (1) but not (2): a
/// systemd/cron-launched daemon and a login-shell client can see different
/// values, so they'd bind/dial different sockets. So we anchor to the user's
/// HOME (stable across launch contexts) under a dedicated `~/.nerve` dir that
/// `start_daemon` locks to 0700, with the socket itself at 0600.
pub fn socket_path() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    base.join(".nerve").join("nerve.sock")
}

#[allow(dead_code)]
pub fn is_daemon_running() -> bool {
    socket_path().exists() && std::os::unix::net::UnixStream::connect(socket_path()).is_ok()
}

pub async fn start_daemon() -> anyhow::Result<()> {
    let path = socket_path();

    // Ensure the parent directory exists and is private to this user (0700).
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }

    // Remove stale socket
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
    // Restrict the socket to the owning user so no other local account can send
    // control commands like __SHUTDOWN__.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    println!("Nerve daemon listening on {}", path.display());

    // Spawn the queue worker: it drains submitted jobs and runs them to
    // completion (headless agent, isolated on a git branch, committed for
    // review). Runs for the life of the daemon, independent of client I/O.
    tokio::spawn(async {
        crate::worker::run_worker().await;
    });

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            handle_client(&mut stream).await;
        });
    }
}

async fn handle_client(stream: &mut UnixStream) {
    // Read the whole request; the client half-closes its write side (see
    // `send_to_daemon`), which gives us EOF. Reading to end (rather than a fixed
    // 4 KiB) means long prompts submitted via SUBMIT aren't truncated.
    let mut buf = Vec::new();
    if stream.read_to_end(&mut buf).await.is_err() {
        return;
    }
    let request = String::from_utf8_lossy(&buf);
    let request = request.trim_end_matches(['\n', '\r']);

    // Control command: shut the whole daemon down. Remove our own socket first
    // so a stale `nerve.sock` isn't left behind for the next start to trip over.
    if request.trim() == "__SHUTDOWN__" {
        let _ = stream.write_all(b"Nerve daemon shutting down.").await;
        let _ = std::fs::remove_file(socket_path());
        std::process::exit(0);
    }

    let queue = crate::queue::Queue::default_location();
    let response = process_command(request, &queue);
    let _ = stream.write_all(response.as_bytes()).await;
}

/// Handle one client request against the job queue and return the text reply.
///
/// The wire format is deliberately dumb: a command word, then tab-separated
/// arguments. `SUBMIT`'s prompt is everything after the second tab, so it may
/// contain tabs and newlines. Kept pure (queue passed in) so it is unit-tested
/// without a running daemon.
pub(crate) fn process_command(request: &str, queue: &crate::queue::Queue) -> String {
    let mut parts = request.splitn(3, '\t');
    let cmd = parts.next().unwrap_or("").trim();

    match cmd {
        "PING" => "PONG".to_string(),
        "SUBMIT" => {
            let repo = parts.next().unwrap_or("").trim();
            let prompt = parts.next().unwrap_or("").trim();
            if repo.is_empty() || prompt.is_empty() {
                return "ERR usage: SUBMIT <repo>\\t<prompt>".to_string();
            }
            match queue.enqueue(repo, prompt) {
                Ok(job) => format!(
                    "OK queued job {} on branch {}",
                    job.id,
                    job.branch.as_deref().unwrap_or("-")
                ),
                Err(e) => format!("ERR could not queue job: {e}"),
            }
        }
        "LIST" => match queue.list() {
            Ok(jobs) if jobs.is_empty() => "No jobs in the queue.".to_string(),
            Ok(jobs) => {
                let mut out = format!("{} job(s):\n", jobs.len());
                for job in jobs {
                    out.push_str(&job.summary_line());
                    out.push('\n');
                }
                out
            }
            Err(e) => format!("ERR could not list jobs: {e}"),
        },
        // Machine-readable listing for remote clients (the TUI polls this over
        // SSH to render the live queue indicator). Returns a JSON array of jobs.
        "LISTJSON" => match queue.list() {
            Ok(jobs) => serde_json::to_string(&jobs)
                .unwrap_or_else(|e| format!("ERR could not serialize jobs: {e}")),
            Err(e) => format!("ERR could not list jobs: {e}"),
        },
        "ATTACH" => {
            // ATTACH <id>\t<context...> — the context (a session-JSON snapshot)
            // is the last field so it may contain anything. Kept a separate
            // message from SUBMIT so that command's free-text prompt can still
            // safely contain tabs.
            let id = parts.next().unwrap_or("").trim().to_string();
            let context = parts.next().unwrap_or("");
            if id.is_empty() || context.is_empty() {
                return "ERR usage: ATTACH <id>\\t<context>".to_string();
            }
            if matches!(queue.get(&id), Ok(None)) {
                return format!("No job with id {id}");
            }
            match queue.save_context(&id, context) {
                Ok(()) => format!("OK attached context to job {id}"),
                Err(e) => format!("ERR could not attach context: {e}"),
            }
        }
        "STATUS" => {
            let id = parts.next().unwrap_or("").trim();
            match queue.get(id) {
                Ok(Some(job)) => {
                    let mut out = format!(
                        "job {}\n  status:  {}\n  repo:    {}\n  branch:  {}\n  context: {}\n",
                        job.id,
                        job.status.label(),
                        job.repo,
                        job.branch.as_deref().unwrap_or("-"),
                        if job.has_context {
                            "attached (full session carried over)"
                        } else {
                            "none"
                        },
                    );
                    if let Some(err) = &job.error {
                        out.push_str(&format!("  error:   {err}\n"));
                    }
                    out.push_str(&format!("  prompt:  {}\n", job.prompt));
                    out
                }
                Ok(None) => format!("No job with id {id}"),
                Err(e) => format!("ERR could not read job: {e}"),
            }
        }
        "CANCEL" => {
            let id = parts.next().unwrap_or("").trim();
            match queue.cancel(id) {
                Ok(true) => format!("Cancelled job {id}"),
                Ok(false) => {
                    format!("Job {id} is not cancellable (unknown id or already started)")
                }
                Err(e) => format!("ERR could not cancel job: {e}"),
            }
        }
        "" => "ERR empty request".to_string(),
        other => format!("ERR unknown command: {other}"),
    }
}

pub async fn send_to_daemon(message: &str) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(socket_path()).await?;
    stream.write_all(message.as_bytes()).await?;
    stream.shutdown().await?;

    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

pub fn stop_daemon() -> anyhow::Result<()> {
    let path = socket_path();
    if path.exists() {
        // Send shutdown command
        if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) {
            use std::io::Write;
            let _ = stream.write_all(b"__SHUTDOWN__");
        }
        // Give the daemon a moment, then clean up the socket file if it remains
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_queue() -> (tempfile::TempDir, crate::queue::Queue) {
        let dir = tempfile::tempdir().unwrap();
        let q = crate::queue::Queue::new(dir.path().join("queue"));
        (dir, q)
    }

    #[test]
    fn process_ping() {
        let (_d, q) = temp_queue();
        assert_eq!(process_command("PING", &q), "PONG");
    }

    #[test]
    fn process_submit_queues_a_job() {
        let (_d, q) = temp_queue();
        let reply = process_command("SUBMIT\t/srv/repo\tadd rate limiting", &q);
        assert!(reply.starts_with("OK queued job "), "got: {reply}");
        let jobs = q.list().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].repo, "/srv/repo");
        assert_eq!(jobs[0].prompt, "add rate limiting");
    }

    #[test]
    fn process_submit_keeps_tabs_and_newlines_in_prompt() {
        let (_d, q) = temp_queue();
        process_command("SUBMIT\t/r\tline1\nline2\twith tab", &q);
        let jobs = q.list().unwrap();
        assert_eq!(jobs[0].prompt, "line1\nline2\twith tab");
    }

    #[test]
    fn process_submit_rejects_missing_args() {
        let (_d, q) = temp_queue();
        assert!(process_command("SUBMIT", &q).starts_with("ERR"));
        assert!(process_command("SUBMIT\t/r\t", &q).starts_with("ERR"));
        assert!(process_command("SUBMIT\t\tprompt", &q).starts_with("ERR"));
        assert!(q.list().unwrap().is_empty());
    }

    #[test]
    fn process_list_empty_then_populated() {
        let (_d, q) = temp_queue();
        assert_eq!(process_command("LIST", &q), "No jobs in the queue.");
        process_command("SUBMIT\t/r\tdo a thing", &q);
        let reply = process_command("LIST", &q);
        assert!(reply.contains("1 job(s):"));
        assert!(reply.contains("do a thing"));
    }

    #[test]
    fn process_status_and_cancel() {
        let (_d, q) = temp_queue();
        let job = q.enqueue("/r", "x").unwrap();
        let status = process_command(&format!("STATUS\t{}", job.id), &q);
        assert!(status.contains(&job.id));
        assert!(status.contains("queued"));

        let cancel = process_command(&format!("CANCEL\t{}", job.id), &q);
        assert!(cancel.contains("Cancelled"));
        assert_eq!(
            q.get(&job.id).unwrap().unwrap().status,
            crate::queue::JobStatus::Cancelled
        );
    }

    #[test]
    fn process_status_unknown_id() {
        let (_d, q) = temp_queue();
        assert!(process_command("STATUS\tnope", &q).contains("No job"));
    }

    #[test]
    fn process_attach_stores_context_and_status_shows_it() {
        let (_d, q) = temp_queue();
        let job = q.enqueue("/r", "resume this").unwrap();
        let ctx = r#"{"conversations":[{"messages":[["user","earlier work"]]}]}"#;
        let reply = process_command(&format!("ATTACH\t{}\t{ctx}", job.id), &q);
        assert!(reply.starts_with("OK attached context"), "got: {reply}");
        assert_eq!(q.load_context(&job.id).unwrap().as_deref(), Some(ctx));
        let status = process_command(&format!("STATUS\t{}", job.id), &q);
        assert!(status.contains("attached"), "status: {status}");
    }

    #[test]
    fn process_attach_rejects_unknown_job_and_missing_args() {
        let (_d, q) = temp_queue();
        assert!(process_command("ATTACH\tghost\tctx", &q).contains("No job"));
        assert!(process_command("ATTACH", &q).starts_with("ERR"));
        assert!(process_command("ATTACH\tid-only", &q).starts_with("ERR"));
    }

    #[test]
    fn process_unknown_command() {
        let (_d, q) = temp_queue();
        assert!(process_command("FROBNICATE", &q).starts_with("ERR unknown command"));
        assert!(process_command("", &q).starts_with("ERR"));
    }

    #[test]
    fn socket_path_is_in_tmp() {
        let path = socket_path();
        assert!(path.to_string_lossy().contains("nerve"));
    }

    #[test]
    fn is_daemon_running_false_when_no_socket() {
        // Clean up any existing socket first
        let path = socket_path();
        let _ = std::fs::remove_file(&path);
        assert!(!is_daemon_running());
    }

    #[test]
    fn stop_daemon_no_panic_when_not_running() {
        let _ = std::fs::remove_file(socket_path());
        // Should not panic even if daemon isn't running
        let result = stop_daemon();
        assert!(result.is_ok());
    }

    #[test]
    fn socket_path_is_absolute() {
        let path = socket_path();
        assert!(path.is_absolute());
    }

    #[test]
    fn socket_is_not_in_world_shared_root() {
        // The socket must live in a dedicated per-user directory, never
        // directly in a world-writable shared root like /tmp, so another local
        // user can't send it control commands.
        let path = socket_path();
        let parent = path.parent().expect("socket has a parent dir");
        assert_ne!(parent, std::path::Path::new("/tmp"));
        assert_ne!(parent, std::env::temp_dir());
    }

    #[test]
    fn socket_path_has_sock_extension() {
        let path = socket_path();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("sock"));
    }
}
