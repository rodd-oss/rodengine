# Test Plan for POST /table/{name}/field (task_rs_3)

## 1. Happy Path Tests

- **`test_add_field_to_existing_table`**: Add valid field to existing table

  - Verifies: Returns 201 Created, field appears in schema, record size recalculated
  - Assertions: HTTP status 201, field exists in table schema, storage buffer capacity updated

- **`test_add_multiple_fields_sequentially`**: Add multiple fields to same table
  - Verifies: Each field added successfully, offsets calculated correctly
  - Assertions: All fields present, offsets sequential with tight packing

## 2. Error Condition Tests

- **`test_add_field_nonexistent_table`**: Attempt to add field to non-existent table

  - Verifies: Returns 404 Not Found
  - Assertions: HTTP status 404, error message indicates table not found

- **`test_add_duplicate_field_name`**: Add field with name that already exists

  - Verifies: Returns 409 Conflict or 400 Bad Request
  - Assertions: HTTP status 409/400, error indicates duplicate field name

- **`test_add_field_invalid_type`**: Add field with unsupported type

  - Verifies: Returns 400 Bad Request
  - Assertions: HTTP status 400, error indicates invalid type

- **`test_add_field_empty_name`**: Add field with empty name string

  - Verifies: Returns 400 Bad Request
  - Assertions: HTTP status 400, error indicates invalid field name

- **`test_add_field_malformed_json`**: Send malformed JSON payload
  - Verifies: Returns 400 Bad Request
  - Assertions: HTTP status 400, error indicates JSON parsing failure

## 3. Edge Case Tests

- **`test_add_field_to_table_with_existing_data`**: Add field to table with existing records

  - Verifies: Existing records remain valid, new field has default value (null/zero)
  - Assertions: Record count unchanged, new field accessible with default value

- **`test_add_field_maximum_fields`**: Attempt to add beyond maximum fields per table

  - Verifies: Returns 400 Bad Request or implements limit
  - Assertions: Either rejects or enforces field limit

- **`test_add_field_concurrent_requests`**: Multiple concurrent add field requests

  - Verifies: Atomic operations prevent race conditions
  - Assertions: All fields added correctly or proper conflict resolution

- **`test_add_field_special_characters`**: Field name with special characters
  - Verifies: Validates naming conventions
  - Assertions: Either accepts valid names or rejects invalid ones

## 4. Type System Tests

- **`test_add_field_builtin_types`**: Add all supported built-in types (i32, u64, f32, bool, etc.)

  - Verifies: Each type accepted and sized correctly
  - Assertions: Type sizes match expected byte counts

- **`test_add_field_custom_composite_type`**: Add user-defined composite type (e.g., Vec3)
  - Verifies: Custom type from registry accepted
  - Assertions: Composite type size calculated correctly

## 5. Schema Persistence Tests

- **`test_add_field_persists_to_schema_json`**: Added field appears in serialized schema
  - Verifies: Schema JSON includes new field after addition
  - Assertions: Serialized schema contains field definition

## 6. Performance/Concurrency Tests

- **`test_add_field_atomicity`**: Field addition is atomic (all-or-nothing)

  - Verifies: Partial failures don't corrupt schema
  - Assertions: Schema consistent after failed addition attempt

- **`test_add_field_does_not_block_reads`**: Adding field doesn't block concurrent reads
  - Verifies: ArcSwap buffer swap allows reads during write
  - Assertions: Read operations continue during field addition

## Key Edge Cases to Consider:

1. **Non-existent table** → 404
2. **Duplicate field name** → 409/400
3. **Invalid type identifier** → 400
4. **Empty/malformed field name** → 400
5. **Maximum field limit reached** → 400
6. **Concurrent modifications** → Atomic resolution
7. **Table with existing data** → Default values for new field
8. **Custom composite types** → Registry validation
9. **Malformed JSON payload** → 400
10. **Missing required parameters** → 400

## Expected Behaviors:

- **Success**: 201 Created, field added to schema, storage buffer updated
- **Validation**: All inputs validated before schema modification
- **Atomicity**: Operation atomic - either fully applied or rolled back
- **Concurrency**: Lock-free reads during write via ArcSwap
- **Persistence**: Schema changes reflected in JSON serialization
