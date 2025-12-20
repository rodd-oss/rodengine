//! Schema structs for persistence.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Schema file format for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaFile {
    /// Schema version
    pub version: u32,
    /// Table definitions
    pub tables: HashMap<String, TableSchema>,
    /// Custom type definitions
    pub custom_types: HashMap<String, CustomTypeSchema>,
    /// Data file checksums for corruption detection
    #[serde(default)]
    pub checksums: HashMap<String, u32>,
}

/// Table schema for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct TableSchema {
    /// Record size in bytes
    pub record_size: usize,
    /// Field definitions
    pub fields: Vec<FieldSchema>,
    /// Foreign key relations
    pub relations: Vec<RelationSchema>,
}

/// Field schema for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct FieldSchema {
    /// Field name
    pub name: String,
    /// Type identifier
    pub r#type: String,
    /// Byte offset within record
    pub offset: usize,
}

/// Relation schema for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelationSchema {
    /// Target table name
    pub to_table: String,
    /// Source field name
    pub from_field: String,
    /// Target field name
    pub to_field: String,
}

/// Custom type schema for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct CustomTypeSchema {
    /// Size in bytes
    pub size: usize,
    /// Alignment requirement
    pub align: usize,
    /// Plain old data flag
    pub pod: bool,
}
