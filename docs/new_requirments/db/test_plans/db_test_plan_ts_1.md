# Test Plan for Task TS-1: Define Table Struct with Name and Vector of Field Definitions

## Context

- Relational in‑memory database for online games (Rust).
- Table struct holds `name: String` and `fields: Vec<Field>`.
- Field struct (task_ts_2) assumed to have at least `name: String` and `type_id`.
- No storage or serialization required yet.

## TRD Alignment Notes

- **Rust Implementation**: TRD §2 mandates Rust for performance and safety
- **Schema Definition**: Table struct is foundation for JSON schema persistence (§5)
- **Field Definitions**: Must support TRD §3 type system (integers, floats, strings, booleans, blobs)
- **Catalog Integration**: Tables stored in catalog for REST API endpoint `/tables` (§5)
- **Validation**: Field name uniqueness aligns with relational model constraints (§3)

## Test Categories

### 1. Basic Construction & Getters

**test_table_new_with_name_and_fields**  
Verifies that a Table can be created with a non‑empty name and a vector of fields.

- Setup: Define a few dummy Field instances.
- Action: `Table::new("players", fields)`.
- Assert: `table.name() == "players"`, `table.fields().len() == fields.len()`.

**test_table_new_with_empty_fields**  
Edge case: Table with zero fields.

- Setup: Empty Vec.
- Action: `Table::new("empty", vec![])`.
- Assert: `table.fields().is_empty()`.

**test_table_implements_debug_clone_partialeq**  
Ensures Table derives common traits for debugging and comparison.

- Assert: `#[derive(Debug, Clone, PartialEq)]` is present.

### 2. Name Validation & Edge Cases

**test_table_name_non_empty**  
If spec requires non‑empty names, constructor should reject empty strings.

- Action: `Table::new("", vec![])`.
- Expect: `Result<Table, ValidationError>` or panic.

**test_table_name_whitespace_only**  
Check trimming or validation of whitespace‑only names.

- Action: `Table::new("  ", vec![])`.
- Expect: Rejection or trimmed name (depending on spec).

**test_table_name_valid_characters**  
Optional: Ensure name contains only allowed characters (e.g., alphanumeric + underscore).

- Expect: Constructor accepts valid identifiers, rejects invalid ones.

### 3. Field Vector Validation

**test_fields_unique_names**  
Prevent duplicate field names within the same table.

- Setup: Two Field instances with same name but possibly different types.
- Action: `Table::new("t", vec![field1, field2])`.
- Expect: `Result::Err` with duplicate‑name error.

**test_fields_empty_name**  
If Field spec forbids empty names, Table constructor should reject fields with empty names.

- Setup: Field with `name: ""`.
- Expect: Validation error.

**test_fields_ordering_preserved**  
Field vector order should be preserved (determines column order).

- Setup: Fields in specific order.
- Assert: `table.fields()[i].name == expected_names[i]`.

### 4. Mutation (if methods exist)

**test_add_field**  
If Table provides `add_field(&mut self, field: Field)` method.

- Setup: Empty table.
- Action: Add a field.
- Assert: `table.fields().len() == 1`, field appears at end.

**test_add_field_duplicate_name_rejected**  
Adding a field whose name already exists should fail.

- Setup: Table with one field named "id".
- Action: Add another field named "id".
- Expect: `Err(DuplicateField)`.

**test_remove_field_by_name**  
If removal is in scope, test that field is removed and vector updated.

- Setup: Table with fields ["id", "score"].
- Action: Remove "id".
- Assert: Remaining field list == ["score"].

### 5. Edge Cases & Stress

**test_large_number_of_fields**  
Table with many fields (e.g., 10_000) should not panic on construction.

- Expect: Constructor succeeds, `table.fields().len()` matches.

**test_field_vector_capacity**  
If fields are provided via Vec, ensure capacity is not unnecessarily large.

- Assert: `table.fields().capacity() == table.fields().len()` (or within reasonable bounds).

**test_table_with_max_field_name_length**  
Field names at maximum allowed length (e.g., 255 bytes).

- Expect: Construction succeeds.

### 6. Integration with Future Tasks

**test_table_works_with_field_struct**  
Ensure Table can be constructed with actual Field struct from task_ts_2.

- Setup: Real Field instances (name, type_id, offset).
- Action: Create Table.
- Assert: No compile errors, fields accessible.

**test_table_can_be_used_in_catalog**  
Verify Table can be stored in a `HashMap<String, Table>` (simulating catalog).

- Action: Insert table into map, retrieve by name.
- Assert: Retrieved table equals original.

## Implementation Notes

- Use `cargo test` with `--test-threads=1` for panic tests.
- If validation returns `Result`, test both `Ok` and `Err` variants.
- Mock Field struct if not yet implemented (simple `struct Field { name: String }`).
- Follow project’s existing test style (see `test_plan_ms_2.md`).
