//! Field definition within a table.

use crate::types::TypeLayout;

/// Field definition within a table.
#[derive(Debug, Clone)]
pub struct Field {
    /// Field name
    pub name: String,
    /// Byte offset within record
    pub offset: usize,
    /// Type identifier (e.g., "u64", "string", "3xf32")
    pub type_id: String,
    /// Field size in bytes (derived from type layout)
    pub size: usize,
    /// Field alignment requirement (derived from type layout)
    pub align: usize,
    /// Reference to type layout (cached for performance)
    pub layout: TypeLayout,
}

impl Field {
    /// Creates a new field with the given parameters.
    ///
    /// # Arguments
    /// * `name` - Field name
    /// * `type_id` - Type identifier
    /// * `layout` - Type layout for this field
    /// * `offset` - Byte offset within record
    ///
    /// # Returns
    /// A new Field instance.
    pub fn new(name: String, type_id: String, layout: TypeLayout, offset: usize) -> Self {
        Self {
            name,
            offset,
            type_id,
            size: layout.size,
            align: layout.align,
            layout,
        }
    }

    /// Returns the end offset of this field (offset + size).
    pub fn end_offset(&self) -> usize {
        self.offset + self.size
    }
}
