# Test Plan: Copy-on-Write Strategy (task_ab_3)

## Overview

Unit tests for implementing copy-on-write strategy for modifications in the relational in-memory database. Part of the ArcSwap buffer feature in the concurrency phase.

## 1. Core Functionality Tests

### test_copy_on_write_on_first_modification

**Verifies**: When a record is modified for the first time, a new buffer is allocated and old data is copied.
**Edge cases**: Empty buffer, single record, multiple records.
**Assertions**: Original buffer unchanged, new buffer contains modification, reference counts correct.

### test_no_copy_on_read_only_access

**Verifies**: Reading records doesn't trigger copy-on-write.
**Edge cases**: Concurrent reads while another thread holds write reference.
**Assertions**: Same buffer reference returned, no allocations.

### test_multiple_writers_sequential_copies

**Verifies**: Each writer gets their own copy, modifications don't interfere.
**Edge cases**: Writers modifying different records, same record.
**Assertions**: Each writer sees only their modifications, buffers diverge.

## 2. Reference Counting & Memory Management Tests

### test_buffer_reclamation_when_no_references

**Verifies**: Old buffers are dropped when no readers hold references.
**Edge cases**: Readers holding old buffer while new writes occur.
**Assertions**: Memory usage doesn't grow unbounded, Arc::strong_count drops to 1.

### test_concurrent_readers_during_write

**Verifies**: Readers continue using old buffer while writer modifies copy.
**Edge cases**: High contention, many concurrent readers.
**Assertions**: Readers see consistent old state, writer sees new state.

## 3. Performance & Edge Case Tests

### test_copy_only_modified_records_optimization

**Verifies**: Implementation optimizes by copying only modified portions if possible.
**Edge cases**: Single field modification in large record, adjacent record modifications.
**Assertions**: Copy size matches expected optimization.

### test_zero_sized_types_no_copy

**Verifies**: Zero-sized types (like `()`) don't trigger unnecessary copies.
**Edge cases**: Tables with only zero-sized fields.
**Assertions**: No buffer allocation on "modification".

### test_modification_detection_mechanism

**Verifies**: System detects when modification actually occurs vs no-op writes.
**Edge cases**: Writing same value back, partial writes that don't change data.
**Assertions**: No copy when values unchanged.

## 4. Atomicity & Consistency Tests

### test_atomic_buffer_swap

**Verifies**: Buffer swap is atomic - readers never see partially written state.
**Edge cases**: Concurrent reads during swap.
**Assertions**: All readers see either old or new complete buffer.

### test_rollback_on_panic

**Verifies**: If modification panics, original buffer remains intact.
**Edge cases**: Panic during copy, panic during modification.
**Assertions**: Original buffer unchanged, no memory leaks.

## 5. Integration Tests

### test_copy_on_write_with_complex_schema

**Verifies**: Copy-on-write works with nested/composite types.
**Edge cases**: Variable-length fields, custom types with internal pointers.
**Assertions**: Deep copy preserves all data correctly.

### test_performance_benchmark_copy_overhead

**Verifies**: Copy overhead scales linearly with modified data size.
**Edge cases**: Large buffers, small modifications.
**Assertions**: Performance within acceptable bounds for game database.

## Key Assertions to Include

- `Arc::strong_count()` checks for reference counting
- `ptr::eq()` comparisons for buffer identity
- Memory allocation tracking via `std::alloc`
- Data integrity verification via checksums
- Concurrent access patterns with `std::thread`

## Edge Cases to Consider

1. **Empty buffers**: Modification should allocate new buffer
2. **Single writer, many readers**: Readers should stay on old buffer
3. **Rapid sequential modifications**: Should create chain of buffers
4. **Memory pressure**: Large copies under low memory conditions
5. **Alignment requirements**: Copied data maintains proper alignment
6. **Cache line boundaries**: Copies respect CPU cache lines
7. **Thread locality**: Writers on different CPU cores
8. **Signal/interrupt safety**: Modifications during signals
