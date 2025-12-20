//! Schema persistence, data file flush/load, and recovery.

#[cfg(test)]
mod test;

mod io_utils;
mod persistence_manager;
mod schema;
mod schema_validation;

// Re-export public items
pub use io_utils::{classify_io_error, retry_io_operation};
pub use persistence_manager::PersistenceManager;
pub use schema::{CustomTypeSchema, FieldSchema, RelationSchema, SchemaFile, TableSchema};
pub use schema_validation::validate_schema;

use crate::config::DbConfig;
use crate::database::Database;
use crate::error::DbError;

/// Helper function to save schema after DDL operations.
pub fn save_schema_after_ddl(db: &Database, config: &DbConfig) -> Result<(), DbError> {
    let persistence = PersistenceManager::new(config);
    persistence.save_schema(db)
}
