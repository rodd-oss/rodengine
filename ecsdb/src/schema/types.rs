use crate::error::{EcsDbError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FieldType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Array {
        element_type: Box<FieldType>,
        length: usize,
    },
    Enum(String),   // References enum definition
    Struct(String), // References custom type
    Custom(String), // User-defined type
}

impl FieldType {
    /// Returns the size in bytes for this type
    pub fn size_bytes(&self) -> Result<usize> {
        match self {
            FieldType::U8 => Ok(1),
            FieldType::U16 => Ok(2),
            FieldType::U32 => Ok(4),
            FieldType::U64 => Ok(8),
            FieldType::I8 => Ok(1),
            FieldType::I16 => Ok(2),
            FieldType::I32 => Ok(4),
            FieldType::I64 => Ok(8),
            FieldType::F32 => Ok(4),
            FieldType::F64 => Ok(8),
            FieldType::Bool => Ok(1),
            FieldType::Array {
                element_type,
                length,
            } => {
                let elem_size = element_type.size_bytes()?;
                Ok(elem_size * length)
            }
            FieldType::Enum(_) => Ok(4), // u32 discriminant
            FieldType::Struct(_) | FieldType::Custom(_) => Err(EcsDbError::SchemaError(
                "Custom types must be resolved before size calculation".into(),
            )),
        }
    }

    /// Returns the alignment requirement in bytes
    pub fn alignment(&self) -> usize {
        match self {
            FieldType::U8 | FieldType::I8 | FieldType::Bool => 1,
            FieldType::U16 | FieldType::I16 => 2,
            FieldType::U32 | FieldType::I32 | FieldType::F32 | FieldType::Enum(_) => 4,
            FieldType::U64 | FieldType::I64 | FieldType::F64 => 8,
            FieldType::Array { element_type, .. } => element_type.alignment(),
            _ => 8, // Conservative default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub indexed: bool,
    pub primary_key: bool,
    pub foreign_key: Option<String>, // References "table.field"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDefinition {
    pub name: String,
    pub fields: Vec<FieldDefinition>,
    pub parent_table: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub name: String,
    pub version: String,
    pub tables: Vec<TableDefinition>,
    pub enums: std::collections::HashMap<String, Vec<String>>,
    pub custom_types: std::collections::HashMap<String, Vec<FieldDefinition>>,
}
