# Test Plan for POST /table Endpoint (task_rs_1)

## Test Cases

### 1. **test_create_table_success**

- **Description**: Valid table creation with proper name and fields
- **Verifies**: Endpoint returns 201 Created, table appears in schema catalog
- **Edge Cases**: None (happy path)
- **Assertions**: Status 201, response contains table metadata, schema includes new table

### 2. **test_create_table_duplicate_name**

- **Description**: Attempt to create table with existing name
- **Verifies**: Endpoint returns 409 Conflict or 400 Bad Request
- **Edge Cases**: Case sensitivity? (Assume case-sensitive)
- **Assertions**: Status 409/400, error message indicates duplicate name

### 3. **test_create_table_invalid_json**

- **Description**: Malformed JSON payload
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Missing fields, wrong types, extra fields
- **Assertions**: Status 400, error indicates JSON parsing failure

### 4. **test_create_table_missing_name**

- **Description**: JSON missing required "name" field
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Empty string name, null name
- **Assertions**: Status 400, error indicates missing/invalid name

### 5. **test_create_table_invalid_name**

- **Description**: Invalid table name (empty, too long, special chars)
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Empty string, whitespace-only, SQL keywords, special characters
- **Assertions**: Status 400, error indicates invalid name format

### 6. **test_create_table_missing_fields**

- **Description**: JSON missing "fields" array
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Empty fields array, null fields
- **Assertions**: Status 400, error indicates missing/invalid fields

### 7. **test_create_table_invalid_field_structure**

- **Description**: Field objects missing required properties
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Missing field name, missing field type, invalid field type
- **Assertions**: Status 400, error indicates invalid field definition

### 8. **test_create_table_duplicate_field_names**

- **Description**: Multiple fields with same name in same table
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Case-sensitive field names
- **Assertions**: Status 400, error indicates duplicate field names

### 9. **test_create_table_unsupported_field_type**

- **Description**: Field type not in supported type list
- **Verifies**: Endpoint returns 400 Bad Request
- **Edge Cases**: Custom types not yet registered, malformed type strings
- **Assertions**: Status 400, error indicates unsupported field type

### 10. **test_create_table_storage_buffer_initialized**

- **Description**: Verify storage buffer is created with table
- **Verifies**: Table has associated Vec<u8> buffer with proper capacity
- **Edge Cases**: Zero records initially, buffer allocation
- **Assertions**: Table storage exists, buffer capacity matches configuration

### 11. **test_create_table_schema_persistence**

- **Description**: Verify table appears in schema JSON after creation
- **Verifies**: Schema serialization includes new table
- **Edge Cases**: Schema file updates, concurrent schema reads
- **Assertions**: Schema contains table definition, fields properly serialized

### 12. **test_create_table_concurrent_requests**

- **Description**: Multiple concurrent table creation requests
- **Verifies**: Thread safety, no race conditions in catalog updates
- **Edge Cases**: Same name from different threads, catalog corruption
- **Assertions**: Only one table created per unique name, no panics

### 13. **test_create_table_with_custom_type**

- **Description**: Table creation with user-defined composite type
- **Verifies**: Custom types (e.g., Vec3 as 3×f32) are accepted
- **Edge Cases**: Type registry lookup, type validation
- **Assertions**: Status 201, custom type properly registered

### 14. **test_create_table_field_offsets_calculated**

- **Description**: Verify field offsets calculated correctly for tight packing
- **Verifies**: Field offsets follow tight packing (no padding)
- **Edge Cases**: Mixed type sizes, alignment requirements
- **Assertions**: Offsets sum to record size, no gaps between fields

### 15. **test_create_table_record_size_calculation**

- **Description**: Verify record size calculated from field definitions
- **Verifies**: Record size matches sum of field sizes
- **Edge Cases**: Zero fields (empty table), large composite types
- **Assertions**: Record size correct, matches storage allocation

## Edge Cases Summary

- Invalid/malformed JSON payloads
- Missing required fields (name, fields)
- Invalid table names (empty, special chars, SQL keywords)
- Duplicate table names
- Invalid field definitions (missing name/type)
- Duplicate field names within table
- Unsupported field types
- Empty fields array
- Concurrent creation requests
- Custom type validation
- Storage buffer initialization
- Schema persistence updates
