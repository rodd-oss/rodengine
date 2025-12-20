//! Validation methods for table schema and records.

use super::field::Field;
use crate::error::DbError;

/// Validates that all fields fit within the calculated record size.
///
/// # Arguments
/// * `fields` - Field definitions to validate
/// * `record_size` - Calculated record size in bytes
///
/// # Returns
/// `Result<(), DbError>` indicating success or validation failure.
pub(crate) fn validate_record_size(fields: &[Field], record_size: usize) -> Result<(), DbError> {
    for field in fields {
        let field_end = field
            .offset
            .checked_add(field.size)
            .ok_or(DbError::CapacityOverflow {
                operation: "field bounds calculation",
            })?;

        if field_end > record_size {
            return Err(DbError::FieldExceedsRecordSize {
                field: field.name.clone(),
                offset: field.offset,
                size: field.size,
                record_size,
            });
        }
    }
    Ok(())
}

/// Validates field alignment and overlapping fields.
///
/// # Arguments
/// * `fields` - Field definitions to validate
///
/// # Returns
/// `Result<(), DbError>` indicating success or validation failure.
pub(crate) fn validate_field_layout(fields: &[Field]) -> Result<(), DbError> {
    // Check field alignment
    for field in fields {
        if field.offset % field.align != 0 {
            return Err(DbError::DataCorruption(format!(
                "Field '{}' offset {} not aligned to {}",
                field.name, field.offset, field.align
            )));
        }
    }

    // Check for overlapping fields
    let mut ranges: Vec<(usize, usize)> = fields
        .iter()
        .map(|f| (f.offset, f.offset + f.size))
        .collect();
    ranges.sort_by_key(|&(start, _)| start);

    for i in 1..ranges.len() {
        if ranges[i - 1].1 > ranges[i].0 {
            return Err(DbError::DataCorruption(
                "Overlapping field ranges detected".to_string(),
            ));
        }
    }

    Ok(())
}

/// Calculates record size from field definitions.
///
/// Record size is the maximum of (field offset + field size) across all fields.
///
/// # Arguments
/// * `fields` - Field definitions
///
/// # Returns
/// `Result<usize, DbError>` containing the calculated record size.
pub(crate) fn calculate_record_size(fields: &[Field]) -> Result<usize, DbError> {
    let mut max_end = 0;

    for field in fields {
        let field_end = field
            .offset
            .checked_add(field.size)
            .ok_or(DbError::CapacityOverflow {
                operation: "record size calculation",
            })?;

        max_end = max_end.max(field_end);
    }

    Ok(max_end)
}

/// Aligns an offset to the given alignment.
pub(crate) fn align_offset(offset: usize, align: usize) -> usize {
    if align == 0 {
        return offset;
    }
    let remainder = offset % align;
    if remainder == 0 {
        offset
    } else {
        offset + (align - remainder)
    }
}
