# Test Plan for DELETE /table/{name} (task_rs_2)

## Test Cases

### 1. **test_delete_existing_empty_table**

- **Description**: Delete a table with no records
- **Verifies**: Table is removed from catalog, storage buffer is deallocated, HTTP 204 No Content returned
- **Edge Cases**: Table exists but empty
- **Assertions**: Table no longer in catalog, subsequent operations on table fail

### 2. **test_delete_existing_table_with_records**

- **Description**: Delete a table containing records
- **Verifies**: All records are removed, storage buffer cleared, referential integrity maintained
- **Edge Cases**: Table with multiple records, varying field types
- **Assertions**: Table removed, no memory leaks, relations referencing deleted table handled

### 3. **test_delete_nonexistent_table**

- **Description**: Attempt to delete a table that doesn't exist
- **Verifies**: HTTP 404 Not Found returned, no side effects
- **Edge Cases**: Invalid table name, malformed URL
- **Assertions**: Catalog unchanged, error response with appropriate message

### 4. **test_delete_table_with_active_relations**

- **Description**: Delete table that has relations to/from other tables
- **Verifies**: Referential integrity constraints enforced
- **Edge Cases**: Multiple relations, circular dependencies
- **Assertions**: Either cascade delete or reject with HTTP 409 Conflict, relations updated/removed

### 5. **test_concurrent_delete_and_read**

- **Description**: Concurrent deletion while reads are in progress
- **Verifies**: ArcSwap buffer swapping works correctly, readers continue with old buffer
- **Edge Cases**: Multiple concurrent readers during deletion
- **Assertions**: No data races, readers complete without panic, new buffer empty

### 6. **test_delete_table_name_validation**

- **Description**: Test various table name formats
- **Verifies**: Name validation (alphanumeric, underscores, length limits)
- **Edge Cases**: Empty string, special characters, very long names
- **Assertions**: Invalid names rejected with HTTP 400 Bad Request

### 7. **test_delete_table_atomicity**

- **Description**: Ensure deletion is atomic (all-or-nothing)
- **Verifies**: If deletion fails mid-operation, database remains consistent
- **Edge Cases**: Simulated failure during buffer deallocation or catalog update
- **Assertions**: Transaction log rollback works, partial state not visible

### 8. **test_delete_table_permissions**

- **Description**: Authorization checks (if implemented)
- **Verifies**: Only authorized users can delete tables
- **Edge Cases**: Missing/invalid authentication tokens
- **Assertions**: HTTP 401 Unauthorized or 403 Forbidden as appropriate

### 9. **test_delete_table_schema_persistence**

- **Description**: Verify schema JSON file is updated after deletion
- **Verifies**: Schema persistence to disk reflects deletion
- **Edge Cases**: Concurrent schema file writes
- **Assertions**: JSON file updated atomically, contains correct schema state

### 10. **test_delete_table_performance**

- **Description**: Measure deletion time for large tables
- **Verifies**: Deletion scales appropriately with table size
- **Edge Cases**: Very large tables (millions of records)
- **Assertions**: Deletion completes within acceptable time bounds

## Edge Cases to Consider:

1. **Concurrent modifications**: Delete while records are being inserted/updated
2. **Memory pressure**: Delete during low memory conditions
3. **Recovery**: Database restart after failed deletion
4. **Case sensitivity**: Table name matching (case-sensitive vs insensitive)
5. **Unicode names**: Tables with non-ASCII characters
6. **System tables**: Attempt to delete internal/system tables
7. **Network failures**: Client disconnects during deletion
8. **Duplicate requests**: Same DELETE request sent multiple times (idempotency)

## Expected Behaviors:

- **Success**: HTTP 204 No Content, table removed from catalog
- **Not Found**: HTTP 404, error message "Table 'name' not found"
- **Conflict**: HTTP 409 if table has active relations (unless cascade)
- **Bad Request**: HTTP 400 for invalid table names
- **Atomic**: Either fully deleted or no change (transactional)
- **Thread-safe**: Concurrent operations handled correctly via ArcSwap
