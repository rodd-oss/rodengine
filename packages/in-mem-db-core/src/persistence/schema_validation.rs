//! Schema validation logic for corruption detection.

use crate::error::DbError;
use crate::table::{validation, Field};
use crate::types::TypeRegistry;

use super::schema::{RelationSchema, SchemaFile, TableSchema};

/// Validates schema integrity for corruption detection.
///
/// # Arguments
/// * `schema` - Schema to validate
/// * `type_registry` - Type registry for type validation
///
/// # Returns
/// `Result<(), DbError>` indicating success or validation failure.
pub fn validate_schema(schema: &SchemaFile, type_registry: &TypeRegistry) -> Result<(), DbError> {
    // Validate custom types
    for (type_id, type_schema) in &schema.custom_types {
        if type_schema.size == 0 {
            return Err(DbError::DataCorruption(format!(
                "Custom type '{}' has zero size",
                type_id
            )));
        }
        if type_schema.align == 0 {
            return Err(DbError::DataCorruption(format!(
                "Custom type '{}' has zero alignment",
                type_id
            )));
        }
        if !type_schema.align.is_power_of_two() {
            return Err(DbError::DataCorruption(format!(
                "Custom type '{}' alignment {} is not a power of two",
                type_id, type_schema.align
            )));
        }
        if type_schema.size % type_schema.align != 0 {
            return Err(DbError::DataCorruption(format!(
                "Custom type '{}' size {} not multiple of alignment {}",
                type_id, type_schema.size, type_schema.align
            )));
        }
    }

    // Validate each table
    for (table_name, table_schema) in &schema.tables {
        validate_table_schema(table_name, table_schema, type_registry)?;
    }

    // Validate relations (after all tables validated)
    validate_relations(&schema.tables)?;

    Ok(())
}

/// Validates a single table schema.
fn validate_table_schema(
    table_name: &str,
    table_schema: &TableSchema,
    type_registry: &TypeRegistry,
) -> Result<(), DbError> {
    // Validate duplicate field names
    let mut seen_names = std::collections::HashSet::new();
    for field_schema in &table_schema.fields {
        if !seen_names.insert(&field_schema.name) {
            return Err(DbError::DataCorruption(format!(
                "Duplicate field name '{}' in table '{}'",
                field_schema.name, table_name
            )));
        }
    }

    // Validate field types exist
    for field_schema in &table_schema.fields {
        if !type_registry.type_ids().contains(&field_schema.r#type) {
            return Err(DbError::DataCorruption(format!(
                "Unknown type '{}' for field '{}' in table '{}'",
                field_schema.r#type, field_schema.name, table_name
            )));
        }
    }

    // Build temporary fields to validate layout
    let fields = build_fields_from_schema(table_schema, type_registry)?;

    // Validate record size matches stored record_size
    let calculated_record_size = validation::calculate_record_size(&fields).map_err(|e| {
        DbError::DataCorruption(format!(
            "Record size calculation failed for table '{}': {}",
            table_name, e
        ))
    })?;
    if calculated_record_size != table_schema.record_size {
        return Err(DbError::DataCorruption(format!(
            "Record size mismatch for table '{}': stored {}, calculated {}",
            table_name, table_schema.record_size, calculated_record_size
        )));
    }

    // Validate field offsets fit within record size
    validation::validate_record_size(&fields, table_schema.record_size).map_err(|e| {
        DbError::DataCorruption(format!(
            "Field validation failed for table '{}': {}",
            table_name, e
        ))
    })?;

    // Validate field alignment and overlapping fields
    validation::validate_field_layout(&fields).map_err(|e| {
        DbError::DataCorruption(format!(
            "Field layout validation failed for table '{}': {}",
            table_name, e
        ))
    })?;

    Ok(())
}

/// Builds field definitions from schema for validation.
fn build_fields_from_schema(
    table_schema: &TableSchema,
    type_registry: &TypeRegistry,
) -> Result<Vec<Field>, DbError> {
    let mut fields = Vec::new();
    for field_schema in &table_schema.fields {
        let layout = type_registry.get(&field_schema.r#type).ok_or_else(|| {
            DbError::DataCorruption(format!(
                "Type '{}' not found for field '{}'",
                field_schema.r#type, field_schema.name
            ))
        })?;
        let field = Field::new(
            field_schema.name.clone(),
            field_schema.r#type.clone(),
            layout.clone(),
            field_schema.offset,
        );
        fields.push(field);
    }
    Ok(fields)
}

/// Validates relations between tables.
fn validate_relations(
    tables: &std::collections::HashMap<String, TableSchema>,
) -> Result<(), DbError> {
    for (table_name, table_schema) in tables {
        for relation_schema in &table_schema.relations {
            validate_relation(table_name, relation_schema, tables)?;
        }
    }
    Ok(())
}

/// Validates a single relation.
fn validate_relation(
    table_name: &str,
    relation_schema: &RelationSchema,
    tables: &std::collections::HashMap<String, TableSchema>,
) -> Result<(), DbError> {
    // Check target table exists
    if !tables.contains_key(&relation_schema.to_table) {
        return Err(DbError::DataCorruption(format!(
            "Relation target table '{}' not found for relation from '{}'.'{}'",
            relation_schema.to_table, table_name, relation_schema.from_field
        )));
    }

    // Check source field exists
    let source_table = &tables[table_name];
    let source_field_exists = source_table
        .fields
        .iter()
        .any(|f| f.name == relation_schema.from_field);
    if !source_field_exists {
        return Err(DbError::DataCorruption(format!(
            "Relation source field '{}' not found in table '{}'",
            relation_schema.from_field, table_name
        )));
    }

    // Check target field exists in target table
    let target_table = &tables[&relation_schema.to_table];
    let target_field_exists = target_table
        .fields
        .iter()
        .any(|f| f.name == relation_schema.to_field);
    if !target_field_exists {
        return Err(DbError::DataCorruption(format!(
            "Relation target field '{}' not found in table '{}'",
            relation_schema.to_field, relation_schema.to_table
        )));
    }

    Ok(())
}
