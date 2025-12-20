//! Table schema and buffer management.
//!
//! Each table has:
//! - Fixed schema with field definitions
//! - Atomic buffer for lock-free operations
//! - Record ID sequence generator
//! - Optional relations to other tables

use std::sync::atomic::{AtomicU64, Ordering};

use crate::atomic_buffer::AtomicBuffer;
use crate::error::DbError;
use crate::types::TypeLayout;

use super::field::Field;
use super::relation::Relation;
use super::validation;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Table schema and buffer management.
#[derive(Debug)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Size of each record in bytes
    pub record_size: usize,
    /// Atomic buffer for lock-free read/write operations
    pub buffer: AtomicBuffer,
    /// Field definitions in declaration order
    pub fields: Vec<Field>,
    /// Foreign key relations to other tables
    pub relations: Vec<Relation>,
    /// Next record ID to assign (atomic counter)
    pub next_id: AtomicU64,
}

impl Table {
    /// Creates a new table with the given name and field definitions.
    ///
    /// # Arguments
    /// * `name` - Table name
    /// * `fields` - Field definitions
    /// * `initial_capacity` - Initial buffer capacity in records (default: 1024)
    ///
    /// # Returns
    /// `Result<Table, DbError>` containing the created table or an error.
    pub fn create(
        name: String,
        fields: Vec<Field>,
        initial_capacity: Option<usize>,
        max_buffer_size: usize,
    ) -> Result<Self, DbError> {
        // Validate fields have unique names
        let mut seen_names = std::collections::HashSet::new();
        for field in &fields {
            if !seen_names.insert(&field.name) {
                return Err(DbError::FieldAlreadyExists {
                    table: name.clone(),
                    field: field.name.clone(),
                });
            }
        }

        // Calculate record size from field offsets and sizes
        let record_size = validation::calculate_record_size(&fields)?;

        // Validate field offsets fit within record size
        validation::validate_record_size(&fields, record_size)?;

        // Validate field alignment and overlapping fields
        validation::validate_field_layout(&fields)?;

        // Set initial capacity (records -> bytes)
        let capacity_records = initial_capacity.unwrap_or(1024);
        let capacity_bytes =
            capacity_records
                .checked_mul(record_size)
                .ok_or(DbError::CapacityOverflow {
                    operation: "initial buffer allocation",
                })?;

        // Enforce max buffer size limit
        if capacity_bytes > max_buffer_size {
            return Err(DbError::MemoryLimitExceeded {
                requested: capacity_bytes,
                limit: max_buffer_size,
                table: name.clone(),
            });
        }

        Ok(Self {
            name,
            record_size,
            buffer: AtomicBuffer::new(capacity_bytes, record_size, max_buffer_size),
            fields,
            relations: Vec::new(),
            next_id: AtomicU64::new(1), // Start IDs at 1
        })
    }

    /// Calculates the byte offset for a field within a record.
    ///
    /// # Arguments
    /// * `field_name` - Name of the field
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the byte offset or an error.
    pub fn field_offset(&self, field_name: &str) -> Result<usize, DbError> {
        self.fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| f.offset)
            .ok_or_else(|| DbError::FieldNotFound {
                table: self.name.clone(),
                field: field_name.to_string(),
            })
    }

    /// Atomically increments and returns the next record ID.
    ///
    /// # Returns
    /// Next available record ID.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Returns the current next ID value without incrementing.
    pub fn current_next_id(&self) -> u64 {
        self.next_id.load(Ordering::Acquire)
    }

    /// Returns the number of records currently in the buffer.
    pub fn record_count(&self) -> usize {
        self.buffer.len() / self.record_size
    }

    /// Returns the byte offset for a record at the given index.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// Byte offset within the buffer where the record starts.
    pub fn record_offset(&self, record_index: usize) -> usize {
        self.buffer.record_offset(record_index)
    }

    /// Reads a record as raw bytes with lock-free access.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the start of the record
    /// - Length of the record (record_size)
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The record must be fully within buffer bounds
    /// - The caller must ensure proper alignment for the intended use
    pub fn read_record_raw(
        &self,
        record_index: usize,
    ) -> Result<
        (
            *const u8,
            usize,
            std::sync::Arc<crate::atomic_buffer::BufferStorage>,
        ),
        DbError,
    > {
        self.buffer.record_slice(record_index)
    }

    /// Reads a record as a byte slice with lock-free access.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// A tuple containing:
    /// - Byte slice of the record
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Performance
    /// - Completes within <1μs latency constraint
    /// - Zero allocations in hot path
    /// - Validates offset within buffer bounds
    pub fn read_record(
        &self,
        record_index: usize,
    ) -> Result<(&[u8], std::sync::Arc<crate::atomic_buffer::BufferStorage>), DbError> {
        let (ptr, len, arc) = self.buffer.record_slice(record_index)?;

        // SAFETY: The pointer is valid as long as the Arc is alive.
        // We create a slice that borrows from the Arc's data.
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        Ok((slice, arc))
    }

    /// Reads a record as a typed pointer with lock-free access.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// A tuple containing:
    /// - Typed raw pointer to the record
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - T must be properly aligned and match the record layout
    /// - The record must be fully within buffer bounds
    /// - T must be `#[repr(C, packed)]` and match field layout exactly
    pub unsafe fn read_record_ptr<T>(
        &self,
        record_index: usize,
    ) -> Result<
        (
            *const T,
            std::sync::Arc<crate::atomic_buffer::BufferStorage>,
        ),
        DbError,
    > {
        let (ptr, _, arc) = self.buffer.record_slice(record_index)?;
        Ok((ptr as *const T, arc))
    }

    /// Reads a field value from a record with lock-free access.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    /// * `field_name` - Name of the field to read
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the field data
    /// - Size of the field in bytes
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The field must be fully within the record bounds
    /// - The caller must ensure proper alignment for the field type
    pub fn read_field_raw(
        &self,
        record_index: usize,
        field_name: &str,
    ) -> Result<
        (
            *const u8,
            usize,
            std::sync::Arc<crate::atomic_buffer::BufferStorage>,
        ),
        DbError,
    > {
        let field = self
            .get_field(field_name)
            .ok_or_else(|| DbError::FieldNotFound {
                table: self.name.clone(),
                field: field_name.to_string(),
            })?;

        let record_offset = self.record_offset(record_index);
        let field_offset =
            record_offset
                .checked_add(field.offset)
                .ok_or(DbError::CapacityOverflow {
                    operation: "field offset calculation",
                })?;
        let field_end = field_offset
            .checked_add(field.size)
            .ok_or(DbError::CapacityOverflow {
                operation: "field end calculation",
            })?;

        self.buffer.slice(field_offset..field_end)
    }

    /// Reads a field value as a typed pointer with lock-free access.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    /// * `field_name` - Name of the field to read
    ///
    /// # Returns
    /// A tuple containing:
    /// - Typed raw pointer to the field
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - T must be properly aligned and match the field type
    /// - The field must be fully within the record bounds
    /// - The field type must match the registered type layout
    pub unsafe fn read_field_ptr<T>(
        &self,
        record_index: usize,
        field_name: &str,
    ) -> Result<
        (
            *const T,
            std::sync::Arc<crate::atomic_buffer::BufferStorage>,
        ),
        DbError,
    > {
        let (ptr, size, arc) = self.read_field_raw(record_index, field_name)?;

        // Verify size matches expected type size
        if size != std::mem::size_of::<T>() {
            return Err(DbError::TypeMismatch {
                expected: format!("{} bytes", std::mem::size_of::<T>()),
                got: format!("{} bytes", size),
            });
        }

        // Verify alignment
        let alignment = std::mem::align_of::<T>();
        if !(ptr as usize).is_multiple_of(alignment) {
            return Err(DbError::TypeMismatch {
                expected: format!("{} byte alignment", alignment),
                got: format!("{} address", ptr as usize),
            });
        }

        Ok((ptr as *const T, arc))
    }

    /// Creates a new record with the given serialized data.
    ///
    /// # Arguments
    /// * `data` - Serialized record data (must be exactly `record_size` bytes)
    ///
    /// # Returns
    /// `Result<u64, DbError>` containing the assigned record ID or an error.
    ///
    /// # Performance
    /// - Completes within <5μs latency constraint
    /// - One buffer clone allocation
    /// - Atomic swap without blocking readers
    pub fn create_record(&self, data: &[u8]) -> Result<u64, DbError> {
        // Validate data size matches record size
        if data.len() != self.record_size {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: data.len(),
                max: self.record_size,
            });
        }

        // Ensure capacity for new record (enforces memory limits)
        let current_len = self.buffer.len();
        let required_capacity = current_len + self.record_size;
        self.buffer
            .ensure_capacity(required_capacity)
            .map_err(|e| match e {
                DbError::MemoryLimitExceeded {
                    requested, limit, ..
                } => DbError::MemoryLimitExceeded {
                    requested,
                    limit,
                    table: self.name.clone(),
                },
                _ => e,
            })?;

        // Clone current buffer for modification (after potential growth)
        let mut new_buffer = self.buffer.load_full();

        // Append new record data
        new_buffer.extend_from_slice(data);

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer)?;

        // Return assigned ID (atomic increment)
        Ok(self.next_id())
    }

    /// Creates a new record from field values.
    ///
    /// # Arguments
    /// * `field_values` - Field values in the same order as table fields
    ///
    /// # Returns
    /// `Result<u64, DbError>` containing the assigned record ID or an error.
    ///
    /// # Performance
    /// - Completes within <5μs latency constraint
    /// - One buffer clone allocation
    /// - Atomic swap without blocking readers
    pub fn create_record_from_values(&self, field_values: &[&[u8]]) -> Result<u64, DbError> {
        // Validate field count matches schema
        if field_values.len() != self.fields.len() {
            return Err(DbError::TypeMismatch {
                expected: format!("{} fields", self.fields.len()),
                got: format!("{} fields", field_values.len()),
            });
        }

        // Serialize field values to byte array
        let mut data = Vec::with_capacity(self.record_size);

        for (field, field_data) in self.fields.iter().zip(field_values.iter()) {
            // Validate field data size matches field size
            if field_data.len() != field.size {
                return Err(DbError::TypeMismatch {
                    expected: format!("{} bytes for field '{}'", field.size, field.name),
                    got: format!("{} bytes", field_data.len()),
                });
            }

            // Ensure we're at the correct offset
            while data.len() < field.offset {
                data.push(0); // Pad with zeros
            }

            // Copy field data
            data.extend_from_slice(field_data);
        }

        // Ensure record is exactly the right size
        while data.len() < self.record_size {
            data.push(0);
        }

        self.create_record(&data)
    }

    /// Updates an existing record with new serialized data.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    /// * `data` - New serialized record data (must be exactly `record_size` bytes)
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    ///
    /// # Performance
    /// - Completes within <5μs latency constraint
    /// - One buffer clone allocation
    /// - Atomic swap without blocking readers
    /// - Failed writes discard cloned buffer without affecting live data
    pub fn update_record(&self, record_index: usize, data: &[u8]) -> Result<(), DbError> {
        // Validate data size matches record size
        if data.len() != self.record_size {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: data.len(),
                max: self.record_size,
            });
        }

        // Calculate record offset
        let offset = self.record_offset(record_index);

        // Clone current buffer for modification
        let mut new_buffer = self.buffer.load_full();

        // Validate offset is within buffer bounds
        let record_end = offset
            .checked_add(self.record_size)
            .ok_or(DbError::CapacityOverflow {
                operation: "record update",
            })?;
        if record_end > new_buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset,
                max: new_buffer.len().saturating_sub(self.record_size),
            });
        }

        // Update record data
        let slice = &mut new_buffer[offset..record_end];
        slice.copy_from_slice(data);

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer).map_err(|e| match e {
            DbError::MemoryLimitExceeded {
                requested, limit, ..
            } => DbError::MemoryLimitExceeded {
                requested,
                limit,
                table: self.name.clone(),
            },
            _ => e,
        })?;

        Ok(())
    }

    /// Partially updates specific fields within a record.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    /// * `field_updates` - Map of field names to serialized field data
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    ///
    /// # Performance
    /// - Completes within <5μs latency constraint
    /// - One buffer clone allocation
    /// - Only modifies specified fields
    pub fn partial_update(
        &self,
        record_index: usize,
        field_updates: &[(&str, &[u8])],
    ) -> Result<(), DbError> {
        // Calculate record offset
        let offset = self.record_offset(record_index);

        // Clone current buffer for modification
        let mut new_buffer = self.buffer.load_full();

        // Validate offset is within buffer bounds
        let record_end = offset
            .checked_add(self.record_size)
            .ok_or(DbError::CapacityOverflow {
                operation: "partial update",
            })?;
        if record_end > new_buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset,
                max: new_buffer.len().saturating_sub(self.record_size),
            });
        }

        // Apply each field update
        for (field_name, field_data) in field_updates {
            let field = self
                .get_field(field_name)
                .ok_or_else(|| DbError::FieldNotFound {
                    table: self.name.clone(),
                    field: field_name.to_string(),
                })?;

            // Validate field data size
            if field_data.len() != field.size {
                return Err(DbError::InvalidOffset {
                    table: self.name.clone(),
                    offset: field_data.len(),
                    max: field.size,
                });
            }

            // Calculate field offset within record
            let field_offset =
                offset
                    .checked_add(field.offset)
                    .ok_or(DbError::CapacityOverflow {
                        operation: "field offset calculation",
                    })?;

            // Update field data
            let field_end =
                field_offset
                    .checked_add(field.size)
                    .ok_or(DbError::CapacityOverflow {
                        operation: "field end calculation",
                    })?;
            let slice = &mut new_buffer[field_offset..field_end];
            slice.copy_from_slice(field_data);
        }

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer).map_err(|e| match e {
            DbError::MemoryLimitExceeded {
                requested, limit, ..
            } => DbError::MemoryLimitExceeded {
                requested,
                limit,
                table: self.name.clone(),
            },
            _ => e,
        })?;

        Ok(())
    }

    /// Deletes a record by setting a deletion flag (soft delete).
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    /// * `deleted_field_name` - Name of the boolean field to use as deletion flag
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    ///
    /// # Note
    /// This is a soft delete that marks the record as deleted but doesn't remove it.
    /// Use `compact_table()` to physically remove deleted records.
    pub fn delete_record(
        &self,
        record_index: usize,
        deleted_field_name: &str,
    ) -> Result<(), DbError> {
        // Find the deletion flag field
        let field = self
            .get_field(deleted_field_name)
            .ok_or_else(|| DbError::FieldNotFound {
                table: self.name.clone(),
                field: deleted_field_name.to_string(),
            })?;

        // Verify it's a boolean field (size = 1)
        if field.size != 1 {
            return Err(DbError::TypeMismatch {
                expected: "bool (size=1)".to_string(),
                got: format!("size={}", field.size),
            });
        }

        // Calculate record offset
        let offset = self.record_offset(record_index);

        // Clone current buffer for modification
        let mut new_buffer = self.buffer.load_full();

        // Validate offset is within buffer bounds
        let record_end = offset
            .checked_add(self.record_size)
            .ok_or(DbError::CapacityOverflow {
                operation: "record deletion",
            })?;
        if record_end > new_buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset,
                max: new_buffer.len().saturating_sub(self.record_size),
            });
        }

        // Calculate field offset within record
        let field_offset = offset
            .checked_add(field.offset)
            .ok_or(DbError::CapacityOverflow {
                operation: "field offset calculation",
            })?;

        // Set deletion flag to true (non-zero)
        new_buffer[field_offset] = 1;

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer).map_err(|e| match e {
            DbError::MemoryLimitExceeded {
                requested, limit, ..
            } => DbError::MemoryLimitExceeded {
                requested,
                limit,
                table: self.name.clone(),
            },
            _ => e,
        })?;

        Ok(())
    }

    /// Compacts the table by removing deleted records.
    ///
    /// # Arguments
    /// * `deleted_field_name` - Name of the boolean field used as deletion flag
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the number of records removed.
    ///
    /// # Performance
    /// - O(n) operation where n is number of records
    /// - One buffer clone allocation
    /// - Maintains record ID sequence
    pub fn compact_table(&self, deleted_field_name: &str) -> Result<usize, DbError> {
        // Find the deletion flag field
        let field = self
            .get_field(deleted_field_name)
            .ok_or_else(|| DbError::FieldNotFound {
                table: self.name.clone(),
                field: deleted_field_name.to_string(),
            })?;

        // Verify it's a boolean field (size = 1)
        if field.size != 1 {
            return Err(DbError::TypeMismatch {
                expected: "bool (size=1)".to_string(),
                got: format!("size={}", field.size),
            });
        }

        // Clone current buffer for modification
        let current_buffer = self.buffer.load_full();
        let record_count = current_buffer.len() / self.record_size;

        // Build new buffer with only non-deleted records
        let mut new_buffer = Vec::with_capacity(current_buffer.capacity());
        let mut records_removed = 0;

        for i in 0..record_count {
            let offset = i
                .checked_mul(self.record_size)
                .ok_or(DbError::CapacityOverflow {
                    operation: "record offset calculation",
                })?;
            let field_offset =
                offset
                    .checked_add(field.offset)
                    .ok_or(DbError::CapacityOverflow {
                        operation: "field offset calculation",
                    })?;

            // Check if record is deleted
            if current_buffer[field_offset] == 0 {
                // Record not deleted, copy it
                let record_end =
                    offset
                        .checked_add(self.record_size)
                        .ok_or(DbError::CapacityOverflow {
                            operation: "record slice calculation",
                        })?;
                let record_slice = &current_buffer[offset..record_end];
                new_buffer.extend_from_slice(record_slice);
            } else {
                records_removed += 1;
            }
        }

        // Only swap if records were actually removed
        if records_removed > 0 {
            self.buffer.store(new_buffer).map_err(|e| match e {
                DbError::MemoryLimitExceeded {
                    requested, limit, ..
                } => DbError::MemoryLimitExceeded {
                    requested,
                    limit,
                    table: self.name.clone(),
                },
                _ => e,
            })?;
        }

        Ok(records_removed)
    }

    /// Returns the field definition for the given field name.
    ///
    /// # Arguments
    /// * `field_name` - Name of the field
    ///
    /// # Returns
    /// `Option<&Field>` containing the field definition if found.
    pub fn get_field(&self, field_name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == field_name)
    }

    /// Adds a relation to another table.
    ///
    /// # Arguments
    /// * `relation` - Relation to add
    pub fn add_relation(&mut self, relation: Relation) {
        self.relations.push(relation);
    }

    /// Removes a relation by target table name.
    ///
    /// # Arguments
    /// * `to_table` - Name of the target table
    ///
    /// # Returns
    /// `bool` indicating whether a relation was removed.
    pub fn remove_relation(&mut self, to_table: &str) -> bool {
        if let Some(pos) = self.relations.iter().position(|r| r.to_table == to_table) {
            self.relations.remove(pos);
            true
        } else {
            false
        }
    }

    /// Adds a new field to the table schema.
    ///
    /// This operation requires rebuilding the table buffer with updated field offsets.
    ///
    /// # Arguments
    /// * `name` - Field name
    /// * `type_id` - Type identifier
    /// * `layout` - Type layout for the field
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the field offset or an error.
    pub fn add_field(
        &mut self,
        name: String,
        type_id: String,
        layout: TypeLayout,
    ) -> Result<usize, DbError> {
        // Validate no duplicate field name
        if self.fields.iter().any(|f| f.name == name) {
            return Err(DbError::FieldAlreadyExists {
                table: self.name.clone(),
                field: name,
            });
        }

        // Validate type_id matches layout
        if layout.type_id != type_id {
            return Err(DbError::TypeMismatch {
                expected: layout.type_id.clone(),
                got: type_id,
            });
        }

        // Calculate offset for new field (end of last field)
        let offset = if let Some(last_field) = self.fields.last() {
            // Align offset to field alignment
            validation::align_offset(last_field.end_offset(), layout.align)
        } else {
            0
        };

        // Create new field
        let field = Field::new(name, type_id, layout, offset);

        // Recalculate record size with new field
        let new_record_size = self.calculate_record_size_with_new_field(&field)?;

        // Validate field fits within new record size
        if field.end_offset() > new_record_size {
            return Err(DbError::FieldExceedsRecordSize {
                field: field.name.clone(),
                offset: field.offset,
                size: field.size,
                record_size: new_record_size,
            });
        }

        // Update table schema
        self.fields.push(field);
        self.record_size = new_record_size;

        // Note: In a real implementation, we would need to rebuild the buffer
        // to accommodate the new field for existing records. This is a blocking
        // operation that requires rewriting all records.

        Ok(offset)
    }

    /// Removes a field from the table schema.
    ///
    /// This operation requires rebuilding the table buffer with updated field offsets.
    ///
    /// # Arguments
    /// * `field_name` - Name of the field to remove
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn remove_field(&mut self, field_name: &str) -> Result<(), DbError> {
        // Find field position
        let pos = self
            .fields
            .iter()
            .position(|f| f.name == field_name)
            .ok_or_else(|| DbError::FieldNotFound {
                table: self.name.clone(),
                field: field_name.to_string(),
            })?;

        // Remove the field
        self.fields.remove(pos);

        // Recalculate record size and rebuild field offsets
        self.rebuild_field_offsets()?;

        // Note: In a real implementation, we would need to rebuild the buffer
        // to remove the field from existing records. This is a blocking
        // operation that requires rewriting all records.

        Ok(())
    }

    /// Rebuilds field offsets after schema changes.
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    fn rebuild_field_offsets(&mut self) -> Result<(), DbError> {
        let mut offset = 0;
        let mut max_end = 0;

        for field in &mut self.fields {
            // Align offset to field alignment
            offset = validation::align_offset(offset, field.align);
            field.offset = offset;

            let field_end = offset
                .checked_add(field.size)
                .ok_or(DbError::CapacityOverflow {
                    operation: "field offset calculation",
                })?;

            max_end = max_end.max(field_end);
            offset = field_end;
        }

        self.record_size = max_end;
        Ok(())
    }

    /// Calculates record size with a new field added.
    ///
    /// # Arguments
    /// * `new_field` - The new field to add
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the new record size.
    fn calculate_record_size_with_new_field(&self, new_field: &Field) -> Result<usize, DbError> {
        let mut max_end = new_field.end_offset();

        for field in &self.fields {
            max_end = max_end.max(field.end_offset());
        }

        Ok(max_end)
    }
}
