# Test Plan: Atomic CRUD Operations (task_ao_1)

## Overview

Tests for ensuring each CRUD operation (create, read, update, delete) is atomic (all‑or‑nothing) in the relational in‑memory database.

## 1. Atomic Create Tests

### `test_create_atomic_success`

**Verifies**: Successful record creation persists completely with all fields.
**Assertions**:

- Record exists in buffer after creation
- All field values match input
- Buffer size increased appropriately
- No partial writes in buffer

### `test_create_atomic_failure_preserves_state`

**Verifies**: Failed creation leaves database unchanged.
**Edge cases**:

- Buffer allocation failure
- Invalid field data
- Schema validation failure
  **Assertions**:
- Buffer size unchanged
- No partial record data in buffer
- Existing records unaffected

### `test_create_concurrent_read_consistency`

**Verifies**: Readers see consistent state during creation.
**Edge cases**:

- Concurrent reads during buffer swap
- Multiple readers during write
  **Assertions**:
- Readers never see partially written records
- All readers see either old or new buffer, never mixed state

## 2. Atomic Read Tests

### `test_read_atomic_consistency`

**Verifies**: Read returns complete record or nothing.
**Assertions**:

- Read returns all field values or error
- No partial field data returned
- Invalid record index returns error, not partial data

### `test_read_during_write_isolation`

**Verifies**: Reads during write see old buffer.
**Edge cases**:

- Read while buffer swap in progress
- Read during copy‑on‑write operation
  **Assertions**:
- Readers see consistent snapshot
- No blocking of readers by writers

### `test_read_after_atomic_swap`

**Verifies**: Reads immediately see new buffer after swap.
**Assertions**:

- After `ArcSwap::store`, all new reads see updated data
- No stale reads after atomic swap
- Consistent view across all readers post‑swap

## 3. Atomic Update Tests

### `test_update_atomic_success`

**Verifies**: Successful update applies all field changes.
**Assertions**:

- All specified fields updated
- Unspecified fields unchanged
- Record remains at same buffer offset
- Buffer integrity maintained

### `test_update_partial_failure_rollback`

**Verifies**: Failed update rolls back all changes.
**Edge cases**:

- Field validation failure mid‑update
- Buffer bounds violation
- Type conversion error
  **Assertions**:
- Original record data preserved
- No intermediate state persisted
- Transaction log used for rollback

### `test_update_concurrent_consistency`

**Verifies**: Concurrent reads see either old or new version.
**Edge cases**:

- Multiple readers during update
- Read overlapping with buffer swap
  **Assertions**:
- Readers see complete record version (old or new)
- No readers see partially updated fields
- Atomic buffer swap ensures version consistency

## 4. Atomic Delete Tests

### `test_delete_atomic_success`

**Verifies**: Record is completely removed.
**Assertions**:

- Record no longer accessible by index
- Buffer space reclaimed/marked free
- No dangling references to deleted record

### `test_delete_failure_preserves_record`

**Verifies**: Failed delete leaves record intact.
**Edge cases**:

- Referential integrity violation
- Buffer manipulation error
- Concurrent modification conflict
  **Assertions**:
- Record remains accessible
- All field data preserved
- Buffer state unchanged

### `test_delete_concurrent_access`

**Verifies**: Concurrent reads handle deleted records.
**Edge cases**:

- Read in progress during delete
- Multiple readers of record being deleted
  **Assertions**:
- Readers with old buffer reference see record
- New readers after delete see no record
- No crashes or invalid memory access

## 5. Edge Case Tests

### `test_power_failure_simulation`

**Verifies**: Crash mid‑operation leaves no partial state.
**Method**: Simulate panic during CRUD operation.
**Assertions**:

- Database state consistent after recovery
- No partially written records
- Transaction log enables clean recovery

### `test_buffer_swap_atomicity`

**Verifies**: `ArcSwap::store` is atomic and readers unaffected.
**Assertions**:

- Single atomic instruction for buffer pointer swap
- Readers continue with old buffer reference
- No reader‑writer locks required

### `test_multi_field_update_atomic`

**Verifies**: Updates to multiple fields are atomic.
**Edge cases**:

- Cross‑field dependencies
- Field ordering in buffer
- Alignment requirements
  **Assertions**:
- All fields updated together or none
- No intermediate state with some fields updated

### `test_record_size_boundary`

**Verifies**: Operations at buffer capacity limits.
**Edge cases**:

- Record size equals remaining buffer space
- Buffer reallocation during operation
- Maximum capacity reached
  **Assertions**:
- Operation succeeds completely or fails
- No buffer corruption at boundaries
- Proper error handling for capacity limits

### `test_concurrent_crud_operations`

**Verifies**: Multiple threads performing CRUD simultaneously.
**Edge cases**:

- Concurrent create and read
- Concurrent update and delete
- Multiple writers to same table
  **Assertions**:
- All operations maintain atomicity
- No data races or corruption
- Readers always see valid state

## 6. Transaction Log Tests (Related to task_ao_2)

### `test_transaction_log_creation`

**Verifies**: Operations create log entries for rollback.
**Assertions**:

- Log entry created before buffer modification
- Entry contains enough data for rollback
- Log persisted before operation completion

### `test_rollback_from_log`

**Verifies**: Failed operations roll back using log.
**Assertions**:

- Log provides complete undo information
- Rollback restores exact previous state
- No side effects from rolled‑back operation

### `test_log_cleanup_after_success`

**Verifies**: Log entries cleaned after successful commit.
**Assertions**:

- Successful operations remove their log entries
- Log doesn't grow unbounded
- Cleanup doesn't interfere with concurrent operations

## Key Assertions & Behaviors

1. **All‑or‑nothing**: Operations either complete fully or have zero effect
2. **Buffer consistency**: `ArcSwap` ensures readers see consistent buffer state
3. **Memory safety**: No dangling pointers or invalid references after operations
4. **Concurrent access**: Readers never blocked, always see valid buffer
5. **State preservation**: Failed operations leave original state unchanged
6. **Transaction integrity**: Multi‑step operations atomic as a whole

## Implementation Notes

- Use `ArcSwap::load` for reads, `ArcSwap::store` for writes
- Implement copy‑on‑write pattern for modifications
- Maintain transaction log for rollback capability
- Validate buffer integrity after each operation
- Test with various table schemas and field types
- Include stress tests with high concurrency
