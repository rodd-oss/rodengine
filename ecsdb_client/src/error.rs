use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Sync error: {0}")]
    SyncError(String),

    #[error("Component not found for entity {entity_id}: {component_type}")]
    ComponentNotFound {
        entity_id: u64,
        component_type: String,
    },

    #[error("Entity not found: {0}")]
    EntityNotFound(u64),

    #[error("Schema mismatch: {0}")]
    SchemaMismatch(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;
