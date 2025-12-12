# Test Plan for task_rc_2: GET /table/{name}/record/{id}

## Overview

Unit tests for retrieving a single record via REST API endpoint `GET /table/{name}/record/{id}` in a relational in-memory database for online games.

## 1. Happy Path Tests

### test_get_existing_record

- **Description**: Retrieve a valid record from an existing table
- **Verifies**: Returns correct record data with 200 OK
- **Assertions**:
  - HTTP status 200
  - Response body matches inserted data
  - Content-Type: application/json
  - Record ID in response matches requested ID

### test_get_record_with_different_field_types

- **Description**: Retrieve record with various field types (i32, f32, bool, custom types)
- **Verifies**: All field types are correctly serialized to JSON
- **Edge cases**:
  - Floating point precision
  - Boolean values (true/false)
  - Custom composite types (e.g., Vec3 as 3×f32)
  - Nullable fields
- **Assertions**:
  - JSON serialization preserves type semantics
  - Custom types serialize to expected format

## 2. Error Condition Tests

### test_get_nonexistent_table

- **Description**: Attempt to retrieve from non-existent table
- **Verifies**: Returns 404 Not Found with appropriate error message
- **Edge cases**:
  - Table name with special characters
  - Table name with spaces
  - Unicode table names
- **Assertions**:
  - HTTP status 404
  - Error message indicates table not found
  - No panic or undefined behavior

### test_get_nonexistent_record_id

- **Description**: Attempt to retrieve non-existent record ID
- **Verifies**: Returns 404 Not Found
- **Edge cases**:
  - ID out of bounds (greater than max record count)
  - Negative ID
  - Zero ID (if 0 is invalid)
  - ID that was recently deleted
- **Assertions**:
  - HTTP status 404
  - Error message indicates record not found

### test_get_record_invalid_id_format

- **Description**: Invalid ID format in URL
- **Verifies**: Returns 400 Bad Request
- **Edge cases**:
  - Non-numeric ID ("abc", "123abc")
  - Floating point ID ("123.45")
  - Empty ID string
  - ID with leading/trailing whitespace
  - ID exceeding usize::MAX
- **Assertions**:
  - HTTP status 400
  - Error message indicates invalid ID format

### test_get_record_empty_table_name

- **Description**: Empty table name in URL
- **Verifies**: Returns 400 Bad Request
- **Assertions**:
  - HTTP status 400
  - Error message indicates invalid table name

### test_get_record_malformed_url

- **Description**: Malformed URL patterns
- **Verifies**: Returns 400 Bad Request
- **Edge cases**:
  - Missing table name segment
  - Missing record ID segment
  - Extra path segments
  - Path traversal attempts ("../other/1")
- **Assertions**:
  - HTTP status 400
  - Router rejects malformed patterns

## 3. Concurrency Tests

### test_get_record_during_concurrent_write

- **Description**: Read while record is being updated
- **Verifies**: Returns consistent snapshot (ArcSwap ensures readers see old buffer)
- **Assertions**:
  - No data races
  - Returns either old or new version consistently
  - No panics or UB during concurrent access
  - ArcSwap load provides atomic view

### test_get_record_parallel_reads

- **Description**: Multiple concurrent reads of same record
- **Verifies**: All readers get same data, no deadlocks
- **Edge case**: High contention scenarios (many concurrent readers)
- **Assertions**:
  - All concurrent reads succeed
  - All readers see identical data
  - No deadlocks or starvation
  - Lock-free reads work as expected

## 4. Memory Safety Tests

### test_get_record_after_table_deletion

- **Description**: Attempt to read after table deleted
- **Verifies**: Returns 404 Not Found (not panic or UB)
- **Edge case**: Race condition between delete and read
- **Assertions**:
  - HTTP status 404
  - Graceful error handling, no panic
  - Memory safety maintained

### test_get_record_corrupted_buffer

- **Description**: Read from buffer with potential corruption
- **Verifies**: Graceful error handling, not panic
- **Edge cases**:
  - Buffer size mismatch with schema
  - Invalid field offsets
  - Malformed record data
- **Assertions**:
  - Returns 500 Internal Server Error or similar
  - Error logged appropriately
  - No undefined behavior

## 5. Performance & Cache Tests

### test_get_record_cache_efficiency

- **Description**: Verify tight packing doesn't cause misalignment
- **Verifies**: Field access doesn't cause cache misses due to padding
- **Assertions**:
  - Record size matches calculated packed size
  - Field offsets are properly aligned for CPU cache
  - No unnecessary padding bytes

### test_get_record_zero_copy

- **Description**: Verify returned references don't copy data
- **Verifies**: References point to original buffer locations
- **Edge case**: Lifetime validation of returned references
- **Assertions**:
  - Field accessors return &T not T
  - No unnecessary allocations during read
  - References remain valid for duration of read

## 6. Schema Evolution Tests

### test_get_record_after_schema_change

- **Description**: Read record after field added/removed
- **Verifies**: Returns only current schema fields, handles missing fields gracefully
- **Edge case**: Schema change during read operation
- **Assertions**:
  - Returns data for current schema fields only
  - Missing fields (from old schema) are omitted
  - New fields (added after record creation) have default values
  - No panic on schema mismatch

## 7. API Contract Tests

### test_get_record_response_format

- **Description**: Verify JSON response format
- **Verifies**: Proper JSON structure with field names and values
- **Assertions**:
  - Content-Type: application/json
  - Valid JSON syntax
  - Field names match schema
  - Values match expected types
  - No extra fields in response

### test_get_record_headers

- **Description**: Verify appropriate HTTP headers
- **Verifies**: CORS headers, cache headers if implemented
- **Assertions**:
  - Appropriate CORS headers if configured
  - Cache-control headers if implemented
  - No sensitive headers exposed

## 8. Integration Tests

### test_get_record_following_post

- **Description**: Create then immediately retrieve record
- **Verifies**: End-to-end workflow, ID assignment consistency
- **Edge case**: Race between create and read
- **Assertions**:
  - Record can be retrieved immediately after creation
  - Retrieved data matches created data
  - ID assignment is consistent

### test_get_record_with_relations

- **Description**: Retrieve record that has relations to other tables
- **Verifies**: Relation data not included unless explicitly requested
- **Edge case**: Circular relations
- **Assertions**:
  - Basic record data returned without relation data
  - No automatic inclusion of related records
  - Relation integrity maintained

## Key Edge Cases Summary

1. **ID boundaries**:

   - First record (ID=1)
   - Last record in table
   - ID > usize::MAX
   - Recently deleted ID

2. **Table name variations**:

   - Unicode/UTF-8 characters
   - Special characters
   - Case sensitivity
   - Empty string

3. **Concurrent operations**:

   - Read during write
   - Read during schema change
   - Read during table deletion
   - Multiple concurrent readers

4. **Memory safety**:

   - Buffer corruption scenarios
   - Out-of-bounds access prevention
   - Lifetime management of references
   - ArcSwap buffer swapping

5. **Performance**:

   - Cache locality verification
   - Zero-copy access
   - Response time under load
   - Memory usage patterns

6. **Error resilience**:
   - Malformed input handling
   - Network timeout simulation
   - Resource exhaustion scenarios
   - Graceful degradation

## Assertion Categories

### HTTP Level

- Status codes (200, 404, 400, 500)
- Response headers
- Content-Type correctness
- Error message clarity

### Data Integrity

- Retrieved data matches stored data
- Field type preservation
- JSON serialization correctness
- No data corruption

### Concurrency & Atomicity

- Consistent snapshots
- No data races
- Lock-free read guarantees
- Thread safety

### Memory Safety

- No undefined behavior
- No memory leaks
- Bounds checking
- Lifetime safety

### Performance

- Response time bounds
- Cache efficiency
- Zero-copy verification
- No unnecessary allocations

## Test Implementation Notes

- Use Rust's `#[test]` attribute for unit tests
- Mock HTTP layer for isolated testing
- Use property-based testing for edge cases
- Include benchmarks for performance-critical paths
- Test both synchronous and async paths if applicable
- Verify error paths don't panic
- Ensure tests are deterministic and repeatable
