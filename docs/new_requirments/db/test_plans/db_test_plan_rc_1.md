# Test Plan for task_rc_1: POST /table/{name}/record

## Overview

Unit tests for the REST API endpoint that inserts records into tables via JSON body.

## Test Cases

### 1. test_insert_record_success

**Description**: Insert valid record into existing table with correct field types
**Verifies**: Record insertion success, correct ID returned, data stored in buffer
**Edge cases**: None (happy path)
**Assertions**: HTTP 201 Created, response contains record ID, data retrievable via GET

### 2. test_insert_record_nonexistent_table

**Description**: Attempt insert into non-existent table
**Verifies**: Error handling for invalid table name
**Edge cases**: Table doesn't exist, malformed table name
**Assertions**: HTTP 404 Not Found, clear error message

### 3. test_insert_record_malformed_json

**Description**: Send invalid JSON in request body
**Verifies**: JSON parsing errors handled gracefully
**Edge cases**: Invalid JSON syntax, empty body, wrong content-type
**Assertions**: HTTP 400 Bad Request, JSON parsing error message

### 4. test_insert_record_type_mismatch

**Description**: Send JSON with field values mismatching schema types
**Verifies**: Type validation works correctly
**Edge cases**: String for integer field, wrong array size for composite types, null for non-nullable fields
**Assertions**: HTTP 400 Bad Request, type mismatch error

### 5. test_insert_record_missing_required_fields

**Description**: Omit required fields from JSON
**Verifies**: Field presence validation
**Edge cases**: Missing non-nullable fields, partial field set
**Assertions**: HTTP 400 Bad Request, missing field error

### 6. test_insert_record_extra_fields

**Description**: Include extra fields not in schema
**Verifies**: Schema validation for extra fields
**Edge cases**: Extra fields in JSON, field name typos
**Assertions**: HTTP 400 Bad Request or ignore extra fields (design decision)

### 7. test_insert_record_concurrent_writes

**Description**: Multiple concurrent insertions into same table
**Verifies**: ArcSwap buffer swapping works, no data corruption
**Edge cases**: Parallel writes, race conditions
**Assertions**: All records inserted successfully, no lost data, atomicity maintained

### 8. test_insert_record_buffer_expansion

**Description**: Insert enough records to trigger buffer reallocation
**Verifies**: Buffer capacity management, zero-copy access after reallocation
**Edge cases**: Buffer growth, memory safety during resize
**Assertions**: Records remain accessible after resize, no memory errors

### 9. test_insert_record_custom_type_validation

**Description**: Insert record with custom composite type (e.g., Vec3)
**Verifies**: Custom type validation and serialization
**Edge cases**: Wrong array length for composite type, nested structures
**Assertions**: HTTP 201 Created, custom type stored correctly

### 10. test_insert_record_relation_integrity

**Description**: Insert record with foreign key references
**Verifies**: Referential integrity validation (if implemented)
**Edge cases**: Invalid foreign key, circular references
**Assertions**: HTTP 400 Bad Request for invalid references, success with valid references

### 11. test_insert_record_atomicity

**Description**: Simulate partial failure during insertion
**Verifies**: Atomic operation - all succeeds or nothing
**Edge cases**: Buffer write failure, out-of-memory, panic during insertion
**Assertions**: No partial data written, transaction rollback works

### 12. test_insert_record_performance

**Description**: Measure insertion latency for benchmarking
**Verifies**: Performance meets low latency requirements
**Edge cases**: Large records, many concurrent inserts
**Assertions**: Insertion completes within acceptable time bounds

## Edge Cases Summary

- Non-existent table
- Malformed JSON syntax
- Type mismatches (string vs integer, wrong array sizes)
- Missing required fields
- Extra unknown fields
- Null values for non-nullable fields
- Buffer capacity limits
- Concurrent modifications
- Custom/composite type validation
- Referential integrity constraints
- Atomicity under failure conditions
- Unicode/UTF-8 in field names/values
- Very large record sizes
- Empty table name in URL
- SQL injection attempts (though no SQL)
- Content-Type header validation
