/// A single undoable change.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub key: String,
    pub old_value: String,
}

/// A stack of undoable changes for live-preview revert.
#[derive(Debug, Default)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new entry onto the stack.
    pub fn push(&mut self, entry: UndoEntry) {
        self.entries.push(entry);
    }

    /// Iterate entries in reverse (most recent first) for rollback.
    pub fn rollback_iter(&self) -> impl Iterator<Item = &UndoEntry> {
        self.entries.iter().rev()
    }

    /// Discard all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns `true` if there are pending changes.
    pub fn is_dirty(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Number of entries in the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
