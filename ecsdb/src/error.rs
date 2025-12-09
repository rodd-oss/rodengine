use thiserror::Error;

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
}

pub type Result<T> = std::result::Result<T, EcsDbError>;
