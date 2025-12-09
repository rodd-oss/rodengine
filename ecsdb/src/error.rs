use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum EcsDbError {
    #[error("Entity not found: {0}")]
    EntityNotFound(u64),

    #[error("Component not found for entity {entity_id}: {component_type}")]
    ComponentNotFound {
        entity_id: u64,
        component_type: String,
    },

    #[error("Schema validation failed: {0}")]
    SchemaError(String),

    #[error("Referential integrity violation: {0}")]
    ReferentialIntegrityViolation(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    #[error("Field type mismatch: expected {expected}, got {got}")]
    FieldTypeMismatch { expected: String, got: String },

    #[error("Field alignment error at offset {offset}")]
    AlignmentError { offset: usize },

    #[error("Write channel closed")]
    ChannelClosed,

    #[error("Timeout waiting for write confirmation")]
    Timeout,

    #[error("Snapshot error: {0}")]
    SnapshotError(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("WAL error: {0}")]
    WalError(String),

    #[error("Blocking task failed: {0}")]
    BlockingTaskError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Compaction error: {0}")]
    CompactionError(String),
}

impl From<JoinError> for EcsDbError {
    fn from(err: JoinError) -> Self {
        EcsDbError::BlockingTaskError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EcsDbError>;
