//! Database error types.

use thiserror::Error;

/// Database operation errors.
#[derive(Error, Debug, Clone)]
pub enum DbError {
    /// Table not found
    #[error("Table '{table}' not found")]
    TableNotFound { table: String },

    /// Field not found in table
    #[error("Field '{field}' not found in table '{table}'")]
    FieldNotFound { table: String, field: String },

    /// Field already exists in table
    #[error("Field '{field}' already exists in table '{table}'")]
    FieldAlreadyExists { table: String, field: String },

    /// Field exceeds record size boundaries
    #[error("Field '{field}' (offset={offset}, size={size}) exceeds record size {record_size}")]
    FieldExceedsRecordSize {
        field: String,
        offset: usize,
        size: usize,
        record_size: usize,
    },

    /// Capacity calculation overflow
    #[error("Capacity overflow during {operation}")]
    CapacityOverflow { operation: &'static str },

    /// Type mismatch error
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    /// Invalid offset access
    #[error("Invalid offset {offset} for table '{table}' (max: {max})")]
    InvalidOffset {
        table: String,
        offset: usize,
        max: usize,
    },

    /// Transaction conflict
    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Procedure panic
    #[error("Procedure panic: {0}")]
    ProcedurePanic(String),

    /// Lock poisoned (RwLock poisoned)
    #[error("Lock poisoned")]
    LockPoisoned,

    /// Table already exists
    #[error("Table '{0}' already exists")]
    TableAlreadyExists(String),

    /// Record not found
    #[error("Record not found at index {index} in table '{table}'")]
    RecordNotFound { table: String, index: usize },

    /// Operation timeout
    #[error("Operation timeout")]
    Timeout,

    /// Data corruption detected
    #[error("Data corruption detected: {0}")]
    DataCorruption(String),

    /// Memory limit exceeded for buffer growth
    #[error("Memory limit exceeded for table '{table}': requested {requested} bytes, limit {limit} bytes")]
    MemoryLimitExceeded {
        requested: usize,
        limit: usize,
        table: String,
    },

    /// Disk full error during persistence
    #[error("Disk full: {0}")]
    DiskFull(String),

    /// I/O error during persistence
    #[error("I/O error: {0}")]
    IoError(String),

    /// Transient I/O error that may succeed on retry
    #[error("Transient I/O error: {0}")]
    TransientIoError(String),
}
