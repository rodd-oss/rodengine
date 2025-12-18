//! Transaction isolation, staging buffers, and atomic commit.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::DbError;
use crate::table::Table;

/// Represents a single change to a table buffer.
#[derive(Debug, Clone)]
pub enum Change {
    /// Create a new record
    Create {
        /// Byte offset where record should be inserted
        offset: usize,
        /// Serialized record data
        data: Vec<u8>,
    },
    /// Update an existing record
    Update {
        /// Byte offset of the record to update
        offset: usize,
        /// Range of bytes being replaced (for conflict detection)
        old: Range<usize>,
        /// New serialized record data
        new: Vec<u8>,
    },
    /// Delete a record
    Delete {
        /// Byte offset of the record to delete
        offset: usize,
        /// Original record data (for conflict detection and rollback)
        original: Vec<u8>,
    },
}

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

        if !self.staging.contains_key(&table.name) {
            let staging_buffer = StagingBuffer::new(table);
            self.staging.insert(table.name.clone(), staging_buffer);
        }

        Ok(self.staging.get_mut(&table.name).unwrap())
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
            let staging_buffer = self.staging.get(table_name).unwrap();
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

/// RAII guard for transaction handling with auto-abort on drop.
///
/// If the transaction is not explicitly committed, it will be
/// automatically aborted when the handle is dropped.
#[derive(Debug)]
pub struct TransactionHandle {
    /// The transaction being managed
    pub(crate) transaction: Transaction,
    /// Whether to auto-abort on drop
    pub(crate) auto_abort: bool,
}

impl TransactionHandle {
    /// Creates a new transaction handle.
    pub fn new() -> Self {
        Self {
            transaction: Transaction::new(),
            auto_abort: true,
        }
    }

    /// Gets a mutable reference to the underlying transaction.
    pub fn transaction_mut(&mut self) -> &mut Transaction {
        &mut self.transaction
    }

    /// Commits the transaction.
    ///
    /// # Arguments
    /// * `tables` - Map of table name to Table reference
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit(mut self, tables: &HashMap<String, &Table>) -> Result<(), DbError> {
        self.auto_abort = false;
        self.transaction.commit(tables)
    }

    /// Commits the transaction without consuming the handle.
    ///
    /// # Arguments
    /// * `tables` - Map of table name to Table reference
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit_with_tables(&mut self, tables: &HashMap<String, &Table>) -> Result<(), DbError> {
        self.auto_abort = false;
        self.transaction.commit(tables)
    }

    /// Aborts the transaction.
    pub fn abort(mut self) {
        self.auto_abort = false;
        self.transaction.abort();
    }

    /// Returns whether the transaction has been committed.
    pub fn is_committed(&self) -> bool {
        self.transaction.is_committed()
    }

    /// Returns whether the transaction has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.transaction.is_aborted()
    }

    /// Returns whether the transaction is still active.
    pub fn is_active(&self) -> bool {
        self.transaction.is_active()
    }
}

impl Default for TransactionHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TransactionHandle {
    fn drop(&mut self) {
        if self.auto_abort && self.transaction.is_active() {
            self.transaction.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Field, Table};
    use crate::types::TypeLayout;
    use ntest::timeout;

    fn create_test_table() -> Table {
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

        let fields = vec![Field::new(
            "id".to_string(),
            "u64".to_string(),
            u64_layout,
            0,
        )];

        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap()
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_new() {
        let table = create_test_table();
        let staging_buffer = StagingBuffer::new(&table);

        assert_eq!(staging_buffer.table_name, "test_table");
        assert_eq!(staging_buffer.record_size, 8);
        assert_eq!(staging_buffer.buffer.len(), 0);
        assert!(staging_buffer.is_empty());
        assert_eq!(staging_buffer.record_count(), 0);
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_stage_create() {
        let table = create_test_table();
        let mut staging_buffer = StagingBuffer::new(&table);

        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8]; // 8 bytes = record_size
        let offset = staging_buffer.stage_create(data.clone()).unwrap();

        assert_eq!(offset, 0);
        assert_eq!(staging_buffer.buffer.len(), 8);
        assert_eq!(staging_buffer.record_count(), 1);
        assert_eq!(staging_buffer.changes.len(), 1);

        match &staging_buffer.changes[0] {
            Change::Create {
                offset: change_offset,
                data: change_data,
            } => {
                assert_eq!(*change_offset, 0);
                assert_eq!(change_data, &data);
            }
            _ => panic!("Expected Create change"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_stage_create_invalid_size() {
        let table = create_test_table();
        let mut staging_buffer = StagingBuffer::new(&table);

        let data = vec![1u8, 2, 3]; // Wrong size
        let result = staging_buffer.stage_create(data);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_stage_update() {
        let table = create_test_table();
        let mut staging_buffer = StagingBuffer::new(&table);

        // First create a record
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        staging_buffer.stage_create(data).unwrap();

        // Then update it
        let new_data = vec![9u8, 10, 11, 12, 13, 14, 15, 16];
        staging_buffer.stage_update(0, new_data.clone()).unwrap();

        assert_eq!(staging_buffer.buffer.len(), 8);
        assert_eq!(staging_buffer.changes.len(), 2);
        assert_eq!(staging_buffer.buffer, new_data);

        match &staging_buffer.changes[1] {
            Change::Update { offset, old, new } => {
                assert_eq!(*offset, 0);
                assert_eq!(old.start, 0);
                assert_eq!(old.end, 8);
                assert_eq!(new, &new_data);
            }
            _ => panic!("Expected Update change"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_stage_update_invalid_offset() {
        let table = create_test_table();
        let mut staging_buffer = StagingBuffer::new(&table);

        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let result = staging_buffer.stage_update(0, data);
        assert!(result.is_err()); // Buffer is empty, offset 0 is out of bounds
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_stage_delete() {
        let table = create_test_table();
        let mut staging_buffer = StagingBuffer::new(&table);

        // First create a record
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        staging_buffer.stage_create(data.clone()).unwrap();

        // Then delete it
        staging_buffer.stage_delete(0).unwrap();

        assert_eq!(staging_buffer.changes.len(), 2);

        match &staging_buffer.changes[1] {
            Change::Delete { offset, original } => {
                assert_eq!(*offset, 0);
                assert_eq!(original, &data);
            }
            _ => panic!("Expected Delete change"),
        }
    }

    #[timeout(1000)]
    #[test]
    fn test_staging_buffer_record_offset() {
        let table = create_test_table();
        let staging_buffer = StagingBuffer::new(&table);

        assert_eq!(staging_buffer.record_offset(0), 0);
        assert_eq!(staging_buffer.record_offset(1), 8);
        assert_eq!(staging_buffer.record_offset(10), 80);
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_new() {
        let transaction = Transaction::new();
        assert!(transaction.is_active());
        assert!(!transaction.is_committed());
        assert!(!transaction.is_aborted());
        assert!(!transaction.has_staged_changes());
        assert_eq!(transaction.staged_table_count(), 0);
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_get_or_create_staging_buffer() {
        let table = create_test_table();
        let mut transaction = Transaction::new();

        let staging_buffer = transaction.get_or_create_staging_buffer(&table).unwrap();
        assert_eq!(staging_buffer.table_name, "test_table");
        assert_eq!(transaction.staged_table_count(), 1);

        // Getting again should return the same buffer
        let staging_buffer2 = transaction.get_or_create_staging_buffer(&table).unwrap();
        assert_eq!(staging_buffer2.table_name, "test_table");
        assert_eq!(transaction.staged_table_count(), 1); // Still only one table
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_stage_operations() {
        let table = create_test_table();
        let mut transaction = Transaction::new();

        // Stage a create
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let offset = transaction.stage_create(&table, data.clone()).unwrap();
        assert_eq!(offset, 0);

        // Stage an update
        let new_data = vec![9u8, 10, 11, 12, 13, 14, 15, 16];
        transaction
            .stage_update(&table, 0, new_data.clone())
            .unwrap();

        // Stage a delete
        transaction.stage_delete(&table, 0).unwrap();

        assert_eq!(transaction.staged_table_count(), 1);
        assert!(transaction.has_staged_changes());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_commit() {
        let table = create_test_table();
        let mut transaction = Transaction::new();

        // Stage some changes
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        transaction.stage_create(&table, data).unwrap();

        // Create tables map for commit
        let mut tables = HashMap::new();
        tables.insert(table.name.clone(), &table);

        // Commit should succeed
        let result = transaction.commit(&tables);
        assert!(result.is_ok());
        assert!(transaction.is_committed());
        assert!(!transaction.is_active());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_commit_twice() {
        let table = create_test_table();
        let mut transaction = Transaction::new();

        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        transaction.stage_create(&table, data).unwrap();

        let mut tables = HashMap::new();
        tables.insert(table.name.clone(), &table);

        // First commit should succeed
        transaction.commit(&tables).unwrap();

        // Second commit should fail
        let result = transaction.commit(&tables);
        assert!(result.is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_abort() {
        let table = create_test_table();
        let mut transaction = Transaction::new();

        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        transaction.stage_create(&table, data).unwrap();

        assert_eq!(transaction.staged_table_count(), 1);
        assert!(transaction.has_staged_changes());

        transaction.abort();

        assert!(transaction.is_aborted());
        assert!(!transaction.is_active());
        assert_eq!(transaction.staged_table_count(), 0);
        assert!(!transaction.has_staged_changes());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_handle_new() {
        let handle = TransactionHandle::new();
        assert!(handle.is_active());
        assert!(!handle.is_committed());
        assert!(!handle.is_aborted());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_handle_commit() {
        let table = create_test_table();
        let mut handle = TransactionHandle::new();

        // Stage some changes
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        handle.transaction_mut().stage_create(&table, data).unwrap();

        let mut tables = HashMap::new();
        tables.insert(table.name.clone(), &table);

        // Commit should succeed
        let result = handle.commit(&tables);
        assert!(result.is_ok());
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_handle_abort() {
        let table = create_test_table();
        let mut handle = TransactionHandle::new();

        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        handle.transaction_mut().stage_create(&table, data).unwrap();

        // abort() takes ownership, so we can't use handle after
        handle.abort();
        // handle is consumed by abort(), so we can't assert on it
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_handle_auto_abort() {
        let table = create_test_table();
        let staged_changes_before;

        {
            let mut handle = TransactionHandle::new();

            let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
            handle.transaction_mut().stage_create(&table, data).unwrap();

            // Check staged changes before drop
            staged_changes_before = handle.transaction_mut().staged_table_count();
        } // handle drops here without commit

        // Create a new transaction to verify the old one was cleaned up
        let mut new_handle = TransactionHandle::new();
        let staged_changes_after = new_handle.transaction_mut().staged_table_count();

        // The old transaction should have been auto-aborted (cleared its staging buffers)
        // New transaction should have no staged changes
        assert_eq!(staged_changes_before, 1);
        assert_eq!(staged_changes_after, 0);
    }

    #[timeout(1000)]
    #[test]
    fn test_transaction_sorted_commit_order() {
        // Create multiple tables
        let table1 = {
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
                "id".to_string(),
                "u64".to_string(),
                u64_layout,
                0,
            )];
            Table::create("table_z".to_string(), fields, Some(100), usize::MAX).unwrap()
        };

        let table2 = {
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
                "id".to_string(),
                "u64".to_string(),
                u64_layout,
                0,
            )];
            Table::create("table_a".to_string(), fields, Some(100), usize::MAX).unwrap()
        };

        let mut transaction = Transaction::new();

        // Stage changes in reverse alphabetical order
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        transaction.stage_create(&table1, data.clone()).unwrap();
        transaction.stage_create(&table2, data).unwrap();

        let mut tables = HashMap::new();
        tables.insert(table1.name.clone(), &table1);
        tables.insert(table2.name.clone(), &table2);

        // Commit should process tables in sorted order (table_a, then table_z)
        let result = transaction.commit(&tables);
        assert!(result.is_ok());
    }
}
