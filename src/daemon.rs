use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

const SOCKET_PATH: &str = "/tmp/nerve.sock";

pub fn socket_path() -> PathBuf {
    PathBuf::from(SOCKET_PATH)
}

pub fn is_daemon_running() -> bool {
    socket_path().exists() && std::os::unix::net::UnixStream::connect(socket_path()).is_ok()
}

pub async fn start_daemon() -> anyhow::Result<()> {
    // Remove stale socket
    let path = socket_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = UnixListener::bind(&path)?;
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
            let response = format!("Nerve daemon received: {}", request);
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
