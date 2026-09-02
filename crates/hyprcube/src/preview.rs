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
    /// Try to connect to Hyprland IPC.  Continues without it if unavailable.
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

        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

        Self {
            ipc,
            undo_stack: UndoStack::new(),
            runtime,
        }
    }

    /// Apply a live preview via IPC and push the old value onto the undo stack.
    ///
    /// - `section` — Hyprland config section, e.g. `Some("general")` or
    ///   `Some("input:touchpad")`.  Combined with `key` to form the IPC keyword
    ///   path (`section:key`).  Pass `None` for palette / non-Hyprland fields.
    /// - `old_value` — the previous value returned by `panel.set_value()`.
    ///
    /// IPC errors are logged but do not propagate; a missing IPC connection is
    /// silently skipped so the app remains usable outside Hyprland.
    pub fn apply_preview(
        &mut self,
        section: Option<&str>,
        key: &str,
        value: &str,
        old_value: &str,
    ) {
        let ipc_key = match section {
            Some(s) => format!("{s}:{key}"),
            None => return, // No IPC keyword for palette/non-Hyprland fields.
        };

        if let Some(ipc) = &self.ipc {
            if let Err(e) = self.runtime.block_on(ipc.set_keyword(&ipc_key, value)) {
                tracing::warn!("IPC preview failed for {ipc_key}={value}: {e}");
            }
        }

        self.undo_stack.push(UndoEntry {
            key: ipc_key,
            old_value: old_value.to_string(),
        });
    }

    /// Revert all previewed changes by replaying the undo stack in reverse
    /// via IPC.
    pub fn revert_all(&mut self) -> Result<(), PreviewError> {
        let entries: Vec<_> = self
            .undo_stack
            .rollback_iter()
            .map(|e| (e.key.clone(), e.old_value.clone()))
            .collect();

        if let Some(ipc) = &self.ipc {
            for (key, old_value) in &entries {
                if let Err(e) = self.runtime.block_on(ipc.set_keyword(key, old_value)) {
                    tracing::warn!("IPC revert failed for {key}: {e}");
                }
            }
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

    /// Tell Hyprland to reload its config from disk.
    pub fn reload_hyprland(&mut self) -> Result<(), PreviewError> {
        let ipc = self.ipc.as_ref().ok_or(PreviewError::NotConnected)?;
        self.runtime.block_on(ipc.reload())?;
        Ok(())
    }

    /// Query Hyprland for config parse errors.  Returns the first error string
    /// if any exist, or `None` on success / no IPC.
    pub fn check_config_errors(&mut self) -> Option<String> {
        let ipc = self.ipc.as_ref()?;
        match self.runtime.block_on(ipc.config_errors()) {
            Ok(json) => {
                if let Some(arr) = json.as_array() {
                    if !arr.is_empty() {
                        let msgs: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect();
                        return Some(msgs.join("; "));
                    }
                }
                None
            }
            Err(_) => None,
        }
    }
}
