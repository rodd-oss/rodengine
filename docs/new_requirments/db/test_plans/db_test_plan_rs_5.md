# Test Plan for POST /relation Endpoint (task_rs_5)

## Test Plan for POST /relation Endpoint (task_rs_5)

### 1. **test_create_relation_success**

- **Description**: Create a valid relation between two existing tables with valid field mapping
- **Verifies**: Relation is created successfully, returns 201 Created with relation ID
- **Edge Cases**: None (happy path)
- **Assertions**:
  - HTTP status 201
  - Response contains relation ID
  - Relation appears in schema
  - Referential integrity tracking initialized

### 2. **test_create_relation_source_table_not_found**

- **Description**: Attempt to create relation with non-existent source table
- **Verifies**: Returns 404 Not Found with appropriate error message
- **Edge Cases**: Source table doesn't exist
- **Assertions**:
  - HTTP status 404
  - Error message indicates missing source table
  - No relation created

### 3. **test_create_relation_destination_table_not_found**

- **Description**: Attempt to create relation with non-existent destination table
- **Verifies**: Returns 404 Not Found with appropriate error message
- **Edge Cases**: Destination table doesn't exist
- **Assertions**:
  - HTTP status 404
  - Error message indicates missing destination table
  - No relation created

### 4. **test_create_relation_source_field_not_found**

- **Description**: Attempt to create relation with non-existent source field
- **Verifies**: Returns 400 Bad Request with field validation error
- **Edge Cases**: Source field doesn't exist in source table
- **Assertions**:
  - HTTP status 400
  - Error message indicates invalid source field
  - No relation created

### 5. **test_create_relation_destination_field_not_found**

- **Description**: Attempt to create relation with non-existent destination field
- **Verifies**: Returns 400 Bad Request with field validation error
- **Edge Cases**: Destination field doesn't exist in destination table
- **Assertions**:
  - HTTP status 400
  - Error message indicates invalid destination field
  - No relation created

### 6. **test_create_relation_duplicate_relation**

- **Description**: Attempt to create duplicate relation (same source/destination tables and fields)
- **Verifies**: Returns 409 Conflict or 400 Bad Request
- **Edge Cases**: Duplicate relation detection
- **Assertions**:
  - HTTP status 409/400
  - Error message indicates duplicate relation
  - Only one relation exists

### 7. **test_create_relation_invalid_field_type_mapping**

- **Description**: Attempt to create relation with incompatible field types (e.g., i32 to string)
- **Verifies**: Returns 400 Bad Request with type mismatch error
- **Edge Cases**: Type compatibility validation
- **Assertions**:
  - HTTP status 400
  - Error message indicates type mismatch
  - No relation created

### 8. **test_create_relation_self_referential**

- **Description**: Create relation where source and destination are the same table
- **Verifies**: Self-referential relations are allowed (if supported)
- **Edge Cases**: Self-referential relations
- **Assertions**:
  - HTTP status 201 (if supported) or 400 (if not)
  - Relation created successfully if supported

### 9. **test_create_relation_missing_required_fields**

- **Description**: Attempt to create relation with missing required JSON fields
- **Verifies**: Returns 400 Bad Request with validation errors
- **Edge Cases**: Missing source_table, destination_table, field_mapping
- **Assertions**:
  - HTTP status 400
  - Error message indicates missing required fields
  - No relation created

### 10. **test_create_relation_invalid_json**

- **Description**: Attempt to create relation with malformed JSON
- **Verifies**: Returns 400 Bad Request
- **Edge Cases**: Invalid JSON syntax
- **Assertions**:
  - HTTP status 400
  - Error message indicates JSON parsing error
  - No relation created

### 11. **test_create_relation_concurrent_creation**

- **Description**: Multiple concurrent requests to create same relation
- **Verifies**: Only one relation created, others get appropriate response
- **Edge Cases**: Race condition handling
- **Assertions**:
  - Exactly one relation created
  - Subsequent requests get 409 Conflict or similar

### 12. **test_create_relation_with_custom_types**

- **Description**: Create relation involving custom composite types (e.g., Vec3)
- **Verifies**: Custom type relations work correctly
- **Edge Cases**: Custom type compatibility
- **Assertions**:
  - HTTP status 201
  - Relation created with custom type mapping
  - Type registry handles custom types

### 13. **test_create_relation_referential_integrity_initialized**

- **Description**: Verify referential integrity tracking is set up after relation creation
- **Verifies**: Delete operations on related tables will enforce referential integrity
- **Edge Cases**: Integrity tracking initialization
- **Assertions**:
  - Relation metadata includes integrity constraints
  - Delete attempts on related records trigger appropriate behavior

### 14. **test_create_relation_max_relations_limit**

- **Description**: Attempt to create relation when maximum relations per table reached
- **Verifies**: Returns 400/429 with limit exceeded error
- **Edge Cases**: System limits
- **Assertions**:
  - HTTP status indicates limit exceeded
  - Error message indicates relation limit
  - No relation created

### 15. **test_create_relation_circular_dependency_detection**

- **Description**: Attempt to create relation that would create circular dependency
- **Verifies**: Returns 400 Bad Request with circular dependency error
- **Edge Cases**: Graph cycle detection
- **Assertions**:
  - HTTP status 400
  - Error message indicates circular dependency
  - No relation created

### Edge Cases Summary:

1. Non-existent tables/fields
2. Duplicate relations
3. Type incompatibility
4. Self-referential relations
5. Missing required fields
6. Invalid JSON
7. Concurrent creation race conditions
8. Custom type handling
9. System limits
10. Circular dependencies
11. Field mapping validation (1:1, 1:many, many:many if supported)
12. Null field handling
13. Default value compatibility
14. Schema version compatibility
15. Atomicity of relation creation (all-or-nothing)
