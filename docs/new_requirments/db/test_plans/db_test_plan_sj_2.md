# Test Plan for Task SJ‑2: Deserialize Schema from JSON

**Objective**: Validate that a JSON schema file is correctly parsed into the internal Rust structs (`Table`, `Field`, `Relation`) with proper error handling and edge‑case coverage.

---

## 1. Happy‑path tests

| Test Name                                        | Verifies                                                                                    | Edge Cases Considered                                     |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `test_deserialize_minimal_schema`                | A valid JSON with one table and one field yields correct `Table` and `Field` structs.       | No relations, no custom types.                            |
| `test_deserialize_multiple_tables`               | Multiple tables with distinct names are created in the catalog.                             | Duplicate table names (should error).                     |
| `test_deserialize_fields_with_all_builtin_types` | All built‑in scalar types (`i32`, `u64`, `f32`, `bool`, etc.) are accepted.                 | Unknown type identifier (should error).                   |
| `test_deserialize_custom_composite_type`         | User‑defined composite type (e.g., `3xf32`) is registered and recognized.                   | Malformed composite‑type syntax.                          |
| `test_deserialize_relations`                     | Relations between existing tables are created with correct source/destination mapping.      | Missing source/destination table, duplicate relation IDs. |
| `test_deserialize_full_schema`                   | A complete schema (tables, fields, relations) matches the in‑memory representation exactly. | Order of fields/tables in JSON vs. internal order.        |

---

## 2. Error‑handling & robustness tests

| Test Name                            | Verifies                                                                                    | Edge Cases Considered                          |
| ------------------------------------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `test_malformed_json`                | Invalid JSON syntax produces a clear `serde_json::Error`.                                   | Truncated file, extra commas, unclosed braces. |
| `test_missing_required_fields`       | Missing `name` (table/field) or `type` (field) results in a descriptive error.              | Null values, empty strings.                    |
| `test_duplicate_names`               | Duplicate table names or field names within the same table cause an error.                  | Case‑sensitive comparison.                     |
| `test_invalid_type_identifier`       | Unknown type strings (e.g., `"unknown"`) are rejected with a meaningful error.              | Empty type string, whitespace‑only.            |
| `test_relation_to_nonexistent_table` | Relation referencing a table that does not exist fails validation.                          | Self‑reference (table to itself) allowed?      |
| `test_version_mismatch`              | Schema version field (if present) older/newer than supported version is handled gracefully. | Missing version field (default to latest).     |
| `test_extra_json_fields`             | Extra JSON fields are ignored (or cause a warning) without breaking deserialization.        | Nested extra fields.                           |
| `test_empty_schema`                  | Empty tables array is accepted (no‑op).                                                     | Empty fields array, empty relations array.     |

---

## 3. Integration‑oriented tests

| Test Name                                 | Verifies                                                                                                           | Edge Cases Considered                             |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------- |
| `test_deserialize_after_serialize`        | Round‑trip: serialize a schema to JSON, then deserialize it back; the two in‑memory representations are identical. | Field‑order preservation, custom‑type registry.   |
| `test_schema_with_large_number_of_fields` | A table with many fields (e.g., 100) is deserialized correctly; record‑size calculation matches packing rules.     | Field‑offset recomputation after deserialization. |
| `test_file_not_found`                     | Attempt to read a non‑existent JSON file returns a clear `io::Error`.                                              | Permission‑denied, directory‑instead‑of‑file.     |

---

## 4. Assertions & Expected Behaviors

- **Success cases**: All internal structs (`Table`, `Field`, `Relation`) are populated with the exact values from the JSON (names, types, references).
- **Error cases**: Errors are returned as `Result::Err` with a descriptive message (e.g., `"duplicate table name 'players'"`). No panics.
- **Validation**: After deserialization, the schema passes internal validation (no dangling references, valid offsets, etc.).
- **Idempotency**: Deserializing the same JSON twice produces the same in‑memory structures (no side‑effects).

---

**Note**: These tests assume the JSON format defined in task SJ‑1 (serialization). The exact shape of the JSON will dictate the specific Serde attributes and validation logic.
