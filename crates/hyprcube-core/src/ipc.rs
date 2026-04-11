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
