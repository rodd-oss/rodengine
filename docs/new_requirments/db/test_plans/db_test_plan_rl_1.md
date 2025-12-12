# Test Plan for Task RL-1: Define Relation Struct

## Overview

Unit tests for the `Relation` struct that references source/destination tables and field mapping. Part of a relational in-memory database for online games (Rust implementation).

## Test Cases

### 1. Basic Struct Construction

**Test**: `relation_new_valid`

- **Description**: Create a Relation with valid source/destination table IDs and field mapping.
- **Verifies**: Struct fields are stored correctly, mapping is preserved.
- **Assertions**: `relation.source_table == expected_source`, `relation.destination_table == expected_dest`, `relation.field_mapping == mapping`.
- **Edge Cases**: None.

### 2. Self-Referential Relation

**Test**: `relation_self_referential`

- **Description**: Source and destination tables are the same.
- **Verifies**: Self-references are allowed; field mapping can map within same table.
- **Edge Cases**: Ensure no infinite loops during traversal (deferred to integrity checks).
- **Assertions**: `relation.source_table == relation.destination_table`.

### 3. Invalid Table References

**Test**: `relation_invalid_table_id_zero`

- **Description**: Table IDs must be non-zero (if using ID 0 as sentinel).
- **Verifies**: Constructor panics or returns `Result::Err` for zero IDs.
- **Edge Cases**: Negative IDs if using signed integers (should be unsigned).
- **Assertions**: `Relation::new(0, 1, mapping)` returns `Err(InvalidTableId)`.

### 4. Invalid Field Mapping

**Test**: `relation_empty_mapping`

- **Description**: Field mapping cannot be empty (must link at least one field pair).
- **Verifies**: Constructor rejects empty `Vec`.
- **Assertions**: `Relation::new(src, dest, vec![])` returns `Err(EmptyMapping)`.

### 5. Duplicate Source Fields

**Test**: `relation_duplicate_source_fields`

- **Description**: Source field appears multiple times in mapping.
- **Verifies**: Mapping must be a bijection? Possibly reject duplicates.
- **Edge Cases**: Duplicate destination fields also may be invalid.
- **Assertions**: `Relation::new(src, dest, [(1,2), (1,3)])` returns `Err(DuplicateSourceField)`.

### 6. Type Mismatch Validation (Future)

**Test**: `relation_field_type_mismatch`

- **Description**: Source and destination field types must match (enforced by schema).
- **Verifies**: Constructor accepts only when types match (requires schema context).
- **Edge Cases**: Custom composite types (e.g., `Vec3`) must match exactly.
- **Assertions**: With schema, `Relation::new_with_schema(...)` returns `Err(TypeMismatch)`.

### 7. Serialization/Deserialization

**Test**: `relation_serde_roundtrip`

- **Description**: Serialize Relation to JSON and back.
- **Verifies**: `serde` support works; all fields preserved.
- **Edge Cases**: Handle optional fields, enums for relation type.
- **Assertions**: `let rt = serde_json::from_str(&serde_json::to_string(&rel).unwrap()).unwrap(); assert_eq!(rt, rel);`

### 8. Clone and Debug

**Test**: `relation_implements_clone_debug`

- **Description**: Relation should derive `Clone`, `Debug`, maybe `PartialEq`.
- **Verifies**: Standard traits for usability.
- **Assertions**: `relation.clone() == relation`, `format!("{:?}", relation)` contains struct name.

### 9. Immutability

**Test**: `relation_fields_immutable`

- **Description**: Once created, fields cannot be changed (unless `&mut` access provided).
- **Verifies**: Struct uses private fields with getters, no public mutability.
- **Assertions**: `relation.source_table()` returns `&TableId`, cannot modify.

### 10. Multiple Relations Between Same Tables

**Test**: `relation_multiple_between_tables`

- **Description**: Allow multiple distinct field mappings between same source/destination.
- **Verifies**: Constructor accepts duplicate (src, dest) pairs with different mappings.
- **Edge Cases**: Need unique relation ID to differentiate.
- **Assertions**: Two relations with same tables but different mappings are distinct.

## Edge Cases Summary

- Self-referential relations.
- Circular reference chains (detection deferred to integrity pass).
- Non-existent tables (validation requires catalog; maybe deferred).
- Field mapping across tables with different record counts (cardinality).
- Zero‑length field names? (Field IDs used instead.)
- Mapping where source/destination fields have different sizes (alignment issues).
- Concurrent modification of underlying tables (later concurrency tests).

## Expected Behaviors

- Relation struct should be lightweight (no heap aside from mapping Vec).
- IDs should be `u32` or `usize` for performance.
- Mapping stored as `Vec<(FieldId, FieldId)>` for cache locality.
- All validation that depends on schema should be deferred to a separate `validate` method that takes schema reference.
