use crate::error::{EcsDbError, Result};
use crate::schema::types::{FieldDefinition, FieldType};
use crate::storage::layout::RecordLayout;
use serde_json::{json, Value as JsonValue};
use serde_value::Value as SerdeValue;
use std::collections::HashMap;

/// Convert component bytes (bincode serialized) to JSON using schema information.
/// This function deserializes the bytes into a generic serde value and then converts to JSON.
/// It works for any component that implements serde::Serialize/Deserialize.
pub fn component_bytes_to_json(bytes: &[u8]) -> Result<JsonValue> {
    // Deserialize into generic serde value
    let value: SerdeValue = bincode::deserialize(bytes).map_err(EcsDbError::SerializationError)?;

    // Convert serde_value to serde_json::Value
    // serde_value::Value implements Serialize, so we can serialize it to JSON
    serde_json::to_value(&value).map_err(|e| EcsDbError::JsonError(e.to_string()))
}

/// Convert component bytes to JSON using field definitions and custom types.
/// This is a lower-level function that extracts fields based on layout.
/// It can handle nested structs and arrays.
pub fn component_bytes_to_json_with_layout(
    bytes: &[u8],
    _field_defs: &[FieldDefinition],
    layout: &RecordLayout,
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<JsonValue> {
    // Build JSON object from fields
    let mut obj = serde_json::Map::new();

    for field_layout in &layout.fields {
        let field_bytes = &bytes[field_layout.offset..field_layout.offset + field_layout.size];
        let value = field_bytes_to_json(
            field_bytes,
            &field_layout.definition.field_type,
            custom_types,
        )?;
        obj.insert(field_layout.definition.name.clone(), value);
    }

    Ok(JsonValue::Object(obj))
}

/// Convert bytes representing a single field to JSON based on field type.
fn field_bytes_to_json(
    bytes: &[u8],
    field_type: &FieldType,
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<JsonValue> {
    match field_type {
        FieldType::U8 => Ok(json!(bytes[0])),
        FieldType::U16 => Ok(json!(u16::from_le_bytes([bytes[0], bytes[1]]))),
        FieldType::U32 => Ok(json!(u32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3]
        ]))),
        FieldType::U64 => Ok(json!(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
        ]))),
        FieldType::I8 => Ok(json!(i8::from_le_bytes([bytes[0]]) as i64)),
        FieldType::I16 => Ok(json!(i16::from_le_bytes([bytes[0], bytes[1]]) as i64)),
        FieldType::I32 => Ok(json!(
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64
        )),
        FieldType::I64 => Ok(json!(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
        ]))),
        FieldType::F32 => Ok(json!(
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64
        )),
        FieldType::F64 => Ok(json!(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
        ]))),
        FieldType::Bool => Ok(json!(bytes[0] != 0)),
        FieldType::Array {
            element_type,
            length,
        } => {
            // Compute element size using helper (recursive)
            let elem_size = compute_field_size_and_alignment(element_type, custom_types)?.0;
            let mut arr = Vec::with_capacity(*length);
            for i in 0..*length {
                let start = i * elem_size;
                let end = start + elem_size;
                let elem_bytes = &bytes[start..end];
                arr.push(field_bytes_to_json(elem_bytes, element_type, custom_types)?);
            }
            Ok(JsonValue::Array(arr))
        }
        FieldType::Enum(_) => {
            // Enum discriminant is u32
            Ok(json!(u32::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3]
            ])))
        }
        FieldType::Struct(name) | FieldType::Custom(name) => {
            let fields = custom_types.get(name).ok_or_else(|| {
                EcsDbError::SchemaError(format!("Custom type '{}' not found", name))
            })?;
            // Compute layout for the custom type
            let layout = crate::storage::layout::compute_record_layout(fields, custom_types)?;
            // Recurse with the custom type's fields
            let mut obj = serde_json::Map::new();
            for field_layout in &layout.fields {
                let field_bytes =
                    &bytes[field_layout.offset..field_layout.offset + field_layout.size];
                let value = field_bytes_to_json(
                    field_bytes,
                    &field_layout.definition.field_type,
                    custom_types,
                )?;
                obj.insert(field_layout.definition.name.clone(), value);
            }
            Ok(JsonValue::Object(obj))
        }
    }
}

/// Helper to compute size and alignment of a field type.
fn compute_field_size_and_alignment(
    field_type: &FieldType,
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<(usize, usize)> {
    match field_type {
        FieldType::U8 => Ok((1, 1)),
        FieldType::U16 => Ok((2, 2)),
        FieldType::U32 => Ok((4, 4)),
        FieldType::U64 => Ok((8, 8)),
        FieldType::I8 => Ok((1, 1)),
        FieldType::I16 => Ok((2, 2)),
        FieldType::I32 => Ok((4, 4)),
        FieldType::I64 => Ok((8, 8)),
        FieldType::F32 => Ok((4, 4)),
        FieldType::F64 => Ok((8, 8)),
        FieldType::Bool => Ok((1, 1)),
        FieldType::Array {
            element_type,
            length,
        } => {
            let (elem_size, elem_alignment) =
                compute_field_size_and_alignment(element_type, custom_types)?;
            Ok((elem_size * length, elem_alignment))
        }
        FieldType::Enum(_) => Ok((4, 4)),
        FieldType::Struct(name) | FieldType::Custom(name) => {
            let fields = custom_types.get(name).ok_or_else(|| {
                EcsDbError::SchemaError(format!("Custom type '{}' not found", name))
            })?;
            let layout = crate::storage::layout::compute_record_layout(fields, custom_types)?;
            Ok((layout.total_size, layout.alignment))
        }
    }
}

/// Convert JSON object to component bytes using field definitions and custom types.
pub fn json_to_component_bytes_with_layout(
    json: &JsonValue,
    _field_defs: &[FieldDefinition],
    layout: &RecordLayout,
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; layout.total_size];
    for field_layout in &layout.fields {
        let field_name = &field_layout.definition.name;
        let value = json.get(field_name).ok_or_else(|| {
            EcsDbError::JsonError(format!("Missing field '{}' in JSON", field_name))
        })?;
        let bytes = json_to_field_bytes(value, &field_layout.definition.field_type, custom_types)?;
        // Ensure bytes length matches field size
        if bytes.len() != field_layout.size {
            return Err(EcsDbError::JsonError(format!(
                "Field '{}' size mismatch: expected {} bytes, got {}",
                field_name,
                field_layout.size,
                bytes.len()
            )));
        }
        buffer[field_layout.offset..field_layout.offset + field_layout.size]
            .copy_from_slice(&bytes);
    }
    Ok(buffer)
}

/// Convert JSON value to bytes for a single field.
fn json_to_field_bytes(
    json: &JsonValue,
    field_type: &FieldType,
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<Vec<u8>> {
    match field_type {
        FieldType::U8 => Ok(vec![json
            .as_u64()
            .ok_or_else(|| EcsDbError::JsonError("Expected u8".into()))?
            as u8]),
        FieldType::U16 => Ok(u16::to_le_bytes(
            json.as_u64()
                .ok_or_else(|| EcsDbError::JsonError("Expected u16".into()))? as u16,
        )
        .to_vec()),
        FieldType::U32 => Ok(u32::to_le_bytes(
            json.as_u64()
                .ok_or_else(|| EcsDbError::JsonError("Expected u32".into()))? as u32,
        )
        .to_vec()),
        FieldType::U64 => Ok(u64::to_le_bytes(
            json.as_u64()
                .ok_or_else(|| EcsDbError::JsonError("Expected u64".into()))?,
        )
        .to_vec()),
        FieldType::I8 => Ok(vec![json
            .as_i64()
            .ok_or_else(|| EcsDbError::JsonError("Expected i8".into()))?
            as i8 as u8]),
        FieldType::I16 => Ok(i16::to_le_bytes(
            json.as_i64()
                .ok_or_else(|| EcsDbError::JsonError("Expected i16".into()))? as i16,
        )
        .to_vec()),
        FieldType::I32 => Ok(i32::to_le_bytes(
            json.as_i64()
                .ok_or_else(|| EcsDbError::JsonError("Expected i32".into()))? as i32,
        )
        .to_vec()),
        FieldType::I64 => Ok(i64::to_le_bytes(
            json.as_i64()
                .ok_or_else(|| EcsDbError::JsonError("Expected i64".into()))?,
        )
        .to_vec()),
        FieldType::F32 => Ok(f32::to_le_bytes(
            json.as_f64()
                .ok_or_else(|| EcsDbError::JsonError("Expected f32".into()))? as f32,
        )
        .to_vec()),
        FieldType::F64 => Ok(f64::to_le_bytes(
            json.as_f64()
                .ok_or_else(|| EcsDbError::JsonError("Expected f64".into()))?,
        )
        .to_vec()),
        FieldType::Bool => Ok(vec![json
            .as_bool()
            .ok_or_else(|| EcsDbError::JsonError("Expected bool".into()))?
            as u8]),
        FieldType::Array {
            element_type,
            length,
        } => {
            // JSON array must have correct length
            let arr = json
                .as_array()
                .ok_or_else(|| EcsDbError::JsonError("Expected array".into()))?;
            if arr.len() != *length {
                return Err(EcsDbError::JsonError(format!(
                    "Array length mismatch: expected {}, got {}",
                    length,
                    arr.len()
                )));
            }
            let elem_size = compute_field_size_and_alignment(element_type, custom_types)?.0;
            let mut buffer = vec![0u8; elem_size * length];
            for (i, elem) in arr.iter().enumerate() {
                let bytes = json_to_field_bytes(elem, element_type, custom_types)?;
                buffer[i * elem_size..(i + 1) * elem_size].copy_from_slice(&bytes);
            }
            Ok(buffer)
        }
        FieldType::Enum(_) => {
            // Enum discriminant is u32
            Ok(u32::to_le_bytes(
                json.as_u64()
                    .ok_or_else(|| EcsDbError::JsonError("Expected enum discriminant".into()))?
                    as u32,
            )
            .to_vec())
        }
        FieldType::Struct(name) | FieldType::Custom(name) => {
            let fields = custom_types.get(name).ok_or_else(|| {
                EcsDbError::SchemaError(format!("Custom type '{}' not found", name))
            })?;
            let layout = crate::storage::layout::compute_record_layout(fields, custom_types)?;
            // Recurse with the custom type's fields
            json_to_component_bytes_with_layout(json, fields, &layout, custom_types)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::types::{FieldDefinition, FieldType};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestComponent {
        x: f32,
        y: f32,
        id: u32,
    }

    #[test]
    fn test_component_bytes_to_json_with_layout() -> Result<()> {
        let comp = TestComponent {
            x: 1.5,
            y: 2.5,
            id: 42,
        };
        let bytes = bincode::serialize(&comp).unwrap();

        // Create field definitions matching TestComponent
        let field_defs = vec![
            FieldDefinition {
                name: "x".to_string(),
                field_type: FieldType::F32,
                nullable: false,
                indexed: false,
                primary_key: false,
                foreign_key: None,
            },
            FieldDefinition {
                name: "y".to_string(),
                field_type: FieldType::F32,
                nullable: false,
                indexed: false,
                primary_key: false,
                foreign_key: None,
            },
            FieldDefinition {
                name: "id".to_string(),
                field_type: FieldType::U32,
                nullable: false,
                indexed: false,
                primary_key: false,
                foreign_key: None,
            },
        ];

        let custom_types = HashMap::new();
        let layout = crate::storage::layout::compute_record_layout(&field_defs, &custom_types)?;
        let json =
            component_bytes_to_json_with_layout(&bytes, &field_defs, &layout, &custom_types)?;

        // Verify JSON structure
        assert!(json.is_object());
        let obj = json.as_object().unwrap();
        assert_eq!(obj["x"].as_f64().unwrap(), 1.5);
        assert_eq!(obj["y"].as_f64().unwrap(), 2.5);
        assert_eq!(obj["id"].as_u64().unwrap(), 42);

        Ok(())
    }
}
