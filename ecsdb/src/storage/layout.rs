use crate::error::{EcsDbError, Result};
use crate::schema::types::{FieldDefinition, FieldType};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub definition: FieldDefinition,
    pub offset: usize,
    pub size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone)]
pub struct RecordLayout {
    pub fields: Vec<FieldLayout>,
    pub total_size: usize,
    pub alignment: usize,
}

pub fn compute_record_layout(
    fields: &[FieldDefinition],
    custom_types: &HashMap<String, Vec<FieldDefinition>>,
) -> Result<RecordLayout> {
    let mut field_layouts = Vec::with_capacity(fields.len());
    let mut current_offset = 0;
    let mut max_alignment = 1;

    for field in fields {
        let (size, alignment) = compute_field_size_and_alignment(&field.field_type, custom_types)?;

        // Add padding to satisfy alignment
        let padding = (alignment - (current_offset % alignment)) % alignment;
        current_offset += padding;

        let layout = FieldLayout {
            definition: field.clone(),
            offset: current_offset,
            size,
            alignment,
        };

        field_layouts.push(layout);
        current_offset += size;
        max_alignment = max_alignment.max(alignment);
    }

    // Add trailing padding to align total size to max alignment
    let total_padding = (max_alignment - (current_offset % max_alignment)) % max_alignment;
    let total_size = current_offset + total_padding;

    Ok(RecordLayout {
        fields: field_layouts,
        total_size,
        alignment: max_alignment,
    })
}

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
            // Array alignment is element alignment
            // Array size is elem_size * length, but may need padding between elements?
            // For simplicity, assume elements are packed (no padding between elements).
            // However, each element must be aligned properly. We'll treat array as contiguous bytes.
            // For now, we'll just multiply.
            Ok((elem_size * length, elem_alignment))
        }
        FieldType::Enum(_) => Ok((4, 4)), // u32 discriminant
        FieldType::Struct(name) | FieldType::Custom(name) => {
            // Look up custom type definition
            let custom_fields = custom_types.get(name).ok_or_else(|| {
                EcsDbError::SchemaError(format!("Custom type '{}' not found", name))
            })?;

            // Recursively compute layout for custom type
            let record_layout = compute_record_layout(custom_fields, custom_types)?;
            Ok((record_layout.total_size, record_layout.alignment))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::types::{FieldDefinition, FieldType};

    #[test]
    fn test_primitive_layout() -> Result<()> {
        let fields = vec![
            FieldDefinition {
                name: "a".to_string(),
                field_type: FieldType::U32,
                nullable: false,
                indexed: false,
                primary_key: false,
                foreign_key: None,
            },
            FieldDefinition {
                name: "b".to_string(),
                field_type: FieldType::U64,
                nullable: false,
                indexed: false,
                primary_key: false,
                foreign_key: None,
            },
        ];

        let custom_types = HashMap::new();
        let layout = compute_record_layout(&fields, &custom_types)?;

        // u32 (4 bytes) at offset 0, u64 (8 bytes) at offset 8 (aligned to 8)
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[0].size, 4);
        assert_eq!(layout.fields[1].offset, 8);
        assert_eq!(layout.fields[1].size, 8);
        assert_eq!(layout.total_size, 16);

        Ok(())
    }

    #[test]
    fn test_array_layout() -> Result<()> {
        let fields = vec![FieldDefinition {
            name: "arr".to_string(),
            field_type: FieldType::Array {
                element_type: Box::new(FieldType::U32),
                length: 3,
            },
            nullable: false,
            indexed: false,
            primary_key: false,
            foreign_key: None,
        }];

        let custom_types = HashMap::new();
        let layout = compute_record_layout(&fields, &custom_types)?;

        // array of 3 u32 = 12 bytes, alignment 4
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[0].size, 12);
        assert_eq!(layout.total_size, 12);

        Ok(())
    }
}
