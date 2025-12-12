# Test Plan: task_pp_1 - Parallel Record Iteration API

## Overview

Tests for implementing parallel iteration over table records using Rayon, as part of the Rust relational in-memory database for online games. Implements TRD's procedural parallelism requirement for maximizing cache hits across CPU cores.

## 1. Basic Parallel Iteration Tests

### test_parallel_iter_empty_table

**Verifies**: Parallel iteration over empty table completes without errors
**Edge cases**: Zero records, zero-sized buffer
**Assertions**: No panics, iterator yields no items, rayon pool doesn't hang

### test_parallel_iter_single_record

**Verifies**: Single record can be processed in parallel context
**Edge cases**: Minimal buffer size, record at boundary
**Assertions**: Iterator yields exactly one record, closure executes once

### test_parallel_iter_multiple_records

**Verifies**: All records are visited exactly once
**Edge cases**: Odd/even record counts, varying record sizes
**Assertions**: Count of processed records equals table size, no duplicates

## 2. Concurrency & Safety Tests

### test_parallel_iter_concurrent_reads

**Verifies**: Multiple parallel iterators can read simultaneously
**Edge cases**: Concurrent iteration while table is being read elsewhere
**Assertions**: No data races, all iterators see consistent snapshot

### test_parallel_iter_buffer_swap_safety

**Verifies**: Iteration continues safely during ArcSwap buffer updates
**Edge cases**: Buffer swap mid-iteration
**Assertions**: Iterators continue with old buffer reference, no crashes

### test_parallel_iter_thread_safety

**Verifies**: API can be called from multiple threads
**Edge cases**: Spawning iterators from rayon thread pool
**Assertions**: No thread-local data assumptions violated

## 3. Performance & Correctness Tests

### test_parallel_iter_chunk_size_optimization

**Verifies**: Rayon chunk size aligns with cache lines (64/128 bytes)
**Edge cases**: Records smaller/larger than cache line
**Assertions**: Chunk size minimizes cache misses

### test_parallel_iter_ordering

**Verifies**: Records processed in deterministic order (optional requirement)
**Edge cases**: Parallel execution may reorder
**Assertions**: If ordering required, use indexed parallel iteration

### test_parallel_iter_mutability_constraints

**Verifies**: Read-only access through references (&T)
**Edge cases**: Attempting mutation through iterator
**Assertions**: Compile-time prevention of mutation

## 4. Error & Edge Case Tests

### test_parallel_iter_panic_handling

**Verifies**: Panic in closure doesn't corrupt database state
**Edge cases**: Panic in one rayon worker thread
**Assertions**: Other threads continue, database remains consistent

### test_parallel_iter_memory_safety

**Verifies**: No out-of-bounds access during parallel iteration
**Edge cases**: Malformed buffer, incorrect record size calculation
**Assertions**: Bounds checks in iterator implementation

### test_parallel_iter_large_dataset

**Verifies**: Scalability with thousands/millions of records
**Edge cases**: Memory pressure, rayon work stealing
**Assertions**: Linear speedup with cores, no OOM

## 5. API Contract Tests

### test_parallel_iter_api_signature

**Verifies**: API matches expected signature: `fn parallel_iter<F>(&self, f: F) where F: Fn(&Record) + Send + Sync`
**Edge cases**: Closure capturing environment
**Assertions**: Compiles with expected constraints

### test_parallel_iter_with_custom_types

**Verifies**: Works with user-defined composite types (Vec3, etc.)
**Edge cases**: Non-standard alignments, padding
**Assertions**: Type casting preserves data integrity

### test_parallel_iter_relation_consistency

**Verifies**: Iteration doesn't violate referential integrity
**Edge cases**: Records with foreign key references
**Assertions**: Related records remain accessible

## Integration with TRD Architecture

- **REST API Procedures**: Parallel iteration will be available via custom transactional procedures executed through REST API endpoints
- **Event Loop Integration**: Procedures using parallel iteration execute within the 15–120 Hz tickrate constraints
- **JSON Schema**: Works with tables defined via JSON schema files, respecting field types and composite type definitions
- **ArcSwap Concurrency**: Maintains lock-free reads during parallel iteration as required by TRD

## Key Assertions

1. Record count consistency
2. No data corruption during parallel access
3. Thread safety (Send + Sync bounds)
4. Memory safety (bounds checks)
5. Performance metrics (optional: benchmark)
6. Error recovery from panics
7. Buffer swap atomicity guarantees

## Test Data Variations

- Empty tables
- Single record tables
- Power-of-two record counts
- Prime number record counts
- Mixed field types (scalars, composites)
- Tables at capacity limits
- Concurrent modification scenarios
