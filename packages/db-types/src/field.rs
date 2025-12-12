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
    tight_packing: bool,
}

impl FieldListBuilder {
    /// Creates a new empty FieldListBuilder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            current_offset: 0,
            tight_packing: false,
        }
    }

    /// Creates a new empty FieldListBuilder with tight packing mode.
    /// In tight packing mode, fields are placed consecutively without alignment gaps.
    /// This violates normal alignment requirements but may be needed for certain use cases.
    ///
    /// # Warning
    /// Fields created with tight packing will fail `validate_alignment()` checks
    /// because their offsets are not aligned to their type's alignment requirements.
    /// This is intentional - tight packing trades alignment for space efficiency.
    #[must_use]
    pub fn new_tight_packing() -> Self {
        Self {
            fields: Vec::new(),
            current_offset: 0,
            tight_packing: true,
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
        let offset = if self.tight_packing {
            self.current_offset
        } else {
            crate::align_offset(self.current_offset, ty)
                .expect("offset calculation would overflow usize")
        };

        let field = Field { name, ty, offset };

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

    /// Returns `true` if this field's offset is properly aligned for its type.
    /// Fields created with tight packing will return `false`.
    pub fn is_aligned(&self) -> bool {
        self.offset.is_multiple_of(self.ty.alignment())
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
    /// # Panics
    /// Panics if `offset + size()` would overflow `usize`.
    /// For overflow-safe version, use `end_offset_checked()`.
    pub fn end_offset(&self) -> usize {
        self.offset
            .checked_add(self.size())
            .expect("offset + size overflow")
    }

    /// Returns the end offset of this field (offset + size) with overflow protection.
    ///
    /// Returns `Some(end_offset)` if the calculation succeeds, or `None` if it would overflow `usize`.
    pub fn end_offset_checked(&self) -> Option<usize> {
        self.offset.checked_add(self.size())
    }
}

/// Calculates the total record size in bytes for a list of fields.
///
/// The record size is calculated as the maximum end offset among all fields.
/// This accounts for field offsets, including any alignment gaps between fields.
/// If there are no fields, the record size is 0.
///
/// # Note on Tight Packing
/// For tight packing (no alignment gaps), use `FieldListBuilder::new_tight_packing()`
/// to create fields. The calculated size will be the sum of field sizes without
/// alignment gaps.
///
/// # Panics
/// Panics if any field's `end_offset()` calculation would overflow `usize`.
/// This should only happen with extremely large offsets that exceed addressable memory.
///
/// # Examples
/// ```
/// use db_types::{FieldListBuilder, calculate_record_size, Type};
///
/// let fields = FieldListBuilder::new()
///     .add_field("id".to_string(), Type::I32)
///     .add_field("score".to_string(), Type::F64)
///     .add_field("active".to_string(), Type::Bool)
///     .build();
///
/// let size = calculate_record_size(&fields);
/// assert_eq!(size, 17); // Based on field offsets and sizes
/// ```
pub fn calculate_record_size(fields: &[Field]) -> usize {
    fields
        .iter()
        .map(|field| field.end_offset())
        .max()
        .unwrap_or(0)
}

/// Calculates the total record size in bytes for a list of fields with overflow protection.
///
/// Returns `Some(size)` if the calculation succeeds, or `None` if any field's
/// `end_offset()` calculation would overflow `usize`.
///
/// # Examples
/// ```
/// use db_types::{FieldListBuilder, calculate_record_size_checked, Type};
///
/// let fields = FieldListBuilder::new()
///     .add_field("id".to_string(), Type::I32)
///     .add_field("score".to_string(), Type::F64)
///     .build();
///
/// let size = calculate_record_size_checked(&fields);
/// assert_eq!(size, Some(16));
/// ```
pub fn calculate_record_size_checked(fields: &[Field]) -> Option<usize> {
    let mut max_offset = 0;

    for field in fields {
        let end_offset = field.offset.checked_add(field.size())?;
        if end_offset > max_offset {
            max_offset = end_offset;
        }
    }

    Some(max_offset)
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

    #[test]
    fn test_calculate_record_size_empty_fields() {
        let fields: Vec<Field> = vec![];
        assert_eq!(calculate_record_size(&fields), 0);
        assert_eq!(calculate_record_size_checked(&fields), Some(0));
    }

    #[test]
    fn test_calculate_record_size_single_primitive() {
        let fields = FieldListBuilder::new()
            .add_field("id".to_string(), Type::I32)
            .build();

        assert_eq!(calculate_record_size(&fields), 4);
        assert_eq!(calculate_record_size_checked(&fields), Some(4));

        // Test other primitive types
        let fields = FieldListBuilder::new()
            .add_field("value".to_string(), Type::F64)
            .build();

        assert_eq!(calculate_record_size(&fields), 8);
        assert_eq!(calculate_record_size_checked(&fields), Some(8));

        let fields = FieldListBuilder::new()
            .add_field("flag".to_string(), Type::Bool)
            .build();

        assert_eq!(calculate_record_size(&fields), 1);
        assert_eq!(calculate_record_size_checked(&fields), Some(1));
    }

    #[test]
    fn test_calculate_record_size_multiple_same_type() {
        let fields = FieldListBuilder::new_tight_packing()
            .add_field("a".to_string(), Type::I32)
            .add_field("b".to_string(), Type::I32)
            .add_field("c".to_string(), Type::I32)
            .build();

        // Each i32 is 4 bytes
        // With tight packing: 4 + 4 + 4 = 12 bytes
        assert_eq!(calculate_record_size(&fields), 12);
        assert_eq!(calculate_record_size_checked(&fields), Some(12));
    }

    #[test]
    fn test_calculate_record_size_mixed_primitives() {
        let fields = FieldListBuilder::new_tight_packing()
            .add_field("a".to_string(), Type::I8) // offset 0, size 1, end 1
            .add_field("b".to_string(), Type::I32) // offset 1, size 4, end 5 (tight packing!)
            .add_field("c".to_string(), Type::F64) // offset 5, size 8, end 13 (tight packing!)
            .build();

        // With tight packing: 1 + 4 + 8 = 13 bytes
        assert_eq!(calculate_record_size(&fields), 13);
        assert_eq!(calculate_record_size_checked(&fields), Some(13));
    }

    #[test]
    fn test_calculate_record_size_field_order_independent() {
        // Test that field order doesn't affect size calculation with tight packing
        // Both [i32, f64] and [f64, i32] should be 12 bytes (4 + 8)
        let fields1 = FieldListBuilder::new_tight_packing()
            .add_field("a".to_string(), Type::I32)
            .add_field("b".to_string(), Type::F64)
            .build();

        let fields2 = FieldListBuilder::new_tight_packing()
            .add_field("b".to_string(), Type::F64)
            .add_field("a".to_string(), Type::I32)
            .build();

        // With tight packing, both should be 12 bytes
        assert_eq!(calculate_record_size(&fields1), 12);
        assert_eq!(calculate_record_size(&fields2), 12);
        assert_eq!(
            calculate_record_size(&fields1),
            calculate_record_size(&fields2)
        );
    }

    #[test]
    fn test_calculate_record_size_large_record() {
        // Create many fields to test no overflow in calculation
        let mut builder = FieldListBuilder::new();
        for i in 0..100 {
            builder = builder.add_field(format!("field_{}", i), Type::I32);
        }
        let fields = builder.build();

        // 100 i32 fields, each 4 bytes, aligned to 4-byte boundaries
        // Total size should be 100 * 4 = 400 bytes
        assert_eq!(calculate_record_size(&fields), 400);
        assert_eq!(calculate_record_size_checked(&fields), Some(400));
    }

    #[test]
    fn test_calculate_record_size_zero_sized_types() {
        // Note: We don't have zero-sized types in our Type enum yet
        // This test would need to be updated if we add them
        // For now, test with smallest types (1 byte)
        let fields = FieldListBuilder::new()
            .add_field("a".to_string(), Type::Bool) // size 1
            .add_field("b".to_string(), Type::I32) // size 4, aligned to 4 from offset 1
            .add_field("c".to_string(), Type::Bool) // size 1
            .build();

        // Offsets: bool at 0, i32 at 4 (aligned from 1), bool at 8
        // End offsets: 1, 8, 9
        // Max end offset: 9
        assert_eq!(calculate_record_size(&fields), 9);
        assert_eq!(calculate_record_size_checked(&fields), Some(9));
    }

    #[test]
    fn test_calculate_record_size_alignment_no_padding() {
        // Test that we don't insert padding between fields with tight packing
        // [u8, u64] should be 1 + 8 = 9 bytes, not 16
        let fields = FieldListBuilder::new_tight_packing()
            .add_field("a".to_string(), Type::U8) // offset 0, size 1
            .add_field("b".to_string(), Type::U64) // offset 1 (tight packing!), size 8
            .build();

        // With tight packing: 1 + 8 = 9 bytes
        assert_eq!(calculate_record_size(&fields), 9);
        assert_eq!(calculate_record_size_checked(&fields), Some(9));
    }

    #[test]
    fn test_calculate_record_size_overflow_panic() {
        // Test that calculate_record_size panics on overflow
        // Create a field with offset near usize::MAX
        let field = Field {
            name: "test".to_string(),
            ty: Type::I32,
            offset: usize::MAX - 3, // This will overflow when adding size 4
        };

        // The panic version should panic
        let result = std::panic::catch_unwind(|| {
            calculate_record_size(&[field.clone()]);
        });
        assert!(result.is_err());

        // The checked version should return None
        assert_eq!(calculate_record_size_checked(&[field]), None);
    }

    #[test]
    fn test_calculate_record_size_memory_layout_match() {
        // Test that calculated size matches what we'd allocate
        let fields = FieldListBuilder::new()
            .add_field("id".to_string(), Type::I32)
            .add_field("score".to_string(), Type::F64)
            .add_field("active".to_string(), Type::Bool)
            .build();

        let calculated_size = calculate_record_size(&fields);

        // Create a buffer with the calculated size
        let buffer = vec![0u8; calculated_size];
        assert_eq!(buffer.len(), calculated_size);

        // Verify we can access all field positions
        for field in &fields {
            assert!(
                field.offset() + field.size() <= calculated_size,
                "Field {} at offset {} with size {} exceeds buffer size {}",
                field.name(),
                field.offset(),
                field.size(),
                calculated_size
            );
        }
    }

    #[test]
    fn test_calculate_record_size_with_custom_field_offsets() {
        // Test with manually created fields (not using FieldListBuilder)
        let fields = vec![
            Field::builder("a".to_string(), Type::I32).build_with_offset(0),
            Field::builder("b".to_string(), Type::F64).build_with_offset(100), // Large gap
            Field::builder("c".to_string(), Type::Bool).build_with_offset(200),
        ];

        // Size should be based on furthest field end offset
        // Field c at offset 200 + size 1 = 201
        assert_eq!(calculate_record_size(&fields), 201);
        assert_eq!(calculate_record_size_checked(&fields), Some(201));
    }

    #[test]
    fn test_calculate_record_size_negative_offset_overflow() {
        // Test with fields that have very large offsets
        let field1 = Field {
            name: "field1".to_string(),
            ty: Type::U8,
            offset: usize::MAX / 2,
        };

        let field2 = Field {
            name: "field2".to_string(),
            ty: Type::U8,
            offset: usize::MAX / 2 + 1,
        };

        // This should not overflow in checked version
        let fields = vec![field1, field2];
        let result = calculate_record_size_checked(&fields);
        assert!(result.is_some());

        // Size should be (usize::MAX / 2 + 1) + 1
        let expected = (usize::MAX / 2 + 1) + 1;
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_field_end_offset_checked() {
        let field = Field::builder("test".to_string(), Type::I32).build_with_offset(100);

        // Normal case
        assert_eq!(field.end_offset_checked(), Some(104));
        assert_eq!(field.end_offset(), 104);

        // Test with field that would overflow
        let overflow_field = Field {
            name: "overflow".to_string(),
            ty: Type::I32,
            offset: usize::MAX - 3,
        };

        assert_eq!(overflow_field.end_offset_checked(), None);
    }

    #[test]
    #[should_panic(expected = "offset + size overflow")]
    fn test_field_end_offset_panic() {
        let field = Field {
            name: "overflow".to_string(),
            ty: Type::I32,
            offset: usize::MAX - 3,
        };

        // This should panic
        field.end_offset();
    }

    #[test]
    fn test_field_is_aligned() {
        // Aligned field
        let aligned = Field::builder("aligned".to_string(), Type::I32).build_with_offset(0);
        assert!(aligned.is_aligned());
        assert!(aligned.validate_alignment().is_ok());

        // Misaligned field (tight packing scenario)
        let misaligned = Field {
            name: "misaligned".to_string(),
            ty: Type::I32,
            offset: 1, // i32 requires 4-byte alignment
        };

        assert!(!misaligned.is_aligned());
        assert!(misaligned.validate_alignment().is_err());
    }

    #[test]
    fn test_calculate_record_size_with_normal_packing() {
        // Test with normal (aligned) packing
        let fields = FieldListBuilder::new()
            .add_field("a".to_string(), Type::U8) // offset 0, size 1
            .add_field("b".to_string(), Type::U64) // offset 8 (aligned from 1), size 8
            .build();

        // With normal packing: u8 at 0, u64 at 8 (aligned) = 16 bytes
        assert_eq!(calculate_record_size(&fields), 16);
        assert_eq!(calculate_record_size_checked(&fields), Some(16));

        // Verify fields are aligned
        for field in &fields {
            assert!(field.is_aligned());
            assert!(field.validate_alignment().is_ok());
        }
    }

    #[test]
    fn test_tight_packing_creates_misaligned_fields() {
        // Create fields with tight packing
        let fields = FieldListBuilder::new_tight_packing()
            .add_field("a".to_string(), Type::U8) // offset 0, size 1
            .add_field("b".to_string(), Type::U64) // offset 1 (tight packing!), size 8
            .build();

        // First field should be aligned (offset 0)
        assert!(fields[0].is_aligned());

        // Second field should NOT be aligned (offset 1 for u64)
        assert!(!fields[1].is_aligned());
        assert!(fields[1].validate_alignment().is_err());

        // Size should be 9 bytes with tight packing
        assert_eq!(calculate_record_size(&fields), 9);
    }
}
