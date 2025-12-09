# ECS Database - Test Backlog

## Overview
This document tracks test coverage for the ECS database crate. It lists all modules, existing tests, and missing tests that need to be implemented to ensure robustness and correctness.

## Test Categories
1. **Unit Tests** - Test individual functions and data structures.
2. **Integration Tests** - Test interactions between modules and the public API.
3. **Property-Based Tests** - Use proptest to generate random inputs and verify invariants.
4. **Performance Tests** - Benchmarks using criterion (already in benches/).
5. **Edge Case Tests** - Test error conditions, boundary values, and invalid inputs.

## Legend
- ✅ Test implemented
- ⚠️ Test partially implemented
- ❌ Test missing
- 🚫 Not applicable

## Module Test Coverage

### Core Modules

#### `error.rs`
- No unit tests needed (error enum).
- Integration tests should verify error propagation.

#### `component.rs`
- ❌ Unit tests for `Component` and `ZeroCopyComponent` trait implementations.
- ❌ Test that `static_size` and `alignment` match `std::mem` for zero-copy types.
- ❌ Test that custom component types can be registered.

### Schema System

#### `schema/types.rs`
- ❌ Unit tests for `FieldType::size_bytes()` for each primitive type.
- ❌ Unit tests for `FieldType::alignment()`.
- ❌ Unit tests for `DatabaseSchema::find_table()`.
- ❌ Edge cases: custom/enum/struct types (should return error).

#### `schema/parser.rs`
- ✅ Existing integration test (`test_schema_loading`).
- ❌ Unit tests for `SchemaParser::from_string` with valid TOML.
- ❌ Unit tests for parsing each field attribute (nullable, indexed, primary_key, foreign_key).
- ❌ Unit tests for parsing custom types and enums.
- ❌ Error handling: missing sections, invalid types, malformed arrays.
- ❌ Property-based: round-trip serialize/deserialize.

#### `schema/validator.rs`
- ❌ Unit tests for `SchemaValidator::validate` with valid schema.
- ❌ Unit tests for each validation function:
  - `check_foreign_keys` valid and invalid references.
  - `check_field_alignment` with zero-length array.
  - `check_reserved_names` for tables and fields.
  - `check_table_names_unique` duplicate detection.
  - `check_field_names_unique` duplicate detection.
- ❌ Error messages should be descriptive.

#### `schema/migrations.rs`
- 🚫 Currently placeholder; no tests needed until implementation.

### Storage Layer

#### `storage/buffer.rs`
- ❌ Unit tests for `StorageBuffer` (legacy) insert, update, read, commit.
- ❌ Unit tests for `ArcStorageBuffer`:
  - `insert` with free list reuse.
  - `free_slot` and fragmentation ratio.
  - `commit` and `commit_with_generation`.
  - `compact` mapping correctness.
  - `snapshot_state` / `restore_state` round-trip.
  - `load_snapshot` validation.
- ❌ Concurrency: atomic buffer swap (hard to test without threads).
- ❌ Edge cases: record size mismatch, offset out of bounds.

#### `storage/field_codec.rs`
- ✅ Existing tests for `encode/decode` and `cast_bytes_to_ref`.
- ❌ Additional tests for:
  - Zero-copy casting with alignment constraints.
  - Error handling for malformed bytes.
  - Round-trip property: encode then decode equals original.
  - Property-based: random structs serialization.

#### `storage/layout.rs`
- ✅ Existing tests for primitive and array layout.
- ❌ Additional tests for:
  - Nested arrays and custom types (when implemented).
  - Alignment padding between fields.
  - Total size matches sum of aligned fields.
  - Edge cases: empty field list, large alignments.

#### `storage/sparse.rs`
- ✅ Existing tests for basic operations and iteration.
- ❌ Additional tests for:
  - Large entity IDs (sparse set growth).
  - Concurrent modifications (if any).
  - Memory usage (should be O(entity_count)).

#### `storage/table.rs`
- ✅ Existing tests for insert/get/update/delete, commit, compaction.
- ❌ Additional tests for:
  - `contains_entity` and `entity_mapping`.
  - `snapshot_write_state` / `restore_write_state` round-trip.
  - `load_snapshot` with invalid data.
  - Fragmentation detection (`is_fragmented`).
  - Foreign key validation (via `validate_foreign_keys`).
  - Table handle implementation (type-erased wrapper).

#### `storage/delta.rs`
- ❌ Unit tests for `DeltaOp` serialization/deserialization.
- ❌ Unit tests for `DeltaTracker`:
  - `record_insert`, `record_update`, `record_delete`.
  - `store_before_image` / `get_before_image`.
  - `take_delta` resets tracker.
- ❌ Property-based: delta serialization round-trip.
- ❌ Integration with transaction engine.

### Entity System

#### `entity/registry.rs`
- ✅ Existing integration test (`test_entity_registry`).
- ❌ Unit tests for:
  - `create_entity` ID generation and reuse.
  - `delete_entity` version increment and freelist.
  - `contains_entity` after delete.
  - `records` snapshotting.
- ❌ Edge cases: delete non-existent entity, duplicate delete.

#### `entity/archetype.rs`
- ✅ Existing tests for mask and registry operations.
- ❌ Additional tests for:
  - Large number of components (mask bit operations).
  - Archetype size tracking.
  - Iteration over entities in archetype (if added).

### Transaction System

#### `transaction/engine.rs`
- ✅ Existing test for `process_transaction`.
- ❌ Additional tests for:
  - Transaction rollback on error.
  - Version increment consistency.
  - Concurrent transactions (if supported).
  - WAL integration (already covered in wal tests).

#### `transaction/wal.rs`
- ✅ Existing tests for checksum and logger.
- ❌ Additional tests for:
  - `WalLogger` with multiple concurrent transactions.
  - `entries_for_transaction` with rollback entries.
  - Checksum tampering detection.
  - Serialization round-trip.

#### `transaction/write_queue.rs`
- ❌ Unit tests for `WriteQueue` (challenging due to threading).
  - Mock the process closures to verify operations.
  - Test single operations (insert, update, delete) success and error.
  - Test batch commit atomicity (all-or-nothing).
  - Test timeout handling.
  - Test shutdown clean-up.
- ❌ Integration with database (covered in db tests).

### Persistence Layer

#### `persistence/file_wal.rs`
- ✅ Existing tests for header, basic ops, rotation, replay.
- ❌ Additional tests for:
  - Corruption recovery (partial writes).
  - Concurrent writes (if supported).
  - Large WAL files exceeding size limits.
  - Async vs sync writes.

#### `persistence/snapshot.rs`
- ❌ Unit tests for `SnapshotHeader` validation.
- ❌ Unit tests for `DatabaseSnapshot`:
  - `write_to_file` / `from_file` round-trip uncompressed.
  - `write_to_file_async` / `from_file_async` round-trip.
  - Compression flag and checksum verification.
  - Checksum mismatch detection.
  - Invalid magic/version errors.
- ❌ Integration with `Database::create_snapshot` and `Database::from_snapshot`.

#### `persistence/mod.rs`
- ❌ Integration tests for combined snapshot + WAL recovery.

### Database API (`db.rs`)
- ✅ Existing test `test_database_basic`.
- ❌ Additional unit tests for:
  - `register_component` duplicate registration.
  - `create_entity` and `delete_entity` with referential integrity.
  - `insert`, `update`, `delete`, `get` with multiple component types.
  - `commit` version increment and delta generation.
  - `compact_if_fragmented` threshold behavior.
  - `create_snapshot` / `from_snapshot` round-trip.
  - Foreign key validation errors.
  - Concurrent reads while writing (should not block).
- ❌ Integration tests using the public API with a real schema file.

### Integration Tests (`tests/integration.rs`)
- ✅ Existing tests for schema loading and entity registry.
- ❌ Additional integration tests:
  - Full CRUD cycle with multiple component types.
  - Transaction commit and rollback.
  - Snapshot save/load.
  - WAL replay after crash simulation.
  - Concurrency: multiple readers, single writer.
  - Foreign key constraints.
  - Schema evolution (when implemented).

## Property-Based Tests (proptest)

- ❌ Schema parsing: for any valid TOML string, parse and validate.
- ❌ Component serialization: for any struct implementing `Component`, encode/decode round-trip.
- ❌ Buffer operations: random sequence of insert/update/delete, verify invariants.
- ❌ Entity registry: random create/delete, verify ID uniqueness and freelist.
- ❌ Delta generation: random transactions, compute delta, apply to replica, verify equality.

## Performance Benchmarks (criterion)

- ✅ Existing benches for inserts, reads, transactions.
- ❌ Additional benchmarks:
  - Concurrent read scaling (multiple threads reading same data).
  - Commit latency with varying batch sizes.
  - Snapshot serialization/deserialization speed.
  - Delta serialization size.
  - Fragmentation impact on compaction.

## Edge Cases & Error Handling

- ❌ Invalid inputs: wrong record sizes, out-of-bounds offsets, malformed schemas.
- ❌ Resource exhaustion: memory allocation failures (use fallible allocator).
- ❌ File I/O errors: disk full, permission denied, corrupted files.
- ❌ Concurrency: panic in write thread, channel disconnection.

## Test Infrastructure

- ❌ Set up CI to run tests with `cargo test --workspace`.
- ❌ Set up MIRI for unsafe code validation (where applicable).
- ❌ Set up sanitizers (address, leak, thread) for concurrency bugs.
- ❌ Code coverage tracking (e.g., tarpaulin, grcov).

## Priority

1. **High**: Unit tests for modules with zero coverage (buffer, delta, validator, write_queue).
2. **Medium**: Integration tests for core database operations.
3. **Low**: Property-based and performance tests.

## Progress Tracking

- Last Updated: 2025-12-09
- Total Modules: 20
- Modules with tests: 9 (45%)
- Modules without tests: 11 (55%)

See `IMPLEMENTED.md` for detailed progress on implementation phases.

## Notes
- When adding tests, follow existing patterns (use `#[cfg(test)]` mod inside each source file).
- Use `tempfile` crate for tests that need temporary files.
- Use `tokio::test` for async tests.
- Use `proptest` for property-based tests (already in dev-dependencies).