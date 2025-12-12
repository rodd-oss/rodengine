# Test Plan for task_ts_3: Table Creation/Destruction

## Overview

Unit tests for implementing table creation/destruction (add/remove from database catalog) in the relational in-memory database.

## Core Unit Tests

### 1. `create_table_success`

**Description**: Add a table with a unique name and valid field list to the catalog.
**Verification**: Catalog contains the table; returned `TableId` is valid.
**Edge Cases**: Empty field list allowed (table can have zero fields).
**Assertions**:

- `catalog.get_table(name).is_some()`
- `catalog.list_tables()` contains the new table name

### 2. `create_table_duplicate_name`

**Description**: Attempt to create a table whose name already exists in the catalog.
**Verification**: Returns `CatalogError::DuplicateTable`; catalog unchanged.
**Edge Cases**: Case-sensitivity (assume case-sensitive).
**Assertions**:

- `catalog.create_table(existing_name, fields)` returns `Err(CatalogError::DuplicateTable)`
- `catalog.get_table(existing_name)` still returns the original table

### 3. `delete_table_success`

**Description**: Remove an existing table from the catalog.
**Verification**: Catalog no longer contains the table; returns `Ok(())`.
**Edge Cases**: Deleting a table that still has records (storage buffer should be dropped).
**Assertions**:

- `catalog.delete_table(name)` returns `Ok(())`
- `catalog.get_table(name).is_none()`
- Storage buffer's `Arc` strong-count drops to zero (memory leak check)

### 4. `delete_nonexistent_table`

**Description**: Attempt to delete a table that does not exist.
**Verification**: Returns `CatalogError::TableNotFound`; catalog unchanged.
**Edge Cases**: Empty string as table name.
**Assertions**:

- `catalog.delete_table("nonexistent")` returns `Err(CatalogError::TableNotFound)`
- Catalog size unchanged

### 5. `list_tables_after_creations`

**Description**: Create multiple tables, then retrieve the list of all table names.
**Verification**: List contains exactly the created names, order undefined.
**Edge Cases**: Catalog empty initially.
**Assertions**:

- `catalog.list_tables().len()` equals number of created tables
- All created table names appear in the list

### 6. `table_creation_with_fields`

**Description**: Create a table with a non-empty field list (e.g., `[("id", i32), ("name", &str)]`).
**Verification**: Stored `Table` struct contains the correct field definitions (names, types, offsets).
**Edge Cases**: Field list with duplicate names (should be rejected at field-addition stage, not here).
**Assertions**:

- `table.fields().len()` equals input field count
- Field names and types match input

### 7. `table_name_validation`

**Description**: Attempt to create tables with invalid names (empty string, characters not allowed).
**Verification**: Returns `CatalogError::InvalidName`.
**Edge Cases**: Decide allowed charset (e.g., alphanumeric + underscores).
**Assertions**:

- `catalog.create_table("", fields)` returns `Err(CatalogError::InvalidName)`
- `catalog.create_table("table-name", fields)` returns `Err(CatalogError::InvalidName)` (if hyphens disallowed)

### 8. `catalog_persistence_not_implemented`

**Description**: Verify that the catalog does **not** yet persist across restarts (no JSON serialization).
**Verification**: After dropping the catalog instance, tables are gone.
**Edge Cases**: This test is a placeholder for future schema-persistence tasks.
**Assertions**:

- New catalog instance has empty table list

## Edge Cases & Additional Considerations

1. **Duplicate table names**: Case-sensitive matching; error must not corrupt catalog state.
2. **Deleting a table that is referenced by a relation**: Not required yet (relations not implemented), but can be noted as future referential-integrity test.
3. **Memory safety**: Deleting a table must drop its storage buffer (`Vec<u8>`) to prevent leaks.
4. **Concurrent access**: Not required for this task; will be added later with `ArcSwap`/atomic operations.
5. **Catalog capacity**: No explicit limit; ensure it can hold many tables without performance degradation.

## Expected Behaviors & Assertions

### Catalog API

- `Catalog::create_table(name, fields) -> Result<TableId, CatalogError>`
  - Success: `catalog.get_table(name).is_some()`
  - Duplicate: `Err(CatalogError::DuplicateTable)`
  - Invalid name: `Err(CatalogError::InvalidName)`
- `Catalog::delete_table(name) -> Result<(), CatalogError>`
  - Success: `catalog.get_table(name).is_none()`
  - Not found: `Err(CatalogError::TableNotFound)`
- `Catalog::list_tables() -> Vec<String>` returns all table names.

## Integration Notes

- These tests assume a single-threaded catalog; concurrency tests belong to later phases.
- Storage-buffer cleanup on deletion can be verified by checking that the buffer's `Arc` strong-count drops to zero.
- Field-addition and relation tests are separate tasks; table-creation tests should not depend on them.
- No JSON serialization/persistence required for this task (task_sj_1/sj_2 handle that).
