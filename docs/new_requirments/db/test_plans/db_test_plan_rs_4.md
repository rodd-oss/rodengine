# Test Plan for DELETE `/table/{name}/field/{fieldName}`

## 1. Basic Functionality Tests

- **`test_delete_field_success`**: Verify successful field deletion from existing table

  - Create table with multiple fields
  - Delete one field via API
  - Assert field is removed from schema
  - Assert remaining fields are unaffected
  - Verify HTTP 200/204 response

- **`test_delete_last_field`**: Delete the only field from a table
  - Create table with single field
  - Delete the field
  - Assert table exists but has no fields
  - Verify table can still accept new fields

## 2. Error Condition Tests

- **`test_delete_field_nonexistent_table`**: Attempt to delete field from non-existent table

  - Call DELETE on non-existent table name
  - Assert HTTP 404 with appropriate error message
  - Verify no schema changes

- **`test_delete_field_nonexistent_field`**: Attempt to delete non-existent field

  - Create table with fields
  - Call DELETE with non-existent field name
  - Assert HTTP 404 with appropriate error message
  - Verify existing fields unchanged

- **`test_delete_field_case_sensitive`**: Verify field name matching is case-sensitive
  - Create field "username"
  - Attempt to delete "UserName" or "USERNAME"
  - Assert HTTP 404 (not found)

## 3. Data Integrity Tests

- **`test_delete_field_with_existing_data`**: Delete field that has data in records

  - Create table with fields including target field
  - Insert records with data in all fields
  - Delete target field
  - Assert:
    - Field removed from schema
    - Records still exist but without deleted field
    - Remaining field data preserved
    - Storage buffer properly compacted (no orphaned data)

- **`test_delete_field_recalculates_offsets`**: Verify field deletion triggers offset recalculation
  - Create table with fields A, B, C
  - Delete field B (middle field)
  - Assert field C's offset adjusted correctly
  - Verify record size reduced appropriately

## 4. Concurrency & Atomicity Tests

- **`test_delete_field_atomic`**: Verify operation is atomic

  - Simulate concurrent reads during field deletion
  - Assert readers see consistent state (either old or new schema)
  - No partial schema visible

- **`test_delete_field_with_concurrent_writes`**: Delete field while concurrent writes occur
  - Start field deletion
  - Concurrently attempt to insert records using the field
  - Assert either:
    - Writes fail gracefully (field not found)
    - Deletion completes atomically before/after writes

## 5. Schema Validation Tests

- **`test_delete_field_preserves_other_metadata`**: Verify only field metadata removed

  - Create table with relations, custom types
  - Delete a field
  - Assert:
    - Table name unchanged
    - Other fields unchanged
    - Relations unaffected (unless they reference deleted field - see below)
    - Custom type registry intact

- **`test_delete_field_referenced_by_relation`**: Attempt to delete field used in relation
  - Create relation referencing the field
  - Attempt to delete field
  - Assert:
    - HTTP 409 Conflict or similar
    - Field not deleted
    - Relation intact
    - Appropriate error message about referential integrity

## 6. Edge Cases

- **`test_delete_field_empty_name`**: Attempt with empty field name string

  - Assert HTTP 400 Bad Request

- **`test_delete_field_special_characters`**: Field names with special chars

  - Create field with underscores, hyphens
  - Verify deletion works with URL encoding

- **`test_delete_field_unicode`**: Unicode field names

  - Create field with non-ASCII characters
  - Verify proper URL encoding/decoding

- **`test_delete_field_twice`**: Delete same field twice
  - Delete field successfully
  - Attempt to delete same field again
  - Assert HTTP 404 (already deleted)

## 7. Performance & Memory Tests

- **`test_delete_field_memory_reclaim`**: Verify storage buffer memory reclaimed

  - Create large table with many records
  - Delete field
  - Assert buffer capacity reduced appropriately
  - Verify no memory leaks

- **`test_delete_field_performance_large_schema`**: Performance with many fields
  - Table with 100+ fields
  - Delete field from middle
  - Measure operation time
  - Assert linear/acceptable time complexity

## 8. API Contract Tests

- **`test_delete_field_response_format`**: Verify API response format

  - Check response headers (Content-Type, etc.)
  - Verify empty body or success message
  - Confirm appropriate status code (200 OK or 204 No Content)

- **`test_delete_field_method_validation`**: Verify only DELETE method accepted
  - Attempt GET, POST, PUT to same endpoint
  - Assert HTTP 405 Method Not Allowed

## Assertions & Expected Behaviors

1. **Schema Consistency**: After deletion, schema must be internally consistent
2. **Data Preservation**: Remaining field data must be intact
3. **Atomicity**: Operation all-or-nothing; no partial states
4. **Error Handling**: Clear error messages for invalid operations
5. **Concurrency Safety**: No data races or inconsistent reads
6. **Memory Safety**: No buffer overflows or use-after-free
7. **HTTP Compliance**: Proper status codes and headers

**Key Edge Cases**: Non-existent resources, concurrent operations, referential integrity constraints, storage buffer compaction, Unicode handling, and large-scale performance.
