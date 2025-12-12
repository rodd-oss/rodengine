# Test Plan for DELETE `/table/{name}/record/{id}` (task_rc_4)

## Overview

Unit tests for the record deletion endpoint in a Rust-based in-memory relational database. Tests verify atomicity, concurrency, referential integrity, and API compliance per TRD requirements.

## 1. Basic Functionality Tests

### `test_delete_existing_record`

**Verifies**: Successful deletion of an existing record

- Creates table with fields, inserts record, deletes via API
- **Assertions**: Record no longer exists, returns 204 No Content
- **Verifies**: Storage buffer updated atomically

### `test_delete_record_returns_correct_id`

**Verifies**: API returns deleted record ID for confirmation

- **Assertions**: Response contains deleted record ID

## 2. Error Condition Tests

### `test_delete_nonexistent_table`

**Verifies**: Attempt to delete from non-existent table

- **Assertions**: Returns 404 Not Found with appropriate error message

### `test_delete_nonexistent_record`

**Verifies**: Attempt to delete non-existent record ID

- **Assertions**: Returns 404 Not Found, no changes to storage

### `test_delete_invalid_record_id_format`

**Verifies**: Invalid ID format handling

- **Test cases**: Non-numeric, negative, overflow values
- **Assertions**: Returns 400 Bad Request with validation error

## 3. Atomicity & Concurrency Tests

### `test_delete_atomic_on_partial_failure`

**Verifies**: Transaction rollback on failure

- Mocks storage failure during buffer swap
- **Assertions**: Transaction rolled back, original record preserved
- **Verifies**: ArcSwap maintains consistent state

### `test_concurrent_delete_and_read`

**Verifies**: Lock-free reads during delete operation

- Spawns reader thread that loads buffer via ArcSwap::load
- **Assertions**: Reader sees consistent state (either old or new buffer)
- **Verifies**: No data races

### `test_concurrent_deletes_on_same_record`

**Verifies**: Race condition handling for same record

- Multiple threads attempting to delete same record
- **Assertions**: Only one succeeds, others get 409 Conflict or 404

## 4. Referential Integrity Tests

### `test_delete_record_with_foreign_key_constraint`

**Verifies**: Referential integrity enforcement

- Creates parent-child tables with relation
- Attempts to delete parent record
- **Assertions**: Returns 409 Conflict (or configurable: cascade/restrict)
- **Verifies**: Relation enforcement from task_rl_2

### `test_delete_with_cascade_relation`

**Verifies**: Cascade deletion option

- **Assertions**: Parent and child records both deleted
- **Verifies**: Atomic transaction across multiple tables

### `test_delete_record_referencing_other_table`

**Verifies**: Deleting referencing record (not owner)

- Should succeed (only foreign key owner needs protection)

## 5. Storage & Memory Tests

### `test_delete_reclaims_storage_space`

**Verifies**: Buffer compaction after delete

- **Assertions**: Storage buffer size reduces appropriately
- **Verifies**: No memory leaks from ArcSwap references

### `test_delete_last_record`

**Verifies**: Empty table handling

- Delete only record in table
- **Assertions**: Table remains with zero records
- **Verifies**: Buffer management handles empty state

### `test_delete_middle_record_packing`

**Verifies**: Tight packing preservation

- Delete record from middle of buffer
- **Assertions**: Remaining records maintain tight packing
- **Verifies**: No padding introduced, cache efficiency preserved

## 6. API Contract Tests

### `test_delete_response_format`

**Verifies**: HTTP response compliance

- **Assertions**: Correct status code (204), appropriate headers
- **Verifies**: No body for successful delete (or minimal confirmation)

### `test_delete_with_authentication`

**Verifies**: Authentication integration (if implemented)

- **Assertions**: Unauthorized requests rejected (401)

## 7. Edge Cases

### `test_delete_record_at_max_id`

**Verifies**: ID boundary conditions

- Record with maximum usize ID

### `test_delete_after_schema_change`

**Verifies**: Schema versioning handling

- Delete record after field added/removed
- **Assertions**: Handles schema changes correctly

### `test_delete_during_parallel_iteration`

**Verifies**: Consistency during parallel access

- Delete while parallel procedure iterates (rayon from task_pp_1)
- **Assertions**: Iterator sees consistent snapshot

### `test_delete_with_custom_type_fields`

**Verifies**: Custom type handling

- Record with user-defined composite types (e.g., `3xf32`)

## 8. Performance Tests

### `test_delete_latency_measurement`

**Verifies**: Meets tickrate requirements

- **Assertions**: Operation latency supports 15-120 Hz tickrate
- **Verifies**: Atomic operation doesn't block readers

### `test_bulk_delete_performance`

**Verifies**: Linear scaling

- Delete many records sequentially
- **Verifies**: No quadratic behavior

## Key Assertions

1. **Atomicity**: Either all changes applied or none (check via transaction log)
2. **Concurrency**: Readers never see partially deleted state
3. **Memory Safety**: No out-of-bounds access after deletion
4. **Referential Integrity**: Relations properly enforced
5. **Storage Efficiency**: Buffer remains tightly packed
6. **API Compliance**: Correct HTTP status codes and error messages

## Test Dependencies

- Mock HTTP layer for endpoint testing
- In-memory database instance with configurable state
- ArcSwap mocking for failure injection
- Relation schema setup utilities
- Concurrent test helpers for race condition verification
