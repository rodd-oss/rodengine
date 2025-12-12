# Test Plan for Task `lf_1`: Read API with ArcSwap::load

## Context

Rust implementation of in-memory relational database. Implements TRD requirement: "Atomic operations via ArcSwap on buffers; lock-free reads and writes." ArcSwap load enables lock‑free reads for REST API endpoints.

## 1. Basic Functionality Tests

- **`test_load_returns_valid_reference`**: Verifies `ArcSwap::load()` returns a non-null reference to the buffer
- **`test_load_preserves_buffer_contents`**: Ensures loaded reference points to same data as original buffer
- **`test_multiple_concurrent_loads`**: Multiple threads can load references simultaneously without blocking

## 2. Concurrency & Atomicity Tests

- **`test_load_during_concurrent_write`**: Reader loads buffer while writer swaps it via ArcSwap
- **`test_stale_reference_remains_valid`**: Old references remain valid after buffer swap (Arc ensures lifetime)
- **`test_memory_ordering_guarantees`**: Verify `ArcSwap::load` provides proper memory ordering (likely `Acquire`)

## 3. Edge Case Tests

- **`test_load_empty_buffer`**: Handle empty `Vec<u8>` buffer (zero-length)
- **`test_load_after_multiple_swaps`**: Buffer reference remains valid after multiple ArcSwap operations
- **`test_concurrent_loads_with_heavy_contention`**: Stress test with many threads loading simultaneously
- **`test_load_with_dropped_original_arc`**: Original Arc dropped but loaded reference still valid

## 4. Integration Tests

- **`test_read_api_integration_with_storage`**: Read API works with actual table storage buffer
- **`test_zero_copy_access_via_loaded_reference`**: Can perform zero-copy field access via loaded buffer reference
- **`test_iterator_over_loaded_buffer`**: Can iterate records using loaded buffer reference

## 5. Safety & Validation Tests

- **`test_no_data_races`**: Verify no data races between readers and writers
- **`test_buffer_lifetime_guarantees`**: Loaded references don't outlive ArcSwap instance
- **`test_thread_safety`**: API can be used safely across thread boundaries (Send + Sync)

## Key Assertions & Behaviors:

- Loaded reference should be `&Vec<u8>` or `Arc<Vec<u8>>` (depending on ArcSwap API)
- Multiple concurrent loads should succeed without panics
- Load operation should be lock-free (no mutex contention)
- Memory ordering should ensure readers see consistent buffer state
- Old references should remain valid due to Arc reference counting
- API should be Send + Sync for cross-thread usage

## Edge Cases to Consider:

- Buffer size changes during load
- Concurrent loads while buffer is being dropped
- Memory pressure causing Arc cloning failures
- Thread termination while holding loaded reference
- Nested loads (loading buffer within loaded buffer context)

## 6. REST API Integration Tests

- **`test_rest_api_read_endpoint_uses_arcswap`**: Verify GET /tables/{name}/records/{id} uses ArcSwap::load() for lock‑free reads
- **`test_concurrent_rest_reads_with_writes`**: Multiple REST API read requests while writes occur via buffer swaps
- **`test_rest_api_response_consistency`**: REST responses reflect consistent buffer state from ArcSwap load
- **`test_rest_api_read_performance`**: Measure read latency to ensure within tickrate constraints (15-120 Hz)
- **`test_rest_api_bulk_read_endpoint`**: GET /tables/{name}/records uses ArcSwap load for entire table iteration

## Integration with REST API

- **REST API Reads**: ArcSwap load enables lock‑free reads for REST API endpoints (GET /table/{name}/record/{id}, etc.)
- **Transaction Atomicity**: Each CRUD operation atomic as per TRD, using ArcSwap atomic swaps
- **Event Loop**: Reads complete within tickrate constraints (15-120 Hz) without blocking
