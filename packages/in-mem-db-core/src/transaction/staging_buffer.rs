use crate::error::DbError;
use crate::table::Table;

use super::change::Change;

/// Holds staged changes for a single table.
///
/// Changes are isolated from the main buffer until commit.
#[derive(Debug)]
pub struct StagingBuffer {
    /// Name of the table this buffer belongs to
    pub table_name: String,
    /// Copy of the table buffer with staged changes applied
    pub buffer: Vec<u8>,
    /// List of changes staged in this transaction
    pub changes: Vec<Change>,
    /// Record size for offset calculations
    pub record_size: usize,
}

impl StagingBuffer {
    /// Creates a new staging buffer from a table's current state.
    ///
    /// # Arguments
    /// * `table` - The table to create a staging buffer for
    ///
    /// # Returns
    /// A new `StagingBuffer` instance.
    pub fn new(table: &Table) -> Self {
        let buffer = table.buffer.load_full();
        Self {
            table_name: table.name.clone(),
            buffer,
            changes: Vec::new(),
            record_size: table.record_size,
        }
    }

    /// Stages a record update.
    ///
    /// # Arguments
    /// * `offset` - Byte offset of the record to update
    /// * `new_data` - New serialized record data
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn stage_update(&mut self, offset: usize, new_data: Vec<u8>) -> Result<(), DbError> {
        // Validate offset is within buffer bounds and aligned to record size
        if offset >= self.buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset,
                max: self.buffer.len().saturating_sub(self.record_size),
            });
        }

        if !offset.is_multiple_of(self.record_size) {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset,
                max: self.buffer.len().saturating_sub(self.record_size),
            });
        }

        // Validate new data size matches record size
        if new_data.len() != self.record_size {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset: new_data.len(),
                max: self.record_size,
            });
        }

        // Save original data for conflict detection
        let old_end = offset + self.record_size;
        let _old_data = self.buffer[offset..old_end].to_vec();

        // Apply update to staging buffer
        let slice = &mut self.buffer[offset..old_end];
        slice.copy_from_slice(&new_data);

        // Record the change
        self.changes.push(Change::Update {
            offset,
            old: offset..old_end,
            new: new_data,
        });

        Ok(())
    }

    /// Stages a record creation.
    ///
    /// # Arguments
    /// * `data` - Serialized record data
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the byte offset where record was inserted.
    pub fn stage_create(&mut self, data: Vec<u8>) -> Result<usize, DbError> {
        // Validate data size matches record size
        if data.len() != self.record_size {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset: data.len(),
                max: self.record_size,
            });
        }

        // Append new record to staging buffer
        let offset = self.buffer.len();
        self.buffer.extend_from_slice(&data);

        // Record the change
        self.changes.push(Change::Create { offset, data });

        Ok(offset)
    }

    /// Stages a record deletion.
    ///
    /// # Arguments
    /// * `offset` - Byte offset of the record to delete
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn stage_delete(&mut self, offset: usize) -> Result<(), DbError> {
        // Validate offset is within buffer bounds and aligned to record size
        if offset >= self.buffer.len() {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset,
                max: self.buffer.len().saturating_sub(self.record_size),
            });
        }

        if !offset.is_multiple_of(self.record_size) {
            return Err(DbError::InvalidOffset {
                table: self.table_name.clone(),
                offset,
                max: self.buffer.len().saturating_sub(self.record_size),
            });
        }

        // Save original data for conflict detection and rollback
        let original = self.buffer[offset..offset + self.record_size].to_vec();

        // Record the change (actual deletion happens at commit time or in compact)
        self.changes.push(Change::Delete { offset, original });

        Ok(())
    }

    /// Returns the current buffer length in bytes.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns the record size in bytes.
    pub fn record_size(&self) -> usize {
        self.record_size
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
        record_index * self.record_size
    }
}
