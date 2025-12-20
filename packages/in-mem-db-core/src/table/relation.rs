//! Relation between tables for foreign key references.

/// Relation between tables for foreign key references.
#[derive(Debug, Clone)]
pub struct Relation {
    /// Name of the target table
    pub to_table: String,
    /// Field name in source table
    pub from_field: String,
    /// Field name in target table
    pub to_field: String,
}
