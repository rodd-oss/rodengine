# Test Plan for task_ds_1: Database Snapshot

**Task ID**: task_ds_1  
**Description**: Periodically snapshot entire database (buffers + schema) to binary file.

## 1. Basic Snapshot Functionality Tests

### test_snapshot_empty_database

- **Verifies**: Snapshot of empty database (no tables) creates valid binary file
- **Edge cases**: Empty schema, zero buffers
- **Assertions**: File exists, can be loaded back, integrity checks pass

### test_snapshot_single_table

- **Verifies**: Single table with records snapshots correctly
- **Edge cases**: Table with various field types (i32, f32, bool, custom types)
- **Assertions**: Loaded data matches original, record counts equal, field values preserved

### test_snapshot_multiple_tables

- **Verifies**: Multiple tables with relations snapshot correctly
- **Edge cases**: Tables with different record counts, inter-table relations
- **Assertions**: All tables restored, relations intact, referential integrity maintained

## 2. Concurrency & Atomicity Tests

### test_snapshot_during_concurrent_writes

- **Verifies**: Snapshot consistency during active write operations
- **Edge cases**: Writers adding/updating/deleting records during snapshot
- **Assertions**: Snapshot is internally consistent (no torn writes), loaded database valid

### test_snapshot_atomic_buffer_swap

- **Verifies**: Snapshot captures atomic buffer state (ArcSwap)
- **Edge cases**: Buffer swap occurs mid-snapshot
- **Assertions**: Snapshot contains either old or new buffer entirely, not mixed state

### test_snapshot_with_active_transactions

- **Verifies**: Snapshot behavior with pending transactions
- **Edge cases**: Uncommitted transaction log entries
- **Assertions**: Only committed data included in snapshot

## 3. Error Handling & Edge Cases

### test_snapshot_disk_full

- **Verifies**: Graceful handling when disk space exhausted
- **Edge cases**: Partial write failure
- **Assertions**: Error returned, no corrupted file left, database state unchanged

### test_snapshot_file_permissions

- **Verifies**: Permission denied scenarios handled
- **Edge cases**: Read-only filesystem, insufficient permissions
- **Assertions**: Appropriate error returned, database unaffected

### test_snapshot_corruption_detection

- **Verifies**: Corruption detection during save/load
- **Edge cases**: Checksum mismatch, version incompatibility, truncated file
- **Assertions**: Load fails with descriptive error, doesn't corrupt in-memory state

## 4. Performance & Resource Tests

### test_snapshot_large_database

- **Verifies**: Snapshot handles large datasets efficiently
- **Edge cases**: Millions of records, multi-gigabyte buffers
- **Assertions**: Memory usage bounded, no OOM, reasonable completion time

### test_snapshot_background_thread

- **Verifies**: Background snapshot doesn't block main event loop
- **Edge cases**: High-frequency snapshots (every tick)
- **Assertions**: Main loop latency unaffected, snapshot completes asynchronously

### test_snapshot_memory_mapping

- **Verifies**: Efficient file I/O (memory mapping if used)
- **Edge cases**: Buffer alignment for optimal I/O
- **Assertions**: File operations efficient, minimal copying

## 5. Schema & Metadata Tests

### test_snapshot_schema_preservation

- **Verifies**: Complete schema (tables, fields, relations) preserved
- **Edge cases**: Custom types, field offsets, validation rules
- **Assertions**: Schema identical after load, field access works

### test_snapshot_versioning

- **Verifies**: Version header included for forward/backward compatibility
- **Edge cases**: Version mismatch on load
- **Assertions**: Clear error on incompatible versions, migration path possible

### test_snapshot_checksum_integrity

- **Verifies**: Checksum validates file integrity
- **Edge cases**: Bit flips, partial corruption
- **Assertions**: Corruption detected, load fails safely

## 6. Integration & Recovery Tests

### test_snapshot_recovery_cycle

- **Verifies**: Complete save → load → verify cycle
- **Edge cases**: Multiple cycles, incremental changes
- **Assertions**: Database fully functional after recovery

### test_snapshot_partial_load

- **Verifies**: Partial snapshot loading (if supported)
- **Edge cases**: Selective table restoration
- **Assertions**: Loaded tables functional, missing tables handled

### test_snapshot_with_runtime_state

- **Verifies**: Runtime state (event loop, handlers) excluded from snapshot
- **Edge cases**: Active procedures, pending API requests
- **Assertions**: Only persistent data saved, runtime reinitialized on load

## Key Assertions & Expected Behaviors

1. **Atomicity**: Snapshot represents consistent database state at a point in time
2. **Integrity**: Loaded snapshot passes all validation checks
3. **Performance**: Snapshot doesn't block main operations (background thread)
4. **Error Safety**: Failures don't corrupt database or leave partial files
5. **Completeness**: All persistent data (buffers + schema) included
6. **Versioning**: Future compatibility considered with version headers
7. **Resource Management**: Memory/disk usage bounded, cleanup on failure
