use hyprcube_core::ipc::HyprlandIpc;
use hyprcube_core::undo::{UndoEntry, UndoStack};

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("IPC not connected")]
    NotConnected,
    #[error("IPC error: {0}")]
    Ipc(#[from] hyprcube_core::ipc::IpcError),
}

/// Orchestrates live preview via Hyprland IPC and undo support.
pub struct PreviewEngine {
    ipc: Option<HyprlandIpc>,
    undo_stack: UndoStack,
    runtime: tokio::runtime::Runtime,
}

impl PreviewEngine {
    /// Try to connect to Hyprland IPC. Continues without it if unavailable.
    pub fn new() -> Self {
        let ipc = match HyprlandIpc::connect() {
            Ok(ipc) => {
                tracing::info!("connected to Hyprland IPC");
                Some(ipc)
            }
            Err(e) => {
                tracing::info!("Hyprland IPC not available: {e}");
                None
            }
        };

        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime");

        Self {
            ipc,
            undo_stack: UndoStack::new(),
            runtime,
        }
    }

    /// Set a Hyprland keyword via IPC and push the old value onto the undo
    /// stack.
    pub fn apply_preview(&mut self, key: &str, value: &str) -> Result<(), PreviewError> {
        let ipc = self.ipc.as_ref().ok_or(PreviewError::NotConnected)?;

        // Query the current value so we can undo later.
        let old_value = self.runtime.block_on(async {
            match ipc.get_option(key).await {
                Ok(json) => {
                    // Hyprland returns option data with str/int/float fields.
                    if let Some(s) = json.get("str").and_then(|v| v.as_str()) {
                        s.to_string()
                    } else if let Some(i) = json.get("int").and_then(|v| v.as_i64()) {
                        i.to_string()
                    } else if let Some(f) = json.get("float").and_then(|v| v.as_f64()) {
                        f.to_string()
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(),
            }
        });

        // Apply the new value.
        self.runtime.block_on(ipc.set_keyword(key, value))?;

        self.undo_stack.push(UndoEntry {
            key: key.to_string(),
            old_value,
        });

        Ok(())
    }

    /// Revert all previewed changes by rolling back the undo stack via IPC.
    pub fn revert_all(&mut self) -> Result<(), PreviewError> {
        let ipc = self.ipc.as_ref().ok_or(PreviewError::NotConnected)?;

        // Collect entries first to avoid borrow conflict.
        let entries: Vec<_> = self
            .undo_stack
            .rollback_iter()
            .map(|e| (e.key.clone(), e.old_value.clone()))
            .collect();

        for (key, old_value) in entries {
            self.runtime.block_on(ipc.set_keyword(&key, &old_value))?;
        }

        self.undo_stack.clear();
        Ok(())
    }

    /// Clear the undo stack (called after saving to disk).
    pub fn commit(&mut self) {
        self.undo_stack.clear();
    }

    /// Returns `true` if there are un-committed preview changes.
    pub fn is_dirty(&self) -> bool {
        self.undo_stack.is_dirty()
    }
}
