use super::types::*;
use crate::error::{EcsDbError, Result};

pub struct SchemaValidator;

impl SchemaValidator {
    pub fn validate(&self, schema: &DatabaseSchema) -> Result<()> {
        self.check_foreign_keys(schema)?;
        self.check_field_alignment(schema)?;
        self.check_reserved_names(schema)?;
        self.check_table_names_unique(schema)?;
        self.check_field_names_unique(schema)?;
        Ok(())
    }

    pub fn check_foreign_keys(&self, schema: &DatabaseSchema) -> Result<()> {
        for table in &schema.tables {
            for field in &table.fields {
                if let Some(fk) = &field.foreign_key {
                    // Parse "table.field" or "table"
                    let (ref_table, ref_field) = if let Some((tbl, fld)) = fk.split_once('.') {
                        (tbl, Some(fld))
                    } else {
                        (fk.as_str(), None)
                    };

                    // Find referenced table
                    let referenced_table = schema.tables.iter()
                        .find(|t| t.name == ref_table)
                        .ok_or_else(|| EcsDbError::SchemaError(
                            format!("Foreign key references unknown table '{}' in table '{}' field '{}'", 
                                ref_table, table.name, field.name)
                        ))?;

                    // If field specified, find referenced field
                    if let Some(ref_field_name) = ref_field {
                        let referenced_field = referenced_table.fields.iter()
                            .find(|f| f.name == ref_field_name)
                            .ok_or_else(|| EcsDbError::SchemaError(
                                format!("Foreign key references unknown field '{}.{}' in table '{}' field '{}'", 
                                    ref_table, ref_field_name, table.name, field.name)
                            ))?;

                        // Ensure referenced field is primary key (or at least indexed)
                        if !referenced_field.primary_key {
                            return Err(EcsDbError::SchemaError(
                                format!("Foreign key must reference a primary key field: '{}.{}' is not primary key", 
                                    ref_table, ref_field_name)
                            ));
                        }

                        // Ensure field types are compatible
                        if !Self::are_types_compatible(
                            &field.field_type,
                            &referenced_field.field_type,
                        ) {
                            return Err(EcsDbError::SchemaError(
                                format!("Foreign key type mismatch: field '{}' type {:?} vs referenced '{}.{}' type {:?}", 
                                    field.name, field.field_type, ref_table, ref_field_name, referenced_field.field_type)
                            ));
                        }
                    } else {
                        // Assume referencing entity ID (primary key)
                        // Check that referenced table has a primary key field
                        let has_pk = referenced_table.fields.iter().any(|f| f.primary_key);
                        if !has_pk {
                            return Err(EcsDbError::SchemaError(format!(
                                "Foreign key references table '{}' which has no primary key field",
                                ref_table
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn are_types_compatible(a: &FieldType, b: &FieldType) -> bool {
        match (a, b) {
            (FieldType::U8, FieldType::U8) => true,
            (FieldType::U16, FieldType::U16) => true,
            (FieldType::U32, FieldType::U32) => true,
            (FieldType::U64, FieldType::U64) => true,
            (FieldType::I8, FieldType::I8) => true,
            (FieldType::I16, FieldType::I16) => true,
            (FieldType::I32, FieldType::I32) => true,
            (FieldType::I64, FieldType::I64) => true,
            (FieldType::F32, FieldType::F32) => true,
            (FieldType::F64, FieldType::F64) => true,
            (FieldType::Bool, FieldType::Bool) => true,
            (
                FieldType::Array {
                    element_type: a_elem,
                    length: a_len,
                },
                FieldType::Array {
                    element_type: b_elem,
                    length: b_len,
                },
            ) => a_len == b_len && Self::are_types_compatible(a_elem, b_elem),
            (FieldType::Enum(a_name), FieldType::Enum(b_name)) => a_name == b_name,
            (FieldType::Struct(a_name), FieldType::Struct(b_name)) => a_name == b_name,
            (FieldType::Custom(a_name), FieldType::Custom(b_name)) => a_name == b_name,
            _ => false,
        }
    }

    pub fn check_field_alignment(&self, schema: &DatabaseSchema) -> Result<()> {
        // For now, just check that array lengths are positive
        for table in &schema.tables {
            for field in &table.fields {
                if let FieldType::Array { length, .. } = &field.field_type {
                    if *length == 0 {
                        return Err(EcsDbError::SchemaError(format!(
                            "Array length must be >0 for field '{}' in table '{}'",
                            field.name, table.name
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn check_reserved_names(&self, schema: &DatabaseSchema) -> Result<()> {
        let reserved = ["id", "version", "entity_id", "table", "schema", "database"];
        for table in &schema.tables {
            if reserved.contains(&table.name.as_str()) {
                return Err(EcsDbError::SchemaError(format!(
                    "Table name '{}' is reserved",
                    table.name
                )));
            }
            for field in &table.fields {
                if reserved.contains(&field.name.as_str()) {
                    return Err(EcsDbError::SchemaError(format!(
                        "Field name '{}' in table '{}' is reserved",
                        field.name, table.name
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn check_table_names_unique(&self, schema: &DatabaseSchema) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for table in &schema.tables {
            if !seen.insert(&table.name) {
                return Err(EcsDbError::SchemaError(format!(
                    "Duplicate table name '{}'",
                    table.name
                )));
            }
        }
        Ok(())
    }

    pub fn check_field_names_unique(&self, schema: &DatabaseSchema) -> Result<()> {
        for table in &schema.tables {
            let mut seen = std::collections::HashSet::new();
            for field in &table.fields {
                if !seen.insert(&field.name) {
                    return Err(EcsDbError::SchemaError(format!(
                        "Duplicate field name '{}' in table '{}'",
                        field.name, table.name
                    )));
                }
            }
        }
        Ok(())
    }
}
