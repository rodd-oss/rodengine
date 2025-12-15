//! Shared types and utilities for the database system.
//!
//! This crate defines field types, table schemas, and other shared data structures.
//!
//! # Key Types
//!
//! - [`Type`]: Enum representing primitive field types (i8, i32, f64, bool, etc.)
//! - [`Field`]: Struct representing a field in a table schema with name, type, and offset
//! - [`FieldBuilder`]: Builder for creating individual [`Field`] instances
//! - [`FieldListBuilder`]: Builder for creating multiple fields with automatic offset calculation
//! - [`FieldError`]: Error type for field creation and validation
//! - [`calculate_record_size`]: Function to calculate total record size from field list
//! - [`calculate_record_size_checked`]: Overflow-safe version of record size calculation
//!
//! # TODO: Document safety considerations
//! Add documentation about overflow protection, alignment requirements, and
//! field name validation policies.
//!
//! # Examples
//!
//! ```
//! use db_types::{Type, Field, FieldListBuilder, align_offset, calculate_record_size};
//!
//! // Create a single field with proper offset calculation
//! let field = Field::builder("age".to_string(), Type::I32)
//!     .build_with_offset(0);
//!
//! assert_eq!(field.name(), "age");
//! assert_eq!(field.ty(), Type::I32);
//! assert_eq!(field.offset(), 0);
//! assert_eq!(field.size(), 4);
//!
//! // Create multiple fields with automatic offset calculation
//! let fields = FieldListBuilder::new()
//!     .add_field("id".to_string(), Type::I32)
//!     .add_field("score".to_string(), Type::F64)
//!     .add_field("active".to_string(), Type::Bool)
//!     .build();
//!
//! assert_eq!(fields.len(), 3);
//! assert_eq!(fields[0].offset(), 0);    // id at offset 0
//! assert_eq!(fields[1].offset(), 8);    // score at offset 8 (aligned from 4)
//! assert_eq!(fields[2].offset(), 16);   // active at offset 16
//!
//! // Calculate record size
//! let size = calculate_record_size(&fields);
//! assert_eq!(size, 17);
//!
//! // Calculate aligned offset
//! let offset = align_offset(1, Type::I64);
//! assert_eq!(offset, Some(8)); // Aligned from 1 to 8
//!
//! // Validate field alignment
//! assert!(fields[0].validate_alignment().is_ok());
//! ```

pub mod field;
pub mod table;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
