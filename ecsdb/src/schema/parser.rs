use super::types::*;
use crate::error::{EcsDbError, Result};
use std::fs;

pub struct SchemaParser;

impl SchemaParser {
    pub fn from_file(path: &str) -> Result<DatabaseSchema> {
        let content = fs::read_to_string(path)?;
        Self::from_string(&content)
    }

    pub fn from_string(toml_str: &str) -> Result<DatabaseSchema> {
        let schema: toml::Value = toml::from_str(toml_str)
            .map_err(|e| EcsDbError::SchemaError(format!("TOML parse error: {}", e)))?;

        let database = schema
            .get("database")
            .ok_or_else(|| EcsDbError::SchemaError("Missing [database] section".into()))?;

        let name = database
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| EcsDbError::SchemaError("Missing database.name".into()))?;

        let version = database
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "1.0.0".to_string());

        // Parse custom types
        let mut custom_types = std::collections::HashMap::new();
        if let Some(types) = schema.get("custom_types") {
            for (type_name, fields) in types.as_table().unwrap_or(&Default::default()) {
                let field_defs = Self::parse_field_list(fields)?;
                custom_types.insert(type_name.clone(), field_defs);
            }
        }

        // Parse enums
        let mut enums = std::collections::HashMap::new();
        if let Some(enum_defs) = schema.get("enums") {
            for (enum_name, variants) in enum_defs.as_table().unwrap_or(&Default::default()) {
                if let Some(vars) = variants.get("variants").and_then(|v| v.as_array()) {
                    let variant_names = vars
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    enums.insert(enum_name.clone(), variant_names);
                }
            }
        }

        // Parse tables
        let mut tables = Vec::new();
        if let Some(table_defs) = schema.get("tables") {
            for (table_name, table_config) in table_defs.as_table().unwrap_or(&Default::default()) {
                let fields = Self::parse_field_list(table_config)?;

                let parent_table = table_config
                    .get("parent_table")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let description = table_config
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                tables.push(TableDefinition {
                    name: table_name.clone(),
                    fields,
                    parent_table,
                    description,
                });
            }
        }

        Ok(DatabaseSchema {
            name,
            version,
            tables,
            enums,
            custom_types,
        })
    }

    fn parse_field_list(config: &toml::Value) -> Result<Vec<FieldDefinition>> {
        let mut fields = Vec::new();

        if let Some(field_array) = config.get("fields").and_then(|v| v.as_array()) {
            for field_val in field_array {
                let name = field_val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| EcsDbError::SchemaError("Field missing 'name'".into()))?;

                let type_str = field_val
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EcsDbError::SchemaError("Field missing 'type'".into()))?;

                let field_type = Self::parse_type(type_str)?;

                let nullable = field_val
                    .get("nullable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let indexed = field_val
                    .get("indexed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let primary_key = field_val
                    .get("primary_key")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let foreign_key = field_val
                    .get("foreign_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                fields.push(FieldDefinition {
                    name,
                    field_type,
                    nullable,
                    indexed,
                    primary_key,
                    foreign_key,
                });
            }
        }

        Ok(fields)
    }

    fn parse_type(type_str: &str) -> Result<FieldType> {
        match type_str {
            "u8" => Ok(FieldType::U8),
            "u16" => Ok(FieldType::U16),
            "u32" => Ok(FieldType::U32),
            "u64" => Ok(FieldType::U64),
            "i8" => Ok(FieldType::I8),
            "i16" => Ok(FieldType::I16),
            "i32" => Ok(FieldType::I32),
            "i64" => Ok(FieldType::I64),
            "f32" => Ok(FieldType::F32),
            "f64" => Ok(FieldType::F64),
            "bool" => Ok(FieldType::Bool),
            s if s.starts_with('[') && s.ends_with(']') => {
                // Parse array: [T; N]
                let inner = &s[1..s.len() - 1];
                let parts: Vec<&str> = inner.split(';').map(|p| p.trim()).collect();
                if parts.len() != 2 {
                    return Err(EcsDbError::SchemaError(format!(
                        "Invalid array type syntax: {}",
                        type_str
                    )));
                }
                let element_type = Box::new(Self::parse_type(parts[0])?);
                let length = parts[1].parse().map_err(|_| {
                    EcsDbError::SchemaError(format!("Invalid array length: {}", parts[1]))
                })?;
                Ok(FieldType::Array {
                    element_type,
                    length,
                })
            }
            s => Ok(FieldType::Custom(s.to_string())),
        }
    }
}
