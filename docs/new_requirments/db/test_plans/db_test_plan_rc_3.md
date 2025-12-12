# Test Plan for task_rc_3: PUT /table/{name}/record/{id}

## Overview

Unit tests for updating records via REST API endpoint in Rust-based relational in-memory database.

## 1. Basic Update Tests

- **test_update_record_success**: Successful update of existing record with valid JSON body
- **test_update_record_returns_updated_data**: PUT returns the updated record
- **test_update_partial_fields**: Updating only specified fields leaves other fields unchanged

## 2. Error Condition Tests

- **test_update_nonexistent_table**: Returns 404 when table doesn't exist
- **test_update_nonexistent_record**: Returns 404 when record ID doesn't exist
- **test_update_invalid_json**: Returns 400 for malformed JSON body
- **test_update_type_mismatch**: Returns 400 when field type doesn't match schema
- **test_update_missing_required_fields**: Returns 400 when required fields are missing
- **test_update_extra_fields**: Returns 400 when JSON contains fields not in schema

## 3. Concurrency & Atomicity Tests

- **test_update_atomicity**: Update is atomic (all-or-nothing)
- **test_concurrent_update_same_record**: Concurrent updates to same record with ArcSwap
- **test_update_during_read**: Readers see consistent state during update
- **test_update_rollback_on_panic**: Failed updates don't corrupt data

## 4. Edge Cases

- **test_update_record_at_boundary**: Updates record at buffer boundaries
- **test_update_with_null_values**: Handles null/None values appropriately
- **test_update_empty_body**: Returns 400 for empty update body
- **test_update_record_id_immutable**: Record ID cannot be changed via update
- **test_update_with_relations**: Updates record with foreign key relations

## 5. Performance & Memory Safety

- **test_update_zero_copy**: Update uses zero-copy semantics where possible
- **test_update_buffer_integrity**: Buffer bounds are maintained
- **test_update_cache_locality**: Packed structure remains cache-friendly
- **test_update_large_record**: Handles updates to large/complex records

## 6. Schema Evolution

- **test_update_after_schema_change**: Updates record after table schema modification
- **test_update_with_default_values**: Applies default values for unspecified fields

## Key Assertions

- HTTP status codes (200, 400, 404)
- Response body matches updated record
- Atomic operation verification via transaction log
- Buffer integrity checks (no out-of-bounds access)
- Concurrency safety (no data races)
- Schema validation errors
- Type safety enforcement

## Edge Cases to Consider

- Invalid UTF-8 in field names/values
- Maximum/minimum values for numeric types
- Floating-point precision changes
- Boolean field updates
- Custom composite type updates (e.g., Vec3)
- Empty table updates
- Sequential vs random record updates
- Power failure simulation (atomicity)
