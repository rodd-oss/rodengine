# Test Plan for task_pp_2: Ensure parallel iteration respects cache locality (chunk size aligned to cache line)

## Context

This task follows task_pp_1 (parallel iteration API using rayon) and focuses on optimizing cache locality by aligning chunk sizes to cache lines (typically 64 bytes on modern CPUs). Part of a Rust relational in-memory database for online games using Vec<u8> storage with tight packing and zero-copy access. Implements TRD's cache efficiency and procedural parallelism requirements.

## Test Names and Descriptions

### 1. test_chunk_size_aligned_to_cache_line

- Verifies that parallel iteration chunk sizes are multiples of cache line size
- Tests with different record sizes and table sizes
- Ensures chunk boundaries align with cache line boundaries

### 2. test_cache_line_alignment_with_small_records

- Tests edge case where record size is smaller than cache line
- Ensures chunks contain whole cache lines worth of data
- Verifies multiple records per cache line are handled correctly

### 3. test_cache_line_alignment_with_large_records

- Tests edge case where record size is larger than cache line
- Ensures chunks start on cache line boundaries
- Verifies single records spanning multiple cache lines

### 4. test_boundary_alignment_cases

- Tests records that cross cache line boundaries
- Ensures no record is split across cache lines within a chunk
- Verifies record integrity across chunk boundaries

### 5. test_different_cache_line_sizes

- Tests with configurable cache line sizes (32, 64, 128 bytes)
- Verifies algorithm adapts to different hardware
- Ensures chunk size calculation respects configured cache line size

### 6. test_empty_and_single_record_tables

- Edge cases: empty table and table with single record
- Ensures no panic and proper handling
- Verifies chunk size calculation for minimal data

### 7. test_chunk_size_calculation_consistency

- Verifies chunk size calculation is deterministic
- Tests same inputs produce same chunk sizes
- Ensures thread-safe calculation

## What Each Test Verifies

1. **Chunk alignment**: Chunk boundaries align with cache line boundaries
2. **Whole records**: No record is split between chunks
3. **Optimal sizing**: Chunk sizes are optimal multiples of cache line size
4. **Memory access**: Sequential memory access patterns within chunks
5. **Performance invariants**: Chunk size ≥ cache line size, chunk size % cache line size == 0
6. **Edge case handling**: Empty tables, small tables, odd-sized records
7. **Configurability**: Works with different cache line sizes
8. **Determinism**: Same inputs produce same parallel partitioning

## Edge Cases to Consider

- **Record size < cache line**: Multiple records per cache line
- **Record size > cache line**: Single record spans multiple cache lines
- **Record size % cache line != 0**: Partial cache line usage
- **Table size % cache line != 0**: Trailing partial cache line
- **Very small tables**: < cache line size
- **Very large tables**: > memory page size
- **Different architectures**: 32-byte vs 64-byte vs 128-byte cache lines
- **Record alignment**: Records not naturally aligned to cache lines
- **Concurrent access**: While parallel iteration is happening
- **Misaligned buffer start**: Buffer not starting at cache line boundary
- **Mixed record sizes**: If table supports variable-length records

## Assertions and Expected Behaviors

1. **Chunk size assertion**: `chunk_size % CACHE_LINE_SIZE == 0`
2. **Record integrity**: Each record appears exactly once across all chunks
3. **Boundary alignment**: `(record_start_offset % CACHE_LINE_SIZE) + record_size <= CACHE_LINE_SIZE` for records within chunks
4. **Performance**: Measured cache misses should be minimized (can use perf counters in integration tests)
5. **Correctness**: Parallel iteration produces same results as sequential iteration
6. **No data races**: Thread-safe access to shared buffer via ArcSwap
7. **Memory safety**: No out-of-bounds access even with misaligned data
8. **Zero-copy preservation**: Parallel iteration maintains zero-copy reference semantics

## Implementation Notes for Tests

- Use `std::sync::atomic` for thread-safe assertions
- Mock cache line sizes for testing different architectures
- Verify rayon's `par_chunks` or similar API is called with properly aligned chunk sizes
- Test with both aligned and unaligned buffer starting addresses
- Include performance benchmarks comparing aligned vs unaligned chunking
- Ensure tests work with the existing `Vec<u8>` storage buffer and record packing from previous tasks
- **TRD Integration**: Cache-optimized parallel iteration will be used in custom procedures exposed via REST API
- **Concurrency Model**: Maintains lock-free reads via ArcSwap as required by TRD
- **Performance Goals**: Achieves TRD's procedural parallelism across CPU cores while maximizing cache hits
