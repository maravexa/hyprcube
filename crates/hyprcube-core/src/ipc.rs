use std::path::PathBuf;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("required environment variable not set: {0}")]
    EnvMissing(String),
    #[error("hyprland socket not found at {0}")]
    SocketNotFound(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Hyprland IPC client communicating over Unix sockets.
pub struct HyprlandIpc {
    socket_path: PathBuf,
}

impl HyprlandIpc {
    /// Create a new IPC client, discovering the socket path from environment variables.
    pub fn connect() -> Result<Self, IpcError> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map_err(|_| IpcError::EnvMissing("XDG_RUNTIME_DIR".into()))?;
        let instance_sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .map_err(|_| IpcError::EnvMissing("HYPRLAND_INSTANCE_SIGNATURE".into()))?;

        let socket_path = PathBuf::from(runtime_dir)
            .join("hypr")
            .join(&instance_sig)
            .join(".socket.sock");

        if !socket_path.exists() {
            return Err(IpcError::SocketNotFound(socket_path));
        }

        Ok(Self { socket_path })
    }

    /// Send a raw command and read the full response.
    async fn request(&self, command: &str) -> Result<String, IpcError> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(command.as_bytes()).await?;
        stream.shutdown().await?;

        let mut buf = String::new();
        stream.read_to_string(&mut buf).await?;
        Ok(buf)
    }

    /// Send a query and parse the JSON response.
    pub async fn query_json(&self, endpoint: &str) -> Result<serde_json::Value, IpcError> {
        let raw = self.request(&format!("j/{endpoint}")).await?;
        let value = serde_json::from_str(&raw)?;
        Ok(value)
    }

    /// Get the value of a Hyprland option.
    pub async fn get_option(&self, name: &str) -> Result<serde_json::Value, IpcError> {
        self.query_json(&format!("getoption {name}")).await
    }

    /// Set a Hyprland keyword.
    pub async fn set_keyword(&self, key: &str, value: &str) -> Result<String, IpcError> {
        self.request(&format!("keyword {key} {value}")).await
    }

    /// Reload the Hyprland configuration.
    pub async fn reload(&self) -> Result<String, IpcError> {
        self.request("reload").await
    }

    /// Dispatch a Hyprland command.
    pub async fn dispatch(&self, cmd: &str) -> Result<String, IpcError> {
        self.request(&format!("dispatch {cmd}")).await
    }

    /// Query monitor information as JSON.
    pub async fn monitors(&self) -> Result<serde_json::Value, IpcError> {
        self.query_json("monitors").await
    }

    /// Query config errors.
    pub async fn config_errors(&self) -> Result<serde_json::Value, IpcError> {
        self.query_json("configerrors").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use tokio::net::UnixListener;
    use tokio::time::timeout;

    struct SocketDir(PathBuf);

    impl SocketDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "hyprcube-ipc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn client(&self) -> HyprlandIpc {
            HyprlandIpc {
                socket_path: self.0.join("socket"),
            }
        }
    }

    impl Drop for SocketDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // A peer waits for write EOF before replying, exercising request half-close.
    async fn serve(listener: &UnixListener, command: &str, chunks: &[&[u8]]) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).await.unwrap();
        assert_eq!(received, command.as_bytes());
        for chunk in chunks {
            stream.write_all(chunk).await.unwrap();
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn requests_half_close_and_read_complete_responses_on_fresh_connections() {
        timeout(Duration::from_secs(5), async {
            let dir = SocketDir::new();
            let client = dir.client();
            let listener = UnixListener::bind(&client.socket_path).unwrap();
            for _ in 0..3 {
                let (result, ()) = tokio::join!(
                    client.monitors(),
                    serve(&listener, "j/monitors", &[b"[{\"name\":", b"\"DP-1\"}]"])
                );
                assert_eq!(result.unwrap()[0]["name"], "DP-1");
            }
        })
        .await
        .expect("mock IPC timed out");
    }

    #[tokio::test]
    async fn ipc_errors_propagate_and_do_not_poison_later_requests() {
        timeout(Duration::from_secs(5), async {
            let dir = SocketDir::new();
            let client = dir.client();
            assert!(matches!(client.monitors().await, Err(IpcError::Io(_))));
            let listener = UnixListener::bind(&client.socket_path).unwrap();
            let (result, ()) = tokio::join!(client.monitors(), serve(&listener, "j/monitors", &[b"{broken"]));
            assert!(matches!(result, Err(IpcError::Json(_))));
            let (result, ()) = tokio::join!(client.monitors(), serve(&listener, "j/monitors", &[&[0xff]]));
            assert!(matches!(result, Err(IpcError::Io(ref e)) if e.kind() == std::io::ErrorKind::InvalidData));
            let (result, ()) = tokio::join!(client.config_errors(), serve(&listener, "j/configerrors", &[b"[]"]));
            assert_eq!(result.unwrap(), serde_json::json!([]));
        }).await.expect("mock IPC timed out");
    }

    #[tokio::test]
    async fn cancelling_a_pending_response_allows_a_new_request() {
        timeout(Duration::from_secs(5), async {
            let dir = SocketDir::new();
            let client = dir.client();
            let listener = UnixListener::bind(&client.socket_path).unwrap();
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let pending = tokio::spawn(async move { client.monitors().await });
            let peer = async {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut command = String::new();
                stream.read_to_string(&mut command).await.unwrap();
                assert_eq!(command, "j/monitors");
                started_tx.send(()).unwrap();
                // Keep the response open until the request future has been dropped.
                std::future::pending::<()>().await;
                drop(stream);
            };
            let cancel = async {
                started_rx.await.unwrap();
                pending.abort();
                assert!(pending.await.unwrap_err().is_cancelled());
            };
            tokio::select! {
                () = peer => panic!("peer unexpectedly completed"),
                () = cancel => {}
            }
            let client = dir.client();
            let (result, ()) =
                tokio::join!(client.monitors(), serve(&listener, "j/monitors", &[b"[]"]));
            assert_eq!(result.unwrap(), serde_json::json!([]));
        })
        .await
        .expect("mock IPC timed out");
    }
}
