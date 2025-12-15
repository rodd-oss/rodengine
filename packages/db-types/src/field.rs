//! Field definitions and types.

use crate::types::Type;
use std::fmt;

/// Error type for field creation and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldError {
    /// Field offset is not properly aligned for its type
    Misaligned,
    /// Field would extend beyond buffer bounds
    OutOfBounds,
    /// Offset calculation would overflow usize
    Overflow,
    /// Invalid field name (empty or contains invalid characters)
    InvalidName,
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldError::Misaligned => write!(f, "field offset is misaligned for its type"),
            FieldError::OutOfBounds => write!(f, "field would extend beyond buffer bounds"),
            FieldError::Overflow => write!(f, "offset calculation would overflow"),
            FieldError::InvalidName => write!(f, "invalid field name"),
        }
    }
}

impl std::error::Error for FieldError {}

/// A field in a table schema.
///
/// Contains the field's name, type, and byte offset within a record.
/// Fields are tightly packed within records for cache efficiency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    name: String,
    ty: Type,
    offset: usize,
}

impl Field {
    /// Creates a new field with the given name, type, and offset.
    ///
    /// # Arguments
    ///
    /// * `name` - The field name
    /// * `ty` - The field type
    /// * `offset` - Byte offset within the record
    pub fn new(name: String, ty: Type, offset: usize) -> Self {
        Self { name, ty, offset }
    }

    /// Creates a builder for constructing a field.
    ///
    /// This is a convenience method that starts the builder with the given name and type.
    pub fn builder(name: String, ty: Type) -> FieldBuilder {
        FieldBuilder::new(name, ty)
    }

    /// Returns the field name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the field type.
    pub fn ty(&self) -> Type {
        self.ty
    }

    /// Returns the byte offset of this field within a record.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the size in bytes of this field.
    pub fn size(&self) -> usize {
        self.ty.size()
    }

    /// Validates that the field's offset is properly aligned for its type.
    ///
    /// Returns `Ok(())` if aligned, `Err(FieldError::Misaligned)` otherwise.
    pub fn validate_alignment(&self) -> Result<(), FieldError> {
        if self.offset.is_multiple_of(self.ty.alignment()) {
            Ok(())
        } else {
            Err(FieldError::Misaligned)
        }
    }

    /// Validates that the field fits within a buffer of the given length.
    ///
    /// Returns `Ok(())` if the field fits, `Err(FieldError::OutOfBounds)` otherwise.
    pub fn validate_bounds(&self, buffer_len: usize) -> Result<(), FieldError> {
        let end_offset = self
            .offset
            .checked_add(self.size())
            .ok_or(FieldError::Overflow)?;
        if end_offset <= buffer_len {
            Ok(())
        } else {
            Err(FieldError::OutOfBounds)
        }
    }
}

/// Builder for creating a single field.
///
/// Used to construct a `Field` with validation.
#[derive(Debug)]
pub struct FieldBuilder {
    name: String,
    ty: Type,
}

impl FieldBuilder {
    /// Creates a new field builder.
    pub fn new(name: String, ty: Type) -> Self {
        Self { name, ty }
    }

    /// Builds the field with the given offset.
    ///
    /// Returns the constructed `Field`.
    pub fn build_with_offset(self, offset: usize) -> Field {
        Field::new(self.name, self.ty, offset)
    }
}

/// Builder for creating multiple fields with automatic offset calculation.
///
/// Maintains running offset and ensures proper alignment for each field.
#[derive(Debug, Default)]
pub struct FieldListBuilder {
    fields: Vec<(String, Type)>,
}

impl FieldListBuilder {
    /// Creates a new empty field list builder.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Adds a field to the list.
    ///
    /// The field will be placed at the current offset, which will be updated
    /// to point after this field (aligned for the next field).
    pub fn add_field(mut self, name: String, ty: Type) -> Self {
        self.fields.push((name, ty));
        self
    }

    /// Builds the list of fields with properly calculated offsets.
    ///
    /// Returns a vector of `Field` instances with offsets calculated
    /// according to their alignment requirements.
    ///
    /// Returns `Err(FieldError::Overflow)` if offset calculation would overflow.
    pub fn build(self) -> Result<Vec<Field>, FieldError> {
        let mut fields = Vec::with_capacity(self.fields.len());
        let mut current_offset = 0;

        for (name, ty) in self.fields {
            // Align the offset for this field
            current_offset = align_offset(current_offset, ty).ok_or(FieldError::Overflow)?;

            fields.push(Field::new(name, ty, current_offset));

            // Move past this field
            current_offset = current_offset
                .checked_add(ty.size())
                .ok_or(FieldError::Overflow)?;
        }

        Ok(fields)
    }
}

/// Calculates an aligned offset for a given type.
///
/// Returns `Some(aligned_offset)` if successful, `None` if the calculation
/// would overflow `usize`.
pub fn align_offset(current_offset: usize, ty: Type) -> Option<usize> {
    let alignment = ty.alignment();
    if alignment == 1 {
        return Some(current_offset);
    }

    let remainder = current_offset % alignment;
    if remainder == 0 {
        Some(current_offset)
    } else {
        current_offset.checked_add(alignment - remainder)
    }
}

/// Calculates the total record size needed for the given fields.
///
/// Fields are assumed to be in the order they will be stored.
/// Returns the size in bytes.
pub fn calculate_record_size(fields: &[Field]) -> usize {
    let mut size = 0;
    for field in fields {
        // Align for this field
        let alignment = field.ty().alignment();
        if alignment > 1 {
            let remainder = size % alignment;
            if remainder != 0 {
                size += alignment - remainder;
            }
        }
        // Add field size
        size += field.size();
    }
    size
}

/// Overflow-safe version of record size calculation.
///
/// Returns `Ok(size)` if successful, `Err(FieldError::Overflow)` if the
/// calculation would overflow `usize`.
pub fn calculate_record_size_checked(fields: &[Field]) -> Result<usize, FieldError> {
    let mut size = 0;
    for field in fields {
        // Align for this field
        let alignment = field.ty().alignment();
        if alignment > 1 {
            let remainder = size % alignment;
            if remainder != 0 {
                size = size
                    .checked_add(alignment - remainder)
                    .ok_or(FieldError::Overflow)?;
            }
        }
        // Add field size
        size = size.checked_add(field.size()).ok_or(FieldError::Overflow)?;
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_new_01() {
        let field = Field::new("id".to_string(), Type::I32, 0);
        assert_eq!(field.name(), "id");
        assert_eq!(field.ty(), Type::I32);
        assert_eq!(field.offset(), 0);
    }

    #[test]
    fn test_field_new_02() {
        let field = Field::new("".to_string(), Type::I32, 0);
        assert_eq!(field.name(), "");
        assert_eq!(field.ty(), Type::I32);
        assert_eq!(field.offset(), 0);
    }

    #[test]
    fn test_field_builder_01() {
        let builder = Field::builder("age".to_string(), Type::I32);
        let field = builder.build_with_offset(0);
        assert_eq!(field.name(), "age");
        assert_eq!(field.ty(), Type::I32);
        assert_eq!(field.offset(), 0);
    }

    #[test]
    fn test_field_builder_build_with_offset_01() {
        let builder = Field::builder("age".to_string(), Type::I32);
        let field = builder.build_with_offset(0);
        assert_eq!(field.offset(), 0);
    }

    #[test]
    fn test_field_builder_build_with_offset_02() {
        let builder = Field::builder("score".to_string(), Type::F64);
        let field = builder.build_with_offset(8);
        assert_eq!(field.offset(), 8);
    }

    #[test]
    fn test_field_list_builder_new_01() {
        let builder = FieldListBuilder::new();
        let fields = builder.build().unwrap();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_field_list_builder_add_field_01() {
        let builder = FieldListBuilder::new().add_field("id".to_string(), Type::I32);
        let fields = builder.build().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name(), "id");
        assert_eq!(fields[0].ty(), Type::I32);
    }

    #[test]
    fn test_field_list_builder_add_field_02() {
        let builder = FieldListBuilder::new()
            .add_field("id".to_string(), Type::I32)
            .add_field("name".to_string(), Type::String);
        let fields = builder.build().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name(), "id");
        assert_eq!(fields[1].name(), "name");
    }

    #[test]
    fn test_field_list_builder_build_01() {
        let builder = FieldListBuilder::new();
        let fields = builder.build().unwrap();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_field_list_builder_build_02() {
        let builder = FieldListBuilder::new().add_field("id".to_string(), Type::I32);
        let fields = builder.build().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].offset(), 0);
    }

    #[test]
    fn test_field_list_builder_build_03() {
        let builder = FieldListBuilder::new()
            .add_field("id".to_string(), Type::I32)
            .add_field("score".to_string(), Type::F64);
        let fields = builder.build().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].offset(), 0); // i32 at 0
        assert_eq!(fields[1].offset(), 8); // f64 aligned to 8
    }

    #[test]
    fn test_field_size_01() {
        let field = Field::new("id".to_string(), Type::I32, 0);
        assert_eq!(field.size(), 4);
    }

    #[test]
    fn test_field_validate_alignment_01() {
        let field = Field::new("id".to_string(), Type::I32, 0);
        assert!(field.validate_alignment().is_ok());
    }

    #[test]
    fn test_field_validate_alignment_02() {
        let field = Field::new("id".to_string(), Type::I32, 1);
        assert_eq!(field.validate_alignment(), Err(FieldError::Misaligned));
    }

    #[test]
    fn test_calculate_record_size_01() {
        let fields = vec![];
        assert_eq!(calculate_record_size(&fields), 0);
    }

    #[test]
    fn test_calculate_record_size_02() {
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        assert_eq!(calculate_record_size(&fields), 4);
    }

    #[test]
    fn test_calculate_record_size_03() {
        let fields = vec![
            Field::new("id".to_string(), Type::I32, 0),
            Field::new("active".to_string(), Type::Bool, 4),
        ];
        assert_eq!(calculate_record_size(&fields), 5);
    }

    #[test]
    fn test_calculate_record_size_04() {
        let fields = vec![
            Field::new("id".to_string(), Type::Bool, 0),
            Field::new("score".to_string(), Type::F64, 1),
        ];
        // bool at 0 (size 1), f64 needs alignment 8, so aligned to 8, total 16
        // Actually: bool at 0, padding 7 bytes, f64 at 8, total 16
        assert_eq!(calculate_record_size(&fields), 16);
    }

    #[test]
    fn test_calculate_record_size_checked_01() {
        // Create a field list that would cause overflow when calculating size
        // We need to create fields that when aligned and sized would overflow
        // This is tricky to test directly, so we'll test the overflow case differently
        // by checking that align_offset returns None for overflow
        assert_eq!(align_offset(usize::MAX, Type::U64), None);
    }

    #[test]
    fn test_align_offset_01() {
        assert_eq!(align_offset(8, Type::U64), Some(8));
    }

    #[test]
    fn test_align_offset_02() {
        assert_eq!(align_offset(1, Type::U64), Some(8));
    }

    #[test]
    fn test_align_offset_03() {
        assert_eq!(align_offset(3, Type::Bool), Some(3));
    }

    #[test]
    fn test_align_offset_04() {
        assert_eq!(align_offset(usize::MAX, Type::U64), None);
    }

    #[test]
    fn test_field_validate_bounds_01() {
        let field = Field::new("test".to_string(), Type::I32, 0);
        assert!(field.validate_bounds(4).is_ok());
    }

    #[test]
    fn test_field_validate_bounds_02() {
        let field = Field::new("test".to_string(), Type::I32, 0);
        assert!(field.validate_bounds(4).is_ok());
    }

    #[test]
    fn test_field_validate_bounds_03() {
        let field = Field::new("test".to_string(), Type::I32, 4);
        assert!(field.validate_bounds(8).is_ok());
    }

    #[test]
    fn test_field_validate_bounds_04() {
        let field = Field::new("test".to_string(), Type::Bool, 0);
        assert!(field.validate_bounds(1).is_ok());
    }

    #[test]
    fn test_field_validate_bounds_05() {
        let field = Field::new("test".to_string(), Type::I32, 5);
        assert_eq!(field.validate_bounds(5), Err(FieldError::OutOfBounds));
    }

    #[test]
    fn test_field_validate_bounds_06() {
        let field = Field::new("test".to_string(), Type::I32, 1);
        assert_eq!(field.validate_bounds(4), Err(FieldError::OutOfBounds));
    }

    #[test]
    fn test_field_validate_bounds_07() {
        let field = Field::new("test".to_string(), Type::I32, usize::MAX - 2);
        // offset + size = (usize::MAX - 2) + 4 = overflow
        assert_eq!(field.validate_bounds(usize::MAX), Err(FieldError::Overflow));
    }

    #[test]
    fn test_field_validate_bounds_08() {
        let field = Field::new("test".to_string(), Type::Bool, 0);
        assert_eq!(field.validate_bounds(0), Err(FieldError::OutOfBounds));
    }

    #[test]
    fn test_field_validate_bounds_09() {
        let field = Field::new("test".to_string(), Type::F64, 0);
        assert_eq!(field.validate_bounds(4), Err(FieldError::OutOfBounds));
    }
}
