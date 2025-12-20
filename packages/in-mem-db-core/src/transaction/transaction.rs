use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::DbError;
use crate::table::Table;

use super::staging_buffer::StagingBuffer;

/// Transaction context holding staged changes across multiple tables.
///
/// Changes are isolated from main buffers until commit.
#[derive(Debug)]
pub struct Transaction {
    /// Map of table name to staging buffer
    staging: HashMap<String, StagingBuffer>,
    /// Whether the transaction has been committed
    committed: AtomicBool,
    /// Whether the transaction has been aborted
    aborted: AtomicBool,
}

impl Transaction {
    /// Creates a new empty transaction.
    pub fn new() -> Self {
        Self {
            staging: HashMap::new(),
            committed: AtomicBool::new(false),
            aborted: AtomicBool::new(false),
        }
    }

    /// Gets or creates a staging buffer for the given table.
    ///
    /// # Arguments
    /// * `table` - The table to get/create staging buffer for
    ///
    /// # Returns
    /// `Result<&mut StagingBuffer, DbError>` containing the staging buffer.
    pub fn get_or_create_staging_buffer(
        &mut self,
        table: &Table,
    ) -> Result<&mut StagingBuffer, DbError> {
        if self.is_committed() {
            return Err(DbError::TransactionConflict(
                "transaction already committed".to_string(),
            ));
        }

        if self.is_aborted() {
            return Err(DbError::TransactionConflict(
                "transaction aborted".to_string(),
            ));
        }

        Ok(self
            .staging
            .entry(table.name.clone())
            .or_insert_with(|| StagingBuffer::new(table)))
    }

    /// Stages a record update in the transaction.
    ///
    /// # Arguments
    /// * `table` - The table containing the record
    /// * `record_index` - Zero-based record index
    /// * `new_data` - New serialized record data
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn stage_update(
        &mut self,
        table: &Table,
        record_index: usize,
        new_data: Vec<u8>,
    ) -> Result<(), DbError> {
        let staging_buffer = self.get_or_create_staging_buffer(table)?;
        let offset = staging_buffer.record_offset(record_index);
        staging_buffer.stage_update(offset, new_data)
    }

    /// Stages a record creation in the transaction.
    ///
    /// # Arguments
    /// * `table` - The table to create the record in
    /// * `data` - Serialized record data
    ///
    /// # Returns
    /// `Result<usize, DbError>` containing the byte offset where record was inserted.
    pub fn stage_create(&mut self, table: &Table, data: Vec<u8>) -> Result<usize, DbError> {
        let staging_buffer = self.get_or_create_staging_buffer(table)?;
        staging_buffer.stage_create(data)
    }

    /// Stages a record deletion in the transaction.
    ///
    /// # Arguments
    /// * `table` - The table containing the record
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn stage_delete(&mut self, table: &Table, record_index: usize) -> Result<(), DbError> {
        let staging_buffer = self.get_or_create_staging_buffer(table)?;
        let offset = staging_buffer.record_offset(record_index);
        staging_buffer.stage_delete(offset)
    }

    /// Commits all staged changes atomically.
    ///
    /// Performs all-or-nothing buffer swaps across all modified tables.
    ///
    /// # Arguments
    /// * `tables` - Map of table name to Table reference
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit(&mut self, tables: &HashMap<String, &Table>) -> Result<(), DbError> {
        if self.is_committed() {
            return Err(DbError::TransactionConflict(
                "transaction already committed".to_string(),
            ));
        }

        if self.is_aborted() {
            return Err(DbError::TransactionConflict(
                "transaction aborted".to_string(),
            ));
        }

        // Sort tables by name to prevent deadlock
        let mut table_names: Vec<String> = self.staging.keys().cloned().collect();
        table_names.sort();

        // Apply changes in sorted order
        for table_name in &table_names {
            let staging_buffer = self.staging.get(table_name).ok_or_else(|| {
                DbError::DataCorruption(format!(
                    "Staging buffer for table '{}' not found during commit",
                    table_name
                ))
            })?;
            let table = tables
                .get(table_name)
                .ok_or_else(|| DbError::TableNotFound {
                    table: table_name.clone(),
                })?;

            // Apply staged changes to the table
            self.apply_staging_buffer(table, staging_buffer)?;
        }

        // Mark as committed
        self.committed.store(true, Ordering::Release);
        Ok(())
    }

    /// Applies a staging buffer to its table.
    ///
    /// # Arguments
    /// * `table` - The table to apply changes to
    /// * `staging_buffer` - The staging buffer with changes
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    fn apply_staging_buffer(
        &self,
        table: &Table,
        staging_buffer: &StagingBuffer,
    ) -> Result<(), DbError> {
        // For now, we just swap the entire buffer
        // In a more sophisticated implementation, we would:
        // 1. Detect conflicts with concurrent transactions
        // 2. Apply only the changes that don't conflict
        // 3. Handle partial failures

        table
            .buffer
            .store(staging_buffer.buffer.clone())
            .map_err(|e| match e {
                DbError::MemoryLimitExceeded {
                    requested, limit, ..
                } => DbError::MemoryLimitExceeded {
                    requested,
                    limit,
                    table: table.name.clone(),
                },
                _ => e,
            })?;
        Ok(())
    }

    /// Aborts the transaction, discarding all staged changes.
    pub fn abort(&mut self) {
        if !self.is_committed() && !self.is_aborted() {
            self.aborted.store(true, Ordering::Release);
            self.staging.clear();
        }
    }

    /// Returns whether the transaction has been committed.
    pub fn is_committed(&self) -> bool {
        self.committed.load(Ordering::Acquire)
    }

    /// Returns whether the transaction has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Acquire)
    }

    /// Returns whether the transaction is still active (not committed or aborted).
    pub fn is_active(&self) -> bool {
        !self.is_committed() && !self.is_aborted()
    }

    /// Returns the number of tables with staged changes.
    pub fn staged_table_count(&self) -> usize {
        self.staging.len()
    }

    /// Returns whether any changes have been staged.
    pub fn has_staged_changes(&self) -> bool {
        !self.staging.is_empty()
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}
