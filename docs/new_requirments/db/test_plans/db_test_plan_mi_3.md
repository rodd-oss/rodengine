# Unit Test Plan for task_mi_3

## 1. Storage Layout Module Tests

**test_storage_buffer_initialization**

- Verifies: `Vec<u8>` buffer initializes with correct capacity
- Edge cases: Zero capacity, large capacity, capacity overflow
- Assertions: Buffer length matches capacity, memory allocation succeeds

**test_field_type_sizes_alignments**

- Verifies: Built-in types (i32, u64, f32, bool) have correct byte sizes
- Edge cases: Custom composite types (Vec3 as 3×f32), alignment requirements
- Assertions: Size calculations match Rust's `std::mem::size_of`

**test_record_size_calculation**

- Verifies: Record size calculated correctly with tight packing (no padding)
- Edge cases: Mixed field types, empty record, maximum field count
- Assertions: Total size = sum of field sizes, alignment = 1

**test_record_write_read_roundtrip**

- Verifies: Data integrity after unsafe pointer casting
- Edge cases: Cross-platform endianness, unaligned access
- Assertions: Written value equals read value for all field types

**test_buffer_bounds_validation**

- Verifies: Out-of-bounds access prevented
- Edge cases: Negative indices, indices beyond buffer length
- Assertions: Panics or returns error on invalid access

## 2. Zero-Copy Module Tests

**test_field_accessor_references**

- Verifies: Field accessors return `&T` references not copies
- Edge cases: Lifetime validation, mutable vs immutable references
- Assertions: Reference points to buffer memory, no allocation

**test_record_iterator_references**

- Verifies: Iterator yields references to records
- Edge cases: Empty table, concurrent modification detection
- Assertions: Iterator items are references, no data copying

## 3. Memory Safety Module Tests

**test_field_offset_validation**

- Verifies: Field offsets stay within record bounds
- Edge cases: Field removal causing offset invalidation
- Assertions: Offset + size ≤ record size

**test_record_index_bounds_checking**

- Verifies: Record indices validated against table size
- Edge cases: Index arithmetic overflow, negative indices
- Assertions: Valid indices succeed, invalid indices fail

## 4. Table Schema Module Tests

**test_table_creation_deletion**

- Verifies: Tables can be created and removed from catalog
- Edge cases: Duplicate table names, deletion of non-existent table
- Assertions: Catalog reflects correct table count

**test_field_addition_removal**

- Verifies: Fields can be added/removed with schema validation
- Edge cases: Duplicate field names, invalid type identifiers
- Assertions: Schema updates propagate to existing records

**test_schema_json_serialization**

- Verifies: Schema serializes/deserializes correctly
- Edge cases: Empty schema, circular references in relations
- Assertions: Round-trip serialization preserves all data

## 5. Custom Types Module Tests

**test_builtin_scalar_types**

- Verifies: All built-in scalar types supported
- Edge cases: Type coercion, overflow/underflow
- Assertions: Type registry contains expected types

**test_composite_type_registration**

- Verifies: User-defined composite types can be registered
- Edge cases: Nested composite types, recursive type definitions
- Assertions: Composite types can be used in field definitions

## 6. Relations Module Tests

**test_relation_creation**

- Verifies: Relations between tables can be defined
- Edge cases: Self-referential relations, circular dependencies
- Assertions: Relation metadata stored correctly

**test_referential_integrity**

- Verifies: Record deletion respects relations
- Edge cases: Cascade delete, orphan prevention
- Assertions: Related records handled according to policy

## 7. Concurrency Module Tests

**test_crud_atomicity**

- Verifies: Each CRUD operation is atomic (all-or-nothing)
- Edge cases: Concurrent modifications, partial failures
- Assertions: No partial state visible to other threads

**test_arcswap_buffer_swapping**

- Verifies: ArcSwap allows atomic buffer swaps
- Edge cases: Concurrent reads during swap, memory reclamation
- Assertions: Readers see consistent state, no data races

**test_lock_free_reads**

- Verifies: Readers not blocked by writers
- Edge cases: High contention, reader starvation
- Assertions: Read operations complete without waiting

## 8. API Module Tests (Integration)

**test_rest_endpoint_validation**

- Verifies: REST endpoints validate input correctly
- Edge cases: Malformed JSON, missing required fields
- Assertions: Appropriate HTTP status codes returned

**test_rpc_dispatch**

- Verifies: RPC requests dispatched to correct handlers
- Edge cases: Unknown methods, handler panics
- Assertions: Responses match expected format

**test_procedure_transaction**

- Verifies: Procedures execute within transactions
- Edge cases: Procedure panic triggers rollback
- Assertions: Changes committed only on success

## 9. Runtime Module Tests

**test_event_loop_tickrate**

- Verifies: Event loop runs at configured tickrate (15-120 Hz)
- Edge cases: Tickrate boundaries, handler execution time
- Assertions: Actual frequency matches configured frequency

**test_parallel_iteration**

- Verifies: Parallel iteration respects cache locality
- Edge cases: Empty tables, single-core execution
- Assertions: Chunk size aligned to cache line

## 10. Persistence Module Tests

**test_binary_snapshot_integrity**

- Verifies: Database snapshots preserve all data
- Edge cases: Large databases, concurrent modifications during snapshot
- Assertions: Checksum validation, round-trip load/save

**test_snapshot_recovery**

- Verifies: Database recovers correctly from snapshot
- Edge cases: Corrupted snapshot file, version mismatch
- Assertions: Recovered state matches original state

## Test Organization Strategy

1. **Unit Tests**: Place `#[cfg(test)]` modules in each source file
2. **Integration Tests**: `tests/` directory with separate test files
3. **Benchmark Tests**: `benches/` directory for performance testing
4. **Property Tests**: Use `proptest` for generative testing of invariants
5. **Concurrency Tests**: Use `loom` for testing concurrent behavior

## Edge Cases to Consider

- **Memory Safety**: All unsafe blocks must have safety comments and validation
- **Concurrency**: Test under high contention with multiple threads
- **Error Handling**: Test all error paths and recovery scenarios
- **Performance**: Benchmark critical paths (record access, buffer swapping)
- **Platform Differences**: Consider endianness and alignment differences
- **Resource Limits**: Test with maximum capacity and memory exhaustion

## Test Assertions Framework

- Use Rust's built-in `assert!`, `assert_eq!`, `assert_ne!`
- For panics: `#[should_panic]` attribute
- For async: `tokio::test` or `async_std::test`
- For property testing: `proptest::proptest!`
