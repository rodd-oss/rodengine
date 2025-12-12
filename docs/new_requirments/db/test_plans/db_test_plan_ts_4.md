# Test Plan for Task TS-4: Field Addition/Removal with Schema Validation

## Context

- In-memory relational database with `Vec<u8>` storage.
- Table schema includes fields with name, type identifier, byte offset.
- Field addition/removal must validate: no duplicate names, valid type.
- This task focuses on schema validation, not data migration (adding/removing fields when table has records may be handled later).

## Assumptions

- Table struct has methods:
  - `add_field(name: &str, type_id: FieldType) -> Result<(), ValidationError>`
  - `remove_field(name: &str) -> Result<(), ValidationError>`
- Validation errors include `DuplicateFieldName`, `InvalidFieldType`, `FieldNotFound`, `TableNotEmpty` (if removal with data).
- Supported field types are defined in a type registry (built‑in scalars and custom composites).
- Adding a field updates the schema but does not modify existing records (future migration task).
- Removing a field is only allowed if the table has zero records (or field is unused). We'll assume strict validation.

## Test Categories

### 1. Field Addition – Success Cases

**test_add_field_to_empty_table**  
Verifies that a field can be added to a table with no existing fields.

- Setup: Empty table (no fields).
- Action: Add field with unique name and valid type (e.g., `i32`).
- Assert: `Ok(())` returned; field count == 1; field can be retrieved with correct name/type.

**test_add_field_to_existing_table**  
Verifies that a field can be added when other fields already exist.

- Setup: Table with one or more fields.
- Action: Add new field with unique name and valid type.
- Assert: `Ok(())`; field count increments; new field appears at end of field list (or appropriate position).

**test_add_field_multiple_types**  
Verifies that all supported field types can be added.

- Setup: Empty table.
- Action: For each built‑in type (`i32`, `u64`, `f32`, `bool`, etc.) and a sample custom type (`Vec3`), add a field.
- Assert: All additions succeed; field types match expected.

### 2. Field Addition – Validation Failures

**test_add_field_duplicate_name**  
Verifies that adding a field with a name that already exists fails.

- Setup: Table with field `"score"` (type `i32`).
- Action: Attempt to add another field named `"score"` (any type).
- Assert: `Err(DuplicateFieldName)`.

**test_add_field_invalid_type**  
Verifies that adding a field with an unrecognized type fails.

- Setup: Empty table.
- Action: Add field with type identifier `"UnknownType"` (or integer out of range).
- Assert: `Err(InvalidFieldType)`.

**test_add_field_empty_name**  
Edge case: field name is empty string (should be invalid).

- Action: Add field with name `""`.
- Assert: `Err(InvalidFieldName)` or `DuplicateFieldName` if empty is considered duplicate? We'll reject empty names.

**test_add_field_name_too_long**  
Optional: enforce maximum name length.

- Action: Add field with name longer than allowed limit (if any).
- Assert: `Err(InvalidFieldName)`.

### 3. Field Removal – Success Cases

**test_remove_existing_field**  
Verifies that an existing field can be removed from an empty table.

- Setup: Table with field `"score"` (type `i32`), zero records.
- Action: Remove field `"score"`.
- Assert: `Ok(())`; field count == 0; retrieving field `"score"` returns `None`.

**test_remove_field_from_multi_field_table**  
Verifies removal when multiple fields exist, ensuring other fields remain.

- Setup: Table with fields `"id"`, `"score"`, `"active"`.
- Action: Remove field `"score"`.
- Assert: `Ok(())`; field count == 2; remaining fields are `"id"` and `"active"` with correct offsets.

### 4. Field Removal – Validation Failures

**test_remove_nonexistent_field**  
Verifies that removing a field that does not exist fails.

- Setup: Table with fields `"id"`, `"score"`.
- Action: Remove field `"name"`.
- Assert: `Err(FieldNotFound)`.

**test_remove_field_table_not_empty**  
Verifies that removing a field when the table has at least one record fails (if we enforce this).

- Setup: Table with field `"score"`; insert one record.
- Action: Attempt to remove field `"score"`.
- Assert: `Err(TableNotEmpty)` or `Err(FieldInUse)`.

**test_remove_last_field**  
Edge case: removing the last field from a table (should succeed if table empty).

- Setup: Table with single field `"id"`, zero records.
- Action: Remove field `"id"`.
- Assert: `Ok(())`; field count == 0.

### 5. Schema Integrity After Operations

**test_field_order_preserved_after_removal**  
Verifies that field indices are stable after removal (remaining fields keep original order).

- Setup: Table with fields `"a"`, `"b"`, `"c"`.
- Action: Remove field `"b"`.
- Assert: Remaining fields are `["a", "c"]` in that order; offsets adjusted accordingly.

**test_add_remove_add_same_name**  
Verifies that a field can be added again after being removed (if table empty).

- Setup: Empty table.
- Action: Add field `"temp"`, remove `"temp"`, add `"temp"` again with different type.
- Assert: Both additions succeed; final type matches second addition.

**test_duplicate_check_ignores_removed_field**  
Ensures duplicate name validation does not consider already‑removed fields.

- Setup: Table with field `"old"`, remove `"old"`.
- Action: Add new field named `"old"`.
- Assert: `Ok(())` (no duplicate error).

### 6. Concurrency & Atomicity (if applicable)

**test_concurrent_add_field**  
If schema modifications are protected by a lock or atomic swap, verify that simultaneous additions are serialized correctly.

- Use threads to attempt adding two fields at same time.
- Assert: Both succeed, no duplicates, field count increases by 2.

**test_add_remove_interleaved**  
Interleave addition and removal operations from different threads (may be out of scope for current task).

- Expect: No data races, schema remains consistent.

### 7. Integration with Type Registry

**test_custom_type_validation**  
Verifies that a custom composite type (e.g., `Vec3`) is recognized as valid.

- Setup: Register custom type `Vec3` (3×`f32`).
- Action: Add field with type `Vec3`.
- Assert: `Ok(())`.

**test_custom_type_not_registered**  
Attempt to add field with custom type that hasn't been registered.

- Action: Add field with type `"UnregisteredComposite"`.
- Assert: `Err(InvalidFieldType)`.

### 8. Error Messages & Types

**test_error_variants_contain_context**  
Validation errors should include details (field name, type, etc.).

- Assert: `DuplicateFieldError { name: "score" }`, `InvalidTypeError { type_id: ... }`.

## Edge Cases to Consider

1. **Duplicate names with different casing** – Should "Score" and "score" be considered duplicates? Likely case‑sensitive; test both.
2. **Field name with whitespace** – Reject or trim? Probably reject.
3. **Adding field with same type as existing but different name** – Allowed.
4. **Removing field that is part of a relation** – Not yet implemented; ignore.
5. **Adding field when table is at maximum field limit** (if any) – Should fail with appropriate error.
6. **Type identifier is zero‑sized** (e.g., `()`). Not supported; invalid.
7. **Adding field after table has been persisted** – Schema persistence not required for this task.
8. **Concurrent reads while modifying schema** – Ensure readers see consistent schema snapshot (ArcSwap).

## Assertions & Expected Behaviors

- All validation failures must occur before any mutation of the schema.
- Successful addition/removal must update the table's field list and any derived metadata (record size, offsets).
- After removal, any attempt to access the removed field (by name or index) should return `None` or panic.
- Error types should be exhaustive and implement `std::error::Error`.

## Dependencies

- Table struct with field vector (task TS‑1).
- Field struct with name, type, offset (task TS‑2).
- Type registry (task CT‑1, CT‑2).
- Possibly record storage (tasks SL‑1 through SL‑5) for table‑not‑empty check.

## Notes

- This test plan assumes validation is performed synchronously; asynchronous validation (e.g., via REST API) will be covered in API tasks.
- Performance of validation (e.g., duplicate name check) should be O(1) or O(n) but not O(n²).
- Consider using `HashMap` for field name lookup for duplicate detection.

## Test Coverage Goals

- All validation error paths.
- Success paths for addition and removal.
- Edge cases (empty names, duplicate after removal, etc.).
- Schema consistency after operations.
- Integration with type system.
