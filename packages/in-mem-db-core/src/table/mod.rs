//! Table schema, field definitions, and relation management.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::atomic_buffer::AtomicBuffer;
use crate::error::DbError;
use crate::types::TypeLayout;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

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

/// Relation between tables for foreign key references.
#[derive(Debug, Clone)]
pub struct Relation {
    /// Name of the target table
    pub to_table: String,
    /// Field name in source table
    pub from_field: String,
    /// Field name in target table
    pub to_field: String,
}

/// Table schema and buffer management.
///
/// Each table has:
/// - Fixed schema with field definitions
/// - Atomic buffer for lock-free operations
/// - Record ID sequence generator
/// - Optional relations to other tables
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
        let record_size = Self::calculate_record_size(&fields)?;

        // Validate field offsets fit within record size
        Self::validate_record_size(&fields, record_size)?;

        // Validate field alignment and overlapping fields
        Self::validate_field_layout(&fields)?;

        // Set initial capacity (records -> bytes)
        let capacity_records = initial_capacity.unwrap_or(1024);
        let capacity_bytes =
            capacity_records
                .checked_mul(record_size)
                .ok_or(DbError::CapacityOverflow {
                    operation: "initial buffer allocation",
                })?;

        Ok(Self {
            name,
            record_size,
            buffer: AtomicBuffer::new(capacity_bytes, record_size),
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

    /// Validates that all fields fit within the calculated record size.
    ///
    /// # Arguments
    /// * `fields` - Field definitions to validate
    /// * `record_size` - Calculated record size in bytes
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or validation failure.
    pub(crate) fn validate_record_size(
        fields: &[Field],
        record_size: usize,
    ) -> Result<(), DbError> {
        for field in fields {
            let field_end =
                field
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
            let field_end =
                field
                    .offset
                    .checked_add(field.size)
                    .ok_or(DbError::CapacityOverflow {
                        operation: "record size calculation",
                    })?;

            max_end = max_end.max(field_end);
        }

        Ok(max_end)
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
        &'static str,
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
    ) -> Result<(&[u8], std::sync::Arc<crate::atomic_buffer::BufferStorage>), &'static str> {
        let (ptr, len, arc) = self.buffer.record_slice(record_index)?;

        // SAFETY: The pointer is valid as long as the Arc is alive.
        // We create a slice that borrows from the Arc's data.
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        Ok((slice, arc))
    }

    /// Iterates over records in parallel using Rayon.
    ///
    /// # Arguments
    /// * `f` - Closure that processes each record chunk and returns a result
    ///
    /// # Returns
    /// `Result<Vec<R>, DbError>` containing the collected results from the closure.
    ///
    /// # Notes
    /// - Requires the `parallel` feature to be enabled.
    /// - The closure receives a byte slice for the record and its index.
    /// - The buffer is loaded once and shared across all parallel tasks.
    /// - Records are processed in contiguous chunks aligned to record_size.
    /// - Chunk boundaries are aligned to 64-byte cache lines to prevent false sharing.
    #[cfg(feature = "parallel")]
    pub fn par_iter_records<F, R>(&self, f: F) -> Result<Vec<R>, DbError>
    where
        F: Fn(&[u8], usize) -> R + Send + Sync,
        R: Send,
    {
        const CACHE_LINE_SIZE: usize = 64;

        /// Greatest common divisor using Euclidean algorithm.
        fn gcd(a: usize, b: usize) -> usize {
            let mut a = a;
            let mut b = b;
            while b != 0 {
                let temp = b;
                b = a % b;
                a = temp;
            }
            a
        }

        /// Least common multiple.
        fn lcm(a: usize, b: usize) -> usize {
            if a == 0 || b == 0 {
                0
            } else {
                a / gcd(a, b) * b
            }
        }

        let buffer = self.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_size = self.record_size;
        let total_bytes = buffer.len();

        if total_bytes == 0 {
            return Ok(Vec::new());
        }

        // Ensure buffer length is multiple of record size
        if total_bytes % record_size != 0 {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: total_bytes,
                max: total_bytes.saturating_sub(record_size),
            });
        }

        let record_count = total_bytes / record_size;
        let base_ptr = buffer.as_ptr() as usize;
        let aligned_offset = align_offset(base_ptr, CACHE_LINE_SIZE) - base_ptr;

        // If aligned offset is beyond buffer length, we have no aligned region
        if aligned_offset >= total_bytes {
            // Entire buffer fits before first cache line boundary, process sequentially
            let results: Vec<R> = (0..record_count)
                .map(|idx| {
                    let start = idx * record_size;
                    let slice = &buffer_slice[start..start + record_size];
                    f(slice, idx)
                })
                .collect();
            return Ok(results);
        }

        // Ensure aligned offset is multiple of record size
        let aligned_record_offset = align_offset(aligned_offset, record_size);
        if aligned_record_offset >= total_bytes {
            // Aligned region starts beyond buffer, process sequentially
            let results: Vec<R> = (0..record_count)
                .map(|idx| {
                    let start = idx * record_size;
                    let slice = &buffer_slice[start..start + record_size];
                    f(slice, idx)
                })
                .collect();
            return Ok(results);
        }

        // Process prefix records (before aligned region) sequentially
        let prefix_record_count = aligned_record_offset / record_size;
        let mut results: Vec<R> = Vec::with_capacity(record_count);
        for idx in 0..prefix_record_count {
            let start = idx * record_size;
            let slice = &buffer_slice[start..start + record_size];
            results.push(f(slice, idx));
        }

        // Aligned region
        let aligned_slice = &buffer_slice[aligned_record_offset..];
        let aligned_bytes = aligned_slice.len();
        let _aligned_record_count = aligned_bytes / record_size;

        // Calculate chunk size that is multiple of both record_size and cache line size
        let chunk_size = lcm(record_size, CACHE_LINE_SIZE);
        if chunk_size == 0 {
            // Should not happen since record_size > 0 and CACHE_LINE_SIZE > 0
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: 0,
                max: total_bytes.saturating_sub(record_size),
            });
        }

        // Process aligned region in parallel chunks
        let chunk_results: Vec<Vec<R>> = aligned_slice
            .par_chunks_exact(chunk_size)
            .enumerate()
            .map(|(chunk_idx, chunk)| {
                let start_record_idx = prefix_record_count + (chunk_idx * chunk_size) / record_size;
                let chunk_record_count = chunk_size / record_size;
                let mut chunk_results = Vec::with_capacity(chunk_record_count);
                for sub_idx in 0..chunk_record_count {
                    let record_idx = start_record_idx + sub_idx;
                    let start = sub_idx * record_size;
                    let slice = &chunk[start..start + record_size];
                    chunk_results.push(f(slice, record_idx));
                }
                chunk_results
            })
            .collect();

        // Flatten chunk results in order
        for chunk_vec in chunk_results {
            results.extend(chunk_vec);
        }

        // Process suffix records (after last full chunk) sequentially
        let processed_records =
            prefix_record_count + (aligned_bytes / chunk_size) * (chunk_size / record_size);
        for idx in processed_records..record_count {
            let start = idx * record_size;
            let slice = &buffer_slice[start..start + record_size];
            results.push(f(slice, idx));
        }

        Ok(results)
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
        &'static str,
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
        &'static str,
    > {
        let field = self.get_field(field_name).ok_or("field not found")?;

        let record_offset = self.record_offset(record_index);
        let field_offset = record_offset + field.offset;
        let field_end = field_offset + field.size;

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
        &'static str,
    > {
        let (ptr, size, arc) = self.read_field_raw(record_index, field_name)?;

        // Verify size matches expected type size
        if size != std::mem::size_of::<T>() {
            return Err("field size does not match type size");
        }

        // Verify alignment
        let alignment = std::mem::align_of::<T>();
        if !(ptr as usize).is_multiple_of(alignment) {
            return Err("field is not properly aligned for type");
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

        // Clone current buffer for modification
        let mut new_buffer = self.buffer.load_full();

        // Ensure capacity for new record
        let required_capacity = new_buffer.len() + self.record_size;
        if required_capacity > new_buffer.capacity() {
            // This will trigger a reallocation and copy
            new_buffer.reserve(self.record_size);
        }

        // Append new record data
        new_buffer.extend_from_slice(data);

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer);

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
        if offset + self.record_size > new_buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset,
                max: new_buffer.len().saturating_sub(self.record_size),
            });
        }

        // Update record data
        let slice = &mut new_buffer[offset..offset + self.record_size];
        slice.copy_from_slice(data);

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer);

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
        if offset + self.record_size > new_buffer.len() {
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
            let field_offset = offset + field.offset;

            // Update field data
            let slice = &mut new_buffer[field_offset..field_offset + field.size];
            slice.copy_from_slice(field_data);
        }

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer);

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
        if offset + self.record_size > new_buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset,
                max: new_buffer.len().saturating_sub(self.record_size),
            });
        }

        // Calculate field offset within record
        let field_offset = offset + field.offset;

        // Set deletion flag to true (non-zero)
        new_buffer[field_offset] = 1;

        // Atomically swap buffers (last-writer-wins semantics)
        self.buffer.store(new_buffer);

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
            let offset = i * self.record_size;
            let field_offset = offset + field.offset;

            // Check if record is deleted
            if current_buffer[field_offset] == 0 {
                // Record not deleted, copy it
                let record_slice = &current_buffer[offset..offset + self.record_size];
                new_buffer.extend_from_slice(record_slice);
            } else {
                records_removed += 1;
            }
        }

        // Only swap if records were actually removed
        if records_removed > 0 {
            self.buffer.store(new_buffer);
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
            align_offset(last_field.end_offset(), layout.align)
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
            offset = align_offset(offset, field.align);
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

/// Aligns an offset to the given alignment.
fn align_offset(offset: usize, align: usize) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TypeLayout, TypeRegistry};
    use ntest::timeout;

    fn create_test_fields() -> Vec<Field> {
        // Create a mock type registry with test layouts
        let _registry = TypeRegistry::new();

        // Create mock layouts for testing
        let u64_layout = unsafe {
            TypeLayout::new(
                "u64".to_string(),
                8,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                    8
                },
                |src, dst| {
                    if src.len() >= 8 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                        8
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<u64>()),
            )
        };

        let string_layout = unsafe {
            TypeLayout::new(
                "string".to_string(),
                260, // Fixed size: 4 bytes length + 256 bytes data
                1,
                false,
                |src, dst| {
                    let string_ptr = src as *const String;
                    let string = &*string_ptr;
                    let bytes = string.as_bytes();
                    let len = bytes.len().min(256);

                    // Write length as u32 (4 bytes)
                    dst.extend_from_slice(&(len as u32).to_ne_bytes());

                    // Write string bytes
                    dst.extend_from_slice(&bytes[..len]);

                    // Pad with zeros
                    let padding = 256 - len;
                    if padding > 0 {
                        dst.extend(std::iter::repeat_n(0u8, padding));
                    }

                    260
                },
                |src, dst| {
                    if src.len() < 260 {
                        return 0;
                    }
                    // Read length (first 4 bytes)
                    let mut len_bytes = [0u8; 4];
                    len_bytes.copy_from_slice(&src[..4]);
                    let len = u32::from_ne_bytes(len_bytes) as usize;

                    let actual_len = len.min(256);
                    let dst_ptr = dst as *mut String;
                    let bytes = &src[4..4 + actual_len];
                    *dst_ptr = String::from_utf8_lossy(bytes).to_string();

                    260
                },
                None,
            )
        };

        let bool_layout = unsafe {
            TypeLayout::new(
                "bool".to_string(),
                1,
                1,
                true,
                |src, dst| {
                    let bool_ptr = src as *const bool;
                    dst.push(if *bool_ptr { 1 } else { 0 });
                    1
                },
                |src, dst| {
                    if src.is_empty() {
                        return 0;
                    }
                    let dst_ptr = dst as *mut bool;
                    *dst_ptr = src[0] != 0;
                    1
                },
                Some(std::any::TypeId::of::<bool>()),
            )
        };

        vec![
            Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
            Field::new("name".to_string(), "string".to_string(), string_layout, 8),
            Field::new("active".to_string(), "bool".to_string(), bool_layout, 268), // After string field (8 + 260)
        ]
    }

    #[timeout(1000)]
    #[test]
    fn test_table_create() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        assert_eq!(table.name, "test_table");
        assert_eq!(table.record_size, 269); // 0-7: id, 8-267: name (260 bytes), 268: active
        assert_eq!(table.fields.len(), 3);
        assert_eq!(table.relations.len(), 0);
        assert_eq!(table.current_next_id(), 1);
        assert_eq!(table.record_count(), 0);
        // Buffer capacity should be 100 records * 269 bytes = 26900 bytes
        assert_eq!(table.buffer.capacity(), 26900);
    }

    #[timeout(1000)]
    #[test]
    fn test_field_offset() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        assert_eq!(table.field_offset("id").unwrap(), 0);
        assert_eq!(table.field_offset("name").unwrap(), 8);
        assert_eq!(table.field_offset("active").unwrap(), 268);

        assert!(table.field_offset("nonexistent").is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_next_id() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        assert_eq!(table.next_id(), 1);
        assert_eq!(table.next_id(), 2);
        assert_eq!(table.next_id(), 3);
        assert_eq!(table.current_next_id(), 4);
    }

    #[timeout(1000)]
    #[test]
    fn test_record_offset() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        assert_eq!(table.record_offset(0), 0);
        assert_eq!(table.record_offset(1), 269);
        assert_eq!(table.record_offset(10), 2690);
    }

    #[timeout(1000)]
    #[test]
    fn test_duplicate_field_names() {
        let _registry = TypeRegistry::new();
        let u64_layout = unsafe {
            TypeLayout::new(
                "u64".to_string(),
                8,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                    8
                },
                |src, dst| {
                    if src.len() >= 8 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                        8
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<u64>()),
            )
        };

        let fields = vec![
            Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
            Field::new("id".to_string(), "u64".to_string(), u64_layout, 8), // Duplicate!
        ];

        let result = Table::create("test_table".to_string(), fields, Some(100));
        assert!(result.is_err());
        match result {
            Err(DbError::FieldAlreadyExists { table, field }) => {
                assert_eq!(table, "test_table");
                assert_eq!(field, "id");
            }
            _ => panic!("Expected FieldAlreadyExists error"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_field_exceeds_record_size() {
        let _registry = TypeRegistry::new();
        let u64_layout = unsafe {
            TypeLayout::new(
                "u64".to_string(),
                8,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                    8
                },
                |src, dst| {
                    if src.len() >= 8 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                        8
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<u64>()),
            )
        };

        // Create a field with offset 96 (aligned to 8), which would require record size 104
        // This should work since record size is calculated from max(field.offset + field.size)
        let fields = vec![Field::new(
            "data".to_string(),
            "u64".to_string(),
            u64_layout,
            96,
        )];

        let result = Table::create("test_table".to_string(), fields, Some(100));
        assert!(result.is_ok());
        let table = result.unwrap();
        assert_eq!(table.record_size, 104); // 96 + 8
    }

    #[timeout(1000)]
    #[test]
    fn test_relations() {
        let fields = create_test_fields();
        let mut table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        let relation = Relation {
            to_table: "other_table".to_string(),
            from_field: "id".to_string(),
            to_field: "foreign_id".to_string(),
        };

        table.add_relation(relation);
        assert_eq!(table.relations.len(), 1);
        assert_eq!(table.relations[0].to_table, "other_table");

        assert!(table.remove_relation("other_table"));
        assert_eq!(table.relations.len(), 0);
        assert!(!table.remove_relation("nonexistent"));
    }

    #[timeout(1000)]
    #[test]
    fn test_get_field() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        assert!(table.get_field("id").is_some());
        assert!(table.get_field("name").is_some());
        assert!(table.get_field("active").is_some());
        assert!(table.get_field("nonexistent").is_none());
    }

    #[timeout(1000)]
    #[test]
    fn test_add_field() {
        let fields = create_test_fields();
        let mut table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create a new field layout
        let i32_layout = unsafe {
            TypeLayout::new(
                "i32".to_string(),
                4,
                4,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 4));
                    4
                },
                |src, dst| {
                    if src.len() >= 4 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 4);
                        4
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<i32>()),
            )
        };

        // Add a new field
        let offset = table
            .add_field("score".to_string(), "i32".to_string(), i32_layout.clone())
            .unwrap();

        // Check that field was added with correct offset
        // Existing fields: id(0-7), name(8-267), active(268)
        // New field should be at aligned offset after active (268 + 1 = 269, aligned to 4 = 272)
        assert_eq!(offset, 272);
        assert_eq!(table.record_size, 276); // 272 + 4 = 276

        // Check field exists
        assert!(table.get_field("score").is_some());
        let field = table.get_field("score").unwrap();
        assert_eq!(field.name, "score");
        assert_eq!(field.type_id, "i32");
        assert_eq!(field.offset, 272);
        assert_eq!(field.size, 4);
        assert_eq!(field.align, 4);

        // Try to add duplicate field - need to clone the layout since add_field takes ownership
        let i32_layout_clone = i32_layout.clone();
        let result = table.add_field("score".to_string(), "i32".to_string(), i32_layout_clone);
        assert!(result.is_err());
        match result {
            Err(DbError::FieldAlreadyExists { table: t, field }) => {
                assert_eq!(t, "test_table");
                assert_eq!(field, "score");
            }
            _ => panic!("Expected FieldAlreadyExists error"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_remove_field() {
        let fields = create_test_fields();
        let mut table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Remove the "name" field
        assert!(table.remove_field("name").is_ok());

        // Check field was removed
        assert!(table.get_field("name").is_none());
        assert!(table.get_field("id").is_some());
        assert!(table.get_field("active").is_some());

        // Check record size was recalculated
        // After removing name (32 bytes), we have id(8) + active(1) = 9 bytes
        // But active needs to be aligned after id: id ends at 8, active align=1 so offset=8
        // So record size = 8 + 1 = 9
        assert_eq!(table.record_size, 9);

        // Check field offsets were rebuilt
        let id_field = table.get_field("id").unwrap();
        assert_eq!(id_field.offset, 0);

        let active_field = table.get_field("active").unwrap();
        assert_eq!(active_field.offset, 8); // After id (8 bytes)

        // Try to remove non-existent field
        let result = table.remove_field("nonexistent");
        assert!(result.is_err());
        match result {
            Err(DbError::FieldNotFound { table: t, field }) => {
                assert_eq!(t, "test_table");
                assert_eq!(field, "nonexistent");
            }
            _ => panic!("Expected FieldNotFound error"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_add_field_with_alignment() {
        let _registry = TypeRegistry::new();

        // Create a table with a u8 field (align=1) and a u64 field (align=8)
        let u8_layout = unsafe {
            TypeLayout::new(
                "u8".to_string(),
                1,
                1,
                true,
                |src, dst| {
                    dst.push(*src);
                    1
                },
                |src, dst| {
                    if src.is_empty() {
                        return 0;
                    }
                    *dst = src[0];
                    1
                },
                Some(std::any::TypeId::of::<u8>()),
            )
        };

        let u64_layout = unsafe {
            TypeLayout::new(
                "u64".to_string(),
                8,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                    8
                },
                |src, dst| {
                    if src.len() >= 8 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                        8
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<u64>()),
            )
        };

        let fields = vec![Field::new(
            "flag".to_string(),
            "u8".to_string(),
            u8_layout,
            0,
        )];

        let mut table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Add u64 field - should be aligned to 8 bytes
        let offset = table
            .add_field("id".to_string(), "u64".to_string(), u64_layout)
            .unwrap();

        // u8 ends at offset 1, next aligned offset for align=8 is 8
        assert_eq!(offset, 8);
        assert_eq!(table.record_size, 16); // 8 + 8 = 16
    }

    #[timeout(1000)]
    #[test]
    fn test_field_type_validation() {
        let fields = create_test_fields();
        let mut table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create layout with type_id "i32"
        let i32_layout = unsafe {
            TypeLayout::new(
                "i32".to_string(),
                4,
                4,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 4));
                    4
                },
                |src, dst| {
                    if src.len() >= 4 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 4);
                        4
                    } else {
                        0
                    }
                },
                Some(std::any::TypeId::of::<i32>()),
            )
        };

        // Try to add field with mismatched type_id
        let result = table.add_field("score".to_string(), "u32".to_string(), i32_layout);
        assert!(result.is_err());
        match result {
            Err(DbError::TypeMismatch { expected, got }) => {
                assert_eq!(expected, "i32");
                assert_eq!(got, "u32");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_read_record_raw() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Buffer is empty initially
        assert!(table.read_record_raw(0).is_err());

        // We would need to add data to test this properly
        // This test verifies the method exists and returns error for empty buffer
    }

    #[timeout(1000)]
    #[test]
    fn test_read_field_raw() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Buffer is empty, but we can still test field lookup
        assert!(table.read_field_raw(0, "id").is_err());
        assert!(table.read_field_raw(0, "nonexistent").is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_concurrent_read_access() {
        use std::sync::Arc;
        use std::thread;

        let fields = create_test_fields();
        let table = Arc::new(Table::create("test_table".to_string(), fields, Some(100)).unwrap());

        // Spawn multiple threads to test concurrent access
        let mut handles = vec![];
        for _ in 0..10 {
            let table_clone = Arc::clone(&table);
            handles.push(thread::spawn(move || {
                // Each thread loads the buffer
                let buffer = table_clone.buffer.load();
                // Verify we can access it
                assert_eq!(buffer.len(), 0);
                // Thread holds Arc, preventing buffer deallocation
                std::thread::sleep(std::time::Duration::from_micros(100));
                // Arc drops here, allowing buffer deallocation if no other references
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // All threads completed without panics
        // ArcSwap epoch tracking ensures old buffers dropped after last reader
    }

    #[timeout(1000)]
    #[test]
    fn test_multiple_readers_same_buffer() {
        use std::sync::Arc;
        use std::thread;

        let fields = create_test_fields();
        let table = Arc::new(Table::create("test_table".to_string(), fields, Some(100)).unwrap());

        // Store some data
        {
            let mut data = vec![0u8; table.record_size * 3];
            // Fill with test data
            for (i, item) in data.iter_mut().enumerate() {
                *item = (i % 256) as u8;
            }
            table.buffer.store(data);
        }

        let mut handles = vec![];
        for _ in 0..5 {
            let table_clone = Arc::clone(&table);
            handles.push(thread::spawn(move || {
                // Each thread reads the same buffer concurrently
                let buffer = table_clone.buffer.load();

                // Verify all threads see the same data
                assert_eq!(buffer.len(), table_clone.record_size * 3);

                // Read some data from the buffer
                for record_idx in 0..3 {
                    let offset = record_idx * table_clone.record_size;
                    if offset < buffer.len() {
                        let slice = buffer.as_slice();
                        let _ = &slice
                            [offset..offset + table_clone.record_size.min(slice.len() - offset)];
                    }
                }

                // Simulate some work
                for _ in 0..1000 {
                    let slice = buffer.as_slice();
                    let _sum: usize = slice.iter().map(|&b| b as usize).sum();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All threads successfully accessed the same buffer concurrently
    }

    #[timeout(1000)]
    #[test]
    fn test_create_record() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create test record data
        let mut data = vec![0u8; table.record_size];
        // Set id field (u64 at offset 0)
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        // Set name field (string at offset 8) - 260 bytes total
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix (4 bytes)
        data[12..17].copy_from_slice(b"hello"); // String data
                                                // Set active field (bool at offset 268)
        data[268] = 1; // true

        // Create record
        let id = table.create_record(&data).unwrap();
        assert_eq!(id, 1);

        // Verify record was added
        assert_eq!(table.record_count(), 1);
        assert_eq!(table.current_next_id(), 2);

        // Read back the record
        let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
        assert_eq!(len, table.record_size);
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(slice, &data[..]);

        // Test invalid data size
        let result = table.create_record(&data[..10]);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_update_record() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create initial record
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 1;

        let id = table.create_record(&data).unwrap();
        assert_eq!(id, 1);

        // Update record
        let mut updated_data = vec![0u8; table.record_size];
        updated_data[0..8].copy_from_slice(&2u64.to_le_bytes());
        updated_data[8..12].copy_from_slice(&6u32.to_ne_bytes()); // Length prefix
        updated_data[12..18].copy_from_slice(b"world!");
        updated_data[268] = 0;

        table.update_record(0, &updated_data).unwrap();

        // Verify update
        let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(slice, &updated_data[..]);

        // Test invalid record index
        let result = table.update_record(1, &updated_data);
        assert!(result.is_err());

        // Test invalid data size
        let result = table.update_record(0, &updated_data[..10]);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_partial_update() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create initial record
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 1;

        table.create_record(&data).unwrap();

        // Partially update only the name field
        let mut name_data = vec![0u8; 260]; // string field size (260 bytes)
        name_data[0..4].copy_from_slice(&6u32.to_ne_bytes()); // Length prefix (4 bytes)
        name_data[4..10].copy_from_slice(b"world!");

        table.partial_update(0, &[("name", &name_data)]).unwrap();

        // Verify only name was updated
        let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

        // id should still be 1
        assert_eq!(&slice[0..8], &1u64.to_le_bytes());
        // name should be updated
        assert_eq!(&slice[8..12], &6u32.to_ne_bytes()); // Length prefix (4 bytes)
        assert_eq!(&slice[12..18], b"world!");
        // active should still be 1
        assert_eq!(slice[268], 1);

        // Test invalid field name
        let result = table.partial_update(0, &[("nonexistent", &name_data)]);
        assert!(result.is_err());

        // Test invalid field data size
        let result = table.partial_update(0, &[("name", &name_data[..10])]);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_delete_and_compact_records() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create 3 records
        for i in 0..3 {
            let mut data = vec![0u8; table.record_size];
            data[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
            data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
            data[12..17].copy_from_slice(b"hello");
            data[268] = 0; // active = false initially
            table.create_record(&data).unwrap();
        }

        assert_eq!(table.record_count(), 3);

        // Delete record 1 (index 1)
        table.delete_record(1, "active").unwrap();

        // Verify record 1 is marked as deleted
        let (ptr, len, _arc) = table.read_record_raw(1).unwrap();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(slice[268], 1); // active = true (deleted)

        // Compact table
        let removed = table.compact_table("active").unwrap();
        assert_eq!(removed, 1);
        assert_eq!(table.record_count(), 2);

        // Verify remaining records are 0 and 2 (original indices)
        let (ptr0, _, _) = table.read_record_raw(0).unwrap();
        let slice0 = unsafe { std::slice::from_raw_parts(ptr0, table.record_size) };
        assert_eq!(&slice0[0..8], &1u64.to_le_bytes()); // id = 1

        let (ptr1, _, _) = table.read_record_raw(1).unwrap();
        let slice1 = unsafe { std::slice::from_raw_parts(ptr1, table.record_size) };
        assert_eq!(&slice1[0..8], &3u64.to_le_bytes()); // id = 3

        // Test invalid deletion field
        let result = table.delete_record(0, "nonexistent");
        assert!(result.is_err());

        // Test compact with non-boolean field
        let result = table.compact_table("id");
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_concurrent_writers_last_writer_wins() {
        use std::sync::Arc;
        use std::thread;

        let fields = create_test_fields();
        let table = Arc::new(Table::create("test_table".to_string(), fields, Some(1000)).unwrap());

        // Create initial record
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 0;

        table.create_record(&data).unwrap();

        // Spawn multiple writers that update the same record
        let mut handles = vec![];
        for i in 0..10 {
            let table_clone = Arc::clone(&table);
            let mut update_data = vec![0u8; table.record_size];
            update_data[0..8].copy_from_slice(&(i as u64 + 100).to_le_bytes());
            update_data[8..12].copy_from_slice(&1u32.to_ne_bytes()); // Length prefix
            update_data[12] = b'A' + i as u8;
            update_data[268] = (i % 2) as u8;

            handles.push(thread::spawn(move || {
                // Each writer clones buffer and updates
                table_clone.update_record(0, &update_data).unwrap();
            }));
        }

        // Wait for all writers
        for handle in handles {
            handle.join().unwrap();
        }

        // Only the last writer's changes should be visible
        // (last-writer-wins semantics)
        let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

        // The final state should be from one of the writers
        // We can't predict which one due to race conditions,
        // but the record should be valid
        let id = u64::from_le_bytes(slice[0..8].try_into().unwrap());
        assert!((100..=109).contains(&id));
        assert_eq!(&slice[8..12], &1u32.to_ne_bytes()); // Length prefix (4 bytes)
        assert!((b'A'..=b'J').contains(&slice[12]));
        assert!(slice[268] == 0 || slice[268] == 1);
    }

    #[timeout(1000)]
    #[test]
    fn test_failed_write_discards_buffer() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create initial record
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 0;

        table.create_record(&data).unwrap();

        // Save initial state
        let initial_len = table.buffer.len();

        // Attempt invalid update (out of bounds)
        let result = table.update_record(5, &data);
        assert!(result.is_err());

        // Buffer should be unchanged
        assert_eq!(table.buffer.len(), initial_len);

        // Attempt another invalid operation
        let result = table.partial_update(0, &[("nonexistent", &data[..10])]);
        assert!(result.is_err());

        // Buffer should still be unchanged
        assert_eq!(table.buffer.len(), initial_len);

        // Valid operation should work
        data[268] = 1;
        table.update_record(0, &data).unwrap();
        assert_eq!(table.buffer.len(), initial_len); // Same length, different content
    }

    #[timeout(1000)]
    #[test]
    fn test_load_full_clones_buffer() {
        let buffer = AtomicBuffer::new(1024, 64);

        // Store initial data
        let initial_data = vec![1u8, 2, 3, 4, 5];
        buffer.store(initial_data.clone());

        // Load for reading
        let read_arc = buffer.load();
        assert_eq!(read_arc.as_slice(), &initial_data);

        // Load for modification (clones)
        let mut cloned = buffer.load_full();
        assert_eq!(&cloned, &initial_data);

        // Modify clone
        cloned.push(6);
        cloned.push(7);

        // Original buffer unchanged
        let read_arc2 = buffer.load();
        assert_eq!(read_arc2.as_slice(), &initial_data);

        // Store modified clone
        buffer.store(cloned);

        // Now buffer is updated
        let read_arc3 = buffer.load();
        assert_eq!(read_arc3.as_slice(), &[1u8, 2, 3, 4, 5, 6, 7]);
    }

    #[timeout(1000)]
    #[test]
    fn test_create_record_from_values() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create field values
        let id_bytes = 1u64.to_le_bytes();
        let mut name_bytes = vec![0u8; 260]; // string field size (260 bytes)
        name_bytes[0..4].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix (4 bytes)
        name_bytes[4..9].copy_from_slice(b"hello");
        let active_bytes = [1u8]; // bool = true

        let field_values = vec![&id_bytes[..], &name_bytes[..], &active_bytes[..]];

        // Create record from values
        let id = table.create_record_from_values(&field_values).unwrap();
        assert_eq!(id, 1);
        assert_eq!(table.record_count(), 1);

        // Read back and verify
        let (slice, _arc) = table.read_record(0).unwrap();
        assert_eq!(slice.len(), table.record_size);

        // Verify id field
        assert_eq!(&slice[0..8], &id_bytes);

        // Verify name field
        assert_eq!(&slice[8..12], &5u32.to_ne_bytes()); // Length prefix (4 bytes)
        assert_eq!(&slice[12..17], b"hello");

        // Verify active field
        assert_eq!(slice[268], 1);

        // Test field count mismatch
        let result = table.create_record_from_values(&[&id_bytes[..]]);
        assert!(result.is_err());

        // Test field size mismatch
        let wrong_size_bytes = [0u8; 4]; // Should be 8 bytes for u64
        let result = table.create_record_from_values(&[
            &wrong_size_bytes[..],
            &name_bytes[..],
            &active_bytes[..],
        ]);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_read_record() {
        let fields = create_test_fields();
        let table = Table::create("test_table".to_string(), fields, Some(100)).unwrap();

        // Create a record
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 1;

        table.create_record(&data).unwrap();

        // Read record as slice
        let (slice, arc) = table.read_record(0).unwrap();
        assert_eq!(slice.len(), table.record_size);
        assert_eq!(slice, &data[..]);

        // Arc should hold the buffer
        assert_eq!(arc.len(), table.record_size);

        // Test out of bounds
        assert!(table.read_record(1).is_err());
    }
}
