/// Error type for type registration and validation.
#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error("Type '{type_id}' has invalid size: {size}")]
    InvalidSize { type_id: String, size: usize },

    #[error("Type '{type_id}' has invalid alignment: {align}")]
    InvalidAlignment { type_id: String, align: usize },

    #[error("Type '{type_id}' size {size} not divisible by alignment {align}")]
    SizeAlignmentMismatch {
        type_id: String,
        size: usize,
        align: usize,
    },

    #[error("POD type '{type_id}' missing internal TypeId")]
    MissingTypeId { type_id: String },

    #[error("Type '{type_id}' already registered")]
    AlreadyRegistered { type_id: String },

    #[error("Type '{type_id}' not found")]
    NotFound { type_id: String },

    #[error("Type validation failed: {message}")]
    ValidationFailed { type_id: String, message: String },
}
