/// Represents a single change to a table buffer.
#[derive(Debug, Clone)]
pub enum Change {
    /// Create a new record
    Create {
        /// Byte offset where record should be inserted
        offset: usize,
        /// Serialized record data
        data: Vec<u8>,
    },
    /// Update an existing record
    Update {
        /// Byte offset of the record to update
        offset: usize,
        /// Range of bytes being replaced (for conflict detection)
        old: std::ops::Range<usize>,
        /// New serialized record data
        new: Vec<u8>,
    },
    /// Delete a record
    Delete {
        /// Byte offset of the record to delete
        offset: usize,
        /// Original record data (for conflict detection and rollback)
        original: Vec<u8>,
    },
}
