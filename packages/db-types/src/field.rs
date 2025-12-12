//! Field definitions and types.

use crate::types::Type;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for field creation and validation.
///
/// # TODO: Add more error variants
/// Consider adding `EmptyName` for empty field names and `NameTooLong` for
/// field names that exceed reasonable limits.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FieldError {
    /// The field offset is not properly aligned for the field type.
    #[error("field offset {offset} is not aligned to {alignment} for type {ty:?}")]
    MisalignedOffset {
        /// The field offset.
        offset: usize,
        /// The required alignment.
        alignment: usize,
        /// The field type.
        ty: Type,
    },
}

/// Represents a field in a table schema.
///
/// # Invariants
///
/// - `offset` must be a multiple of `ty.alignment()` for proper memory alignment
/// - `name` should be non-empty for valid schema fields (enforced by schema validation)
/// - `offset + ty.size()` must not overflow `usize` (checked during construction)
///
/// # TODO: Consider making fields private with validation
/// The fields are currently public for serialization but could be made private
/// with a validated constructor to ensure invariants are maintained.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Field {
    /// Name of the field.
    name: String,
    /// Type of the field.
    ty: Type,
    /// Byte offset within the record (calculated during schema creation).
    ///
    /// Must be a multiple of `ty.alignment()` for proper memory alignment.
    offset: usize,
}

/// Builder for creating Field instances with proper offset calculation.
#[derive(Debug)]
pub struct FieldBuilder {
    name: String,
    ty: Type,
}

impl FieldBuilder {
    /// Creates a new field builder with the given name and type.
    ///
    /// # TODO: Add field name validation
    /// Consider validating that field names are non-empty and follow naming conventions.
    /// This could return `Result<Self, FieldError>` instead of panicking.
    #[must_use]
    pub fn new(name: String, ty: Type) -> Self {
        Self { name, ty }
    }

    /// Builds the field with an offset calculated from the current offset.
    /// The offset is calculated based on the current offset and the field's alignment.
    ///
    /// # Panics
    ///
    /// Panics if the offset calculation would overflow `usize`. This should only
    /// happen with extremely large offsets that exceed the addressable memory space.
    #[must_use]
    pub fn build_with_offset(self, current_offset: usize) -> Field {
        let offset = crate::align_offset(current_offset, self.ty)
            .expect("offset calculation would overflow usize");
        Field {
            name: self.name,
            ty: self.ty,
            offset,
        }
    }

    /// Builds the field with a specific offset, validating alignment.
    /// Returns an error if the offset is not properly aligned for the field type.
    pub fn build_with_validated_offset(self, offset: usize) -> Result<Field, FieldError> {
        let alignment = self.ty.alignment();
        if !offset.is_multiple_of(alignment) {
            return Err(FieldError::MisalignedOffset {
                offset,
                alignment,
                ty: self.ty,
            });
        }

        Ok(Field {
            name: self.name,
            ty: self.ty,
            offset,
        })
    }
}

/// Builder for creating multiple fields with automatic offset calculation.
#[derive(Debug, Default)]
pub struct FieldListBuilder {
    fields: Vec<Field>,
    current_offset: usize,
}

impl FieldListBuilder {
    /// Creates a new empty FieldListBuilder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            current_offset: 0,
        }
    }

    /// Adds a field to the list, automatically calculating its offset.
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// Panics if the offset calculation would overflow `usize`. This should only
    /// happen with extremely large offsets that exceed the addressable memory space.
    ///
    /// # TODO: Add field name validation
    /// Consider validating field names here or in the FieldBuilder.
    #[must_use]
    pub fn add_field(mut self, name: String, ty: Type) -> Self {
        let field = Field::builder(name, ty).build_with_offset(self.current_offset);
        self.current_offset = field.end_offset();
        self.fields.push(field);
        self
    }

    /// Builds the list of fields.
    #[must_use]
    pub fn build(self) -> Vec<Field> {
        self.fields
    }

    /// Returns the current offset (end offset of the last field).
    #[must_use]
    pub fn current_offset(&self) -> usize {
        self.current_offset
    }

    /// Returns the number of fields in the builder.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns true if the builder has no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl Field {
    /// Creates a new field builder with the given name and type.
    /// Use the builder to create fields with proper offset calculation.
    #[must_use]
    pub fn builder(name: String, ty: Type) -> FieldBuilder {
        FieldBuilder::new(name, ty)
    }

    /// Validates that the field's offset is properly aligned.
    /// Returns an error if the offset is not aligned to the field type's alignment requirement.
    ///
    /// # TODO: Add debug assertions for development
    /// Consider adding `debug_assert!` in this method to catch alignment issues
    /// during development while keeping runtime checks for production.
    pub fn validate_alignment(&self) -> Result<(), FieldError> {
        let alignment = self.ty.alignment();
        if !self.offset.is_multiple_of(alignment) {
            return Err(FieldError::MisalignedOffset {
                offset: self.offset,
                alignment,
                ty: self.ty,
            });
        }
        Ok(())
    }

    /// Returns the name of the field.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the type of the field.
    pub fn ty(&self) -> Type {
        self.ty
    }

    /// Returns the byte offset of the field within the record.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the size of this field in bytes.
    pub fn size(&self) -> usize {
        self.ty.size()
    }

    /// Returns the alignment requirement of this field in bytes.
    pub fn alignment(&self) -> usize {
        self.ty.alignment()
    }

    /// Returns the end offset of this field (offset + size).
    ///
    /// # TODO: Consider overflow protection
    /// This could overflow if `offset + size()` exceeds `usize::MAX`.
    /// Consider returning `Option<usize>` or using `checked_add` for safety.
    pub fn end_offset(&self) -> usize {
        self.offset + self.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_builder() {
        let field = Field::builder("age".to_string(), Type::I32).build_with_offset(0);
        assert_eq!(field.name(), "age");
        assert_eq!(field.ty(), Type::I32);
        assert_eq!(field.offset(), 0);
        assert_eq!(field.size(), 4);
        assert_eq!(field.alignment(), 4);
        assert_eq!(field.end_offset(), 4);
    }

    #[test]
    fn test_field_builder_with_offset() {
        // Test with alignment = 4, starting at offset 1
        let field = Field::builder("age".to_string(), Type::I32).build_with_offset(1);
        assert_eq!(field.offset(), 4); // Aligned from 1 to 4
        assert_eq!(field.end_offset(), 8);

        // Test with alignment = 1, starting at offset 5
        let field = Field::builder("flag".to_string(), Type::Bool).build_with_offset(5);
        assert_eq!(field.offset(), 5); // No alignment needed
        assert_eq!(field.end_offset(), 6);
    }

    #[test]
    fn test_field_builder_with_validated_offset() {
        // Test valid aligned offset
        let field = Field::builder("age".to_string(), Type::I32)
            .build_with_validated_offset(4)
            .unwrap();
        assert_eq!(field.offset(), 4);

        // Test invalid misaligned offset
        let result = Field::builder("x".to_string(), Type::I32).build_with_validated_offset(3);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            FieldError::MisalignedOffset {
                offset: 3,
                alignment: 4,
                ty: Type::I32,
            }
        );

        // Test valid offset for alignment = 1
        let field = Field::builder("flag".to_string(), Type::Bool)
            .build_with_validated_offset(7)
            .unwrap();
        assert_eq!(field.offset(), 7);
    }

    #[test]
    fn test_field_equality() {
        let field1 = Field::builder("age".to_string(), Type::I32).build_with_offset(0);
        let field2 = Field::builder("age".to_string(), Type::I32).build_with_offset(0);
        let field3 = Field::builder("score".to_string(), Type::F32).build_with_offset(0);

        assert_eq!(field1, field2);
        assert_ne!(field1, field3);
    }

    #[test]
    fn test_field_serialization() {
        let field = Field::builder("age".to_string(), Type::I32).build_with_offset(4);
        let json = serde_json::to_string(&field).unwrap();
        let decoded: Field = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.name(), "age");
        assert_eq!(decoded.ty(), Type::I32);
        assert_eq!(decoded.offset(), 4);
    }

    #[test]
    fn test_field_accessors() {
        let field = Field::builder("count".to_string(), Type::U64).build_with_offset(8);

        assert_eq!(field.name(), "count");
        assert_eq!(field.ty(), Type::U64);
        assert_eq!(field.offset(), 8);
        assert_eq!(field.size(), 8);
        assert_eq!(field.alignment(), 8);
        assert_eq!(field.end_offset(), 16);
    }

    #[test]
    fn test_field_validate_alignment() {
        // Create a field with proper alignment (via builder)
        let field = Field::builder("age".to_string(), Type::I32).build_with_offset(0);
        assert!(field.validate_alignment().is_ok());

        // Create a field with proper alignment at non-zero offset
        let field = Field::builder("age".to_string(), Type::I32).build_with_offset(4);
        assert!(field.validate_alignment().is_ok());

        // Test deserialized field with proper alignment
        let json = r#"{"name":"test","ty":"I32","offset":8}"#;
        let field: Field = serde_json::from_str(json).unwrap();
        assert!(field.validate_alignment().is_ok());
    }

    #[test]
    fn test_field_validate_alignment_error() {
        // Create a JSON representation of a misaligned field
        let json = r#"{"name":"test","ty":"I32","offset":3}"#;
        let field: Field = serde_json::from_str(json).unwrap();

        let result = field.validate_alignment();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            FieldError::MisalignedOffset {
                offset: 3,
                alignment: 4,
                ty: Type::I32,
            }
        );
    }

    #[test]
    fn test_field_error_display() {
        let error = FieldError::MisalignedOffset {
            offset: 3,
            alignment: 4,
            ty: Type::I32,
        };

        let display = format!("{}", error);
        assert!(display.contains("field offset 3 is not aligned to 4 for type I32"));
    }

    #[test]
    fn test_field_list_builder_empty() {
        let builder = FieldListBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.current_offset(), 0);

        let fields = builder.build();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_field_list_builder_single_field() {
        let builder = FieldListBuilder::new().add_field("id".to_string(), Type::I32);

        assert!(!builder.is_empty());
        assert_eq!(builder.len(), 1);
        assert_eq!(builder.current_offset(), 4); // i32 size = 4

        let fields = builder.build();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name(), "id");
        assert_eq!(fields[0].ty(), Type::I32);
        assert_eq!(fields[0].offset(), 0);
        assert_eq!(fields[0].end_offset(), 4);
    }

    #[test]
    fn test_field_list_builder_multiple_fields() {
        let builder = FieldListBuilder::new()
            .add_field("id".to_string(), Type::I32) // offset 0, size 4
            .add_field("score".to_string(), Type::F64) // offset 8 (aligned from 4), size 8
            .add_field("active".to_string(), Type::Bool) // offset 16, size 1
            .add_field("count".to_string(), Type::U64); // offset 24 (aligned from 17), size 8

        assert_eq!(builder.len(), 4);
        assert_eq!(builder.current_offset(), 32); // 24 + 8 = 32

        let fields = builder.build();
        assert_eq!(fields.len(), 4);

        // Check field 0: id (i32)
        assert_eq!(fields[0].name(), "id");
        assert_eq!(fields[0].ty(), Type::I32);
        assert_eq!(fields[0].offset(), 0);
        assert_eq!(fields[0].end_offset(), 4);

        // Check field 1: score (f64) - should be aligned to 8
        assert_eq!(fields[1].name(), "score");
        assert_eq!(fields[1].ty(), Type::F64);
        assert_eq!(fields[1].offset(), 8); // Aligned from 4 to 8
        assert_eq!(fields[1].end_offset(), 16);

        // Check field 2: active (bool) - no alignment needed
        assert_eq!(fields[2].name(), "active");
        assert_eq!(fields[2].ty(), Type::Bool);
        assert_eq!(fields[2].offset(), 16);
        assert_eq!(fields[2].end_offset(), 17);

        // Check field 3: count (u64) - should be aligned to 8 from 17
        assert_eq!(fields[3].name(), "count");
        assert_eq!(fields[3].ty(), Type::U64);
        assert_eq!(fields[3].offset(), 24); // Aligned from 17 to 24
        assert_eq!(fields[3].end_offset(), 32);
    }

    #[test]
    fn test_field_list_builder_with_mixed_alignment() {
        // Test with types that have different alignment requirements
        let builder = FieldListBuilder::new()
            .add_field("a".to_string(), Type::U8) // offset 0, size 1, align 1
            .add_field("b".to_string(), Type::I16) // offset 2 (aligned from 1), size 2, align 2
            .add_field("c".to_string(), Type::I32) // offset 4 (aligned from 3), size 4, align 4
            .add_field("d".to_string(), Type::I128); // offset 16 (aligned from 8), size 16, align 16

        let fields = builder.build();

        assert_eq!(fields[0].offset(), 0); // u8
        assert_eq!(fields[1].offset(), 2); // i16 (aligned from 1 to 2)
        assert_eq!(fields[2].offset(), 4); // i32 (aligned from 3 to 4)
        assert_eq!(fields[3].offset(), 16); // i128 (aligned from 8 to 16)

        // Verify all fields are properly aligned
        for field in &fields {
            assert!(field.validate_alignment().is_ok());
        }
    }
}
