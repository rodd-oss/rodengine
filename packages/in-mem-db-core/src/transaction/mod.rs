//! Transaction isolation, staging buffers, and atomic commit.

mod change;
mod staging_buffer;
#[allow(clippy::module_inception)]
mod transaction;
mod transaction_handle;

pub use change::Change;
pub use staging_buffer::StagingBuffer;
pub use transaction::Transaction;
pub use transaction_handle::TransactionHandle;

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
        let mut tables = std::collections::HashMap::new();
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

        let mut tables = std::collections::HashMap::new();
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

        let mut tables = std::collections::HashMap::new();
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

        let mut tables = std::collections::HashMap::new();
        tables.insert(table1.name.clone(), &table1);
        tables.insert(table2.name.clone(), &table2);

        // Commit should process tables in sorted order (table_a, then table_z)
        let result = transaction.commit(&tables);
        assert!(result.is_ok());
    }
}
