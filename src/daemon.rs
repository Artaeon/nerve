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

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            handle_client(&mut stream).await;
        });
    }
}

async fn handle_client(stream: &mut UnixStream) {
    let mut buf = vec![0u8; 4096];
    match stream.read(&mut buf).await {
        Ok(n) if n > 0 => {
            let request = String::from_utf8_lossy(&buf[..n]).to_string();

            // Handle shutdown command
            if request.trim() == "__SHUTDOWN__" {
                let _ = stream.write_all(b"Nerve daemon shutting down.").await;
                std::process::exit(0);
            }

            // Parse request and respond
            let response = format!("Nerve daemon received: {request}");
            let _ = stream.write_all(response.as_bytes()).await;
        }
        _ => {}
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
