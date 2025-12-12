# Test Plan for `task_sj_1`: Serialize entire schema (tables, fields, relations) to JSON file

## 1. Test Names & Descriptions

| Test Name                                         | Brief Description                                                                                        |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `test_serialize_empty_schema`                     | Serialize a schema with no tables, fields, or relations.                                                 |
| `test_serialize_single_table_no_fields`           | Serialize a schema with one table but zero fields.                                                       |
| `test_serialize_table_with_builtin_field_types`   | Serialize a table containing fields of all built‑in scalar types (i32, u64, f32, bool, etc.).            |
| `test_serialize_table_with_custom_composite_type` | Serialize a table with a user‑defined composite type (e.g., `Vec3` as `3×f32`).                          |
| `test_serialize_multiple_tables`                  | Serialize a schema with several unrelated tables.                                                        |
| `test_serialize_relation_one_to_one`              | Serialize a schema containing a one‑to‑one relation between two tables.                                  |
| `test_serialize_relation_one_to_many`             | Serialize a schema containing a one‑to‑many relation.                                                    |
| `test_serialize_self_referential_relation`        | Serialize a relation where a table references itself.                                                    |
| `test_serialize_complete_schema_roundtrip`        | Full integration test: create a complex schema, serialize to JSON, deserialize, and verify equality.     |
| `test_serialize_file_io_success`                  | Integration test writing JSON to a temporary file and verifying file content matches expected structure. |
| `test_serialize_file_io_error_handling`           | Verify appropriate error is returned when writing to a read‑only or invalid path (mock file system).     |
| `test_serialize_concurrent_access`                | Ensure serialization does not break when other threads are reading/writing the schema concurrently.      |

---

## 2. What Each Test Verifies

- **`test_serialize_empty_schema`**: Verifies that an empty schema produces valid JSON (empty arrays for tables/relations) and does not panic.
- **`test_serialize_single_table_no_fields`**: Verifies that a table with zero fields is serialized correctly (fields array empty).
- **`test_serialize_table_with_builtin_field_types`**: Ensures each built‑in type’s metadata (name, size, alignment) is preserved in the JSON output.
- **`test_serialize_table_with_custom_composite_type`**: Checks that custom composite types are serialized with their internal layout (e.g., component count, component type).
- **`test_serialize_multiple_tables`**: Confirms that the order and contents of multiple independent tables are preserved.
- **`test_serialize_relation_one_to_one`**: Validates that relation metadata (source/destination table names, field mapping) is correctly serialized.
- **`test_serialize_relation_one_to_many`**: Same as above, with a one‑to‑many cardinality.
- **`test_serialize_self_referential_relation`**: Ensures a relation that points to the same table is handled correctly (no infinite recursion).
- **`test_serialize_complete_schema_roundtrip`**: End‑to‑end validation that serialization + deserialization yields an identical in‑memory schema (using `Eq`/`PartialEq` on schema structs).
- **`test_serialize_file_io_success`**: Verifies that the JSON string written to a file matches the in‑memory representation (using `serde_json::to_string_pretty`).
- **`test_serialize_file_io_error_handling`**: Confirms that file‑write errors (e.g., permission denied) are propagated as `Result::Err` and do not corrupt the schema.
- **`test_serialize_concurrent_access`**: Ensures serialization can run while other threads hold `ArcSwap` references to the schema (no deadlocks, data races).

---

## 3. Edge Cases to Consider

| Edge Case                                         | Test Approach                                                                                                                               |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| **Empty schema**                                  | Serialize a schema with no tables, fields, or relations.                                                                                    |
| **Table with zero fields**                        | Ensure the fields array is empty, not omitted.                                                                                              |
| **Custom composite types**                        | Verify nested/composite types (e.g., `Vec3`, `ColorRGBA`) are serialized with their internal layout.                                        |
| **Circular references between tables**            | Check that serialization does not enter infinite recursion; may require special handling (e.g., store table names instead of full objects). |
| **Large number of tables/fields/relations**       | Stress test with hundreds of elements to ensure performance and correct ordering.                                                           |
| **Concurrent modifications during serialization** | Use `ArcSwap` to snapshot the schema atomically; verify snapshot consistency.                                                               |
| **File I/O failures**                             | Mock the filesystem to simulate disk‑full, read‑only, or invalid‑path errors.                                                               |
| **UTF‑8 names**                                   | Tables, fields, and relations with Unicode names should be serialized/escaped correctly.                                                    |
| **Schema with invalid internal state**            | Ensure serialization validates the schema (e.g., field offsets within record size) before writing.                                          |
| **Backwards compatibility**                       | Future‑proof: serialized JSON should include a version field.                                                                               |

---

## 4. Assertions & Expected Behaviors

| Assertion               | Expected Behavior                                                                                                              |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **JSON structure**      | Output must be valid JSON conforming to a predefined schema (e.g., `{"version": "1.0", "tables": [...], "relations": [...]}`). |
| **Round‑trip equality** | `deserialize(serialize(schema)) == schema` (requires `Eq` on schema structs).                                                  |
| **Idempotency**         | Repeated serialization of the same schema produces identical JSON (field order stable).                                        |
| **Atomic snapshot**     | Serialization must capture a consistent snapshot of the schema even if concurrent modifications occur.                         |
| **Error propagation**   | File‑write errors must be returned as `Result::Err`; in‑memory schema remains unchanged.                                       |
| **No panics**           | Serialization never panics, even on empty or malformed internal state (validation should happen earlier).                      |
| **Memory safety**       | Serialization must not trigger undefined behavior (no unsafe code in the serialization path).                                  |
| **Performance**         | Serialization of a typical schema (dozens of tables) completes within a few milliseconds (benchmark).                          |

**Implementation note**: Use `serde` with custom serializers for `Field`, `Table`, `Relation`. Unit tests should mock the file system (e.g., with `tempfile` crate) to avoid side effects. Integration tests can write to a temporary directory and verify file contents.
