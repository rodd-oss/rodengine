# Test Plan for task_lf_2: Ensure readers are not blocked by concurrent writers

## Context

Part of lock-free reads feature using ArcSwap for atomic buffer swapping in relational in-memory database for online games (Rust implementation).

## Test Suite

### 1. Basic Concurrency Test

**Test Name**: `test_concurrent_readers_writers`
**Description**: Verify multiple readers can read while writers are writing.
**Verification**:

- Spawn N reader threads that continuously read from buffer
- Spawn M writer threads that perform buffer swaps via ArcSwap
- Assert all readers complete without blocking/timeouts
- Verify readers see consistent snapshots (either old or new buffer, never partial)

### 2. Writer Starvation Prevention Test

**Test Name**: `test_writer_starvation_prevention`
**Description**: Ensure writers can make progress even with many concurrent readers.
**Verification**:

- Create many reader threads holding references to old buffer
- Writer thread attempts buffer swap
- Assert writer can complete swap within reasonable time
- Verify new readers see updated buffer after swap

### 3. Buffer Consistency Test

**Test Name**: `test_buffer_consistency_during_swap`
**Description**: Verify readers see either complete old buffer or complete new buffer, never mixed state.
**Verification**:

- Writer prepares new buffer with different data
- During swap, readers sample data
- Assert each reader sees either all old values or all new values
- No reader should see partial update (some fields old, some new)

### 4. Memory Safety Test

**Test Name**: `test_memory_safety_concurrent_access`
**Description**: Ensure no use-after-free when buffers are swapped.
**Verification**:

- Readers obtain Arc references via `ArcSwap::load`
- Writer swaps to new buffer, old buffer should have Arc count > 0
- Readers continue using old buffer references
- Assert no segfaults or invalid memory access
- Verify old buffer is dropped only after last reader releases it

### 5. Performance/Latency Test

**Test Name**: `test_reader_latency_under_writer_load`
**Description**: Measure reader response times during heavy write activity.
**Verification**:

- Baseline: reader latency with no writers
- Under load: reader latency with concurrent buffer swaps
- Assert reader latency doesn't increase significantly (e.g., < 2x baseline)
- Reader operations should complete in O(1) time regardless of writer activity

### 6. High Contention Stress Test

**Test Name**: `test_high_contention_stress`
**Description**: Stress test with many concurrent readers and writers.
**Verification**:

- 100+ reader threads continuously reading
- 10+ writer threads performing frequent swaps
- Run for extended duration (e.g., 10 seconds)
- Assert no deadlocks, panics, or data corruption
- Verify all threads make progress

### 7. Snapshot Isolation Test

**Test Name**: `test_snapshot_isolation`
**Description**: Verify readers get point-in-time consistent snapshots.
**Verification**:

- Writer performs series of sequential updates (A→B→C)
- Readers capture snapshots at different times
- Assert each reader sees one of {A, B, C} consistently
- No reader should see intermediate states between updates

### 8. Error Case: Writer Panic Test

**Test Name**: `test_writer_panic_doesnt_block_readers`
**Description**: Ensure writer panic doesn't leave readers blocked.
**Verification**:

- Writer thread panics during buffer preparation
- Readers should continue accessing current buffer
- Assert readers aren't blocked waiting for writer
- System should recover for next writer attempt

## Edge Cases to Consider

1. **Zero readers during swap**: Writer should succeed immediately
2. **Single reader holding reference**: Old buffer should persist until reader drops it
3. **Rapid successive swaps**: Readers should see sequential snapshots
4. **Large buffer sizes**: Memory pressure shouldn't affect concurrency
5. **Mixed read/write patterns**: Some readers doing heavy computation on buffer
6. **System clock changes**: Timestamp-based operations during swaps
7. **CPU core affinity**: Threads pinned to different cores
8. **Power-of-two buffer sizes**: Alignment considerations

## Assertions/Expected Behaviors

- `ArcSwap::load` should never block
- Readers should always get valid `Arc<Vec<u8>>` reference
- Writer `store` should be atomic and non-blocking for readers
- Memory ordering: `Acquire` for loads, `Release` for stores
- Reference counts should track active readers correctly
- No data races or undefined behavior
- Readers see sequentially consistent snapshots

## Implementation Notes

- Use `std::thread` for concurrency testing
- Consider using `std::sync::atomic` for coordination
- Use `ArcSwap` crate as specified in TRD
- Test with various buffer sizes (small to large)
- Include timing measurements for performance tests
- Use `#[test]` attribute for all test functions
- Consider using `#[cfg(test)]` module for test helpers
