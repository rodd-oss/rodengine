# Test Plan for `task_re_1`: Load Database from Binary Snapshot

## Context

Rust implementation of in-memory relational database. Binary snapshot loading implements TRD's parallel disk persistence with binary format for disk snapshots. Snapshots can be loaded via REST API endpoints.

## 1. Basic Loading Tests

- **`test_load_valid_snapshot`**: Load a valid snapshot with simple schema (1-2 tables, basic fields)
  - Verifies: Database state matches snapshot content, all tables/fields restored
  - Assertions: Table count, field definitions, record data matches original

- **`test_load_empty_database`**: Load snapshot of empty database (no tables)
  - Verifies: Empty database state restored correctly
  - Assertions: No tables present, storage buffers empty

## 2. Schema Restoration Tests

- **`test_load_complex_schema`**: Load snapshot with complex schema (multiple tables, relations, custom types)
  - Verifies: All schema elements restored (tables, fields, relations, custom types)
  - Assertions: Field offsets correct, relations intact, custom type registry populated

- **`test_load_schema_with_relations`**: Load snapshot containing table relations
  - Verifies: Referential integrity maintained after load
  - Assertions: Relation mappings preserved, foreign key constraints valid

## 3. Data Integrity Tests

- **`test_load_with_records`**: Load snapshot containing actual data records
  - Verifies: All record data restored with correct values
  - Assertions: Record count matches, field values identical, buffer contents byte-for-byte equal

- **`test_load_large_dataset`**: Load snapshot with many records (stress test)
  - Verifies: Performance and memory handling for large datasets
  - Assertions: All records accessible, no data corruption

## 4. Error Handling & Edge Cases

- **`test_load_corrupted_snapshot`**: Attempt to load corrupted/malformed binary file
  - Verifies: Graceful error handling, no panic
  - Assertions: Returns `Err` with appropriate error type, database remains in clean state

- **`test_load_version_mismatch`**: Load snapshot with incompatible version
  - Verifies: Version checking works
  - Assertions: Returns `Err(VersionMismatch)`, provides migration path info

- **`test_load_missing_file`**: Attempt to load non-existent snapshot file
  - Verifies: File not found handling
  - Assertions: Returns `Err(FileNotFound)`, database initialized as empty

- **`test_load_partial_corruption`**: Snapshot partially corrupted (valid header, corrupted data)
  - Verifies: Partial corruption detection
  - Assertions: Returns `Err(CorruptedData)`, indicates corruption location

## 5. Checksum & Validation Tests

- **`test_load_invalid_checksum`**: Snapshot with incorrect checksum
  - Verifies: Checksum validation prevents loading corrupted data
  - Assertions: Returns `Err(ChecksumMismatch)`

- **`test_load_truncated_file`**: Snapshot file truncated (incomplete write)
  - Verifies: File size validation
  - Assertions: Returns `Err(TruncatedFile)`

## 6. Integration & State Tests

- **`test_load_then_modify`**: Load snapshot, then perform CRUD operations
  - Verifies: Loaded database fully functional
  - Assertions: Operations succeed, data consistency maintained

- **`test_load_over_existing_data`**: Load snapshot when database already contains data
  - Verifies: Proper state replacement/merging
  - Assertions: Old data replaced, new state matches snapshot

- **`test_load_concurrent_access`**: Load snapshot while other threads attempt access
  - Verifies: Thread safety during load operation
  - Assertions: No data races, consistent state after load

## 7. Performance & Resource Tests

- **`test_load_memory_usage`**: Monitor memory allocation during load
  - Verifies: Efficient memory usage, no leaks
  - Assertions: Memory usage proportional to snapshot size

- **`test_load_timeout`**: Large snapshot load with timeout
  - Verifies: Load operation doesn't hang
  - Assertions: Completes within reasonable time or returns timeout error

## 8. REST API Integration Tests

- **`test_load_via_rest_endpoint`**: Load snapshot via REST API endpoint (POST /snapshot/load)
  - Verifies: Snapshot loading integrated with REST API as per TRD
  - Assertions: Endpoint returns appropriate status, database state updated

- **`test_rest_api_load_with_authentication`**: Load snapshot via REST API with authentication headers
  - Verifies: Security integration for snapshot loading
  - Assertions: Proper auth required, unauthorized requests rejected

- **`test_rest_api_load_progress_endpoint`**: Monitor load progress via REST API (GET /snapshot/load/progress)
  - Verifies: Progress tracking for large snapshot loads
  - Assertions: Progress endpoint returns accurate status

- **`test_rest_api_load_cancel_endpoint`**: Cancel in-progress load via REST API (DELETE /snapshot/load)
  - Verifies: Load cancellation capability
  - Assertions: Cancel stops load, database returns to previous state

- **`test_rest_api_load_error_handling`**: REST API returns appropriate HTTP errors for failed loads
  - Verifies: Error mapping from internal errors to HTTP status codes
  - Assertions: 400 for validation errors, 500 for internal errors, etc.

## 9. Integration with Runtime

- **`test_load_within_tickrate`**: Load snapshot within event loop tickrate constraints (15-120 Hz)
  - Verifies: Snapshot loading doesn't block event loop
  - Assertions: Load operation completes within tick duration or uses background thread

## Edge Cases to Consider:

1. **Zero-byte snapshot file** - Should fail validation
2. **Snapshot with maximum field count** - Test limits
3. **Snapshot with malformed UTF-8 in field names** - Encoding validation
4. **Snapshot with invalid field offsets** - Out-of-bounds detection
5. **Snapshot with circular relations** - Should still load if schema valid
6. **Snapshot from different endianness system** - Cross-platform compatibility
7. **Snapshot with deleted table references** - Handle dangling relations
8. **Snapshot with custom type dependencies** - Ensure type registry order

## Expected Behaviors:

- Successful load returns `Ok(())`, database state matches snapshot
- Failed load returns appropriate `Err` variant, database remains unchanged
- All validation occurs before any state modification (atomic load)
- Memory safety maintained even with corrupted snapshots
- Thread-safe: concurrent reads see consistent state (either old or new)
