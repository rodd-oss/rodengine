# Test Plan for task_ds_2: Perform snapshot in background thread without blocking main loop

## 1. Core Functionality Tests

**test_background_snapshot_starts_without_blocking**

- **Verifies**: Snapshot initiation doesn't block the main event loop
- **Assertions**: Main loop continues processing ticks while snapshot runs
- **Edge cases**: High-frequency tickrate (120 Hz), snapshot with large dataset

**test_snapshot_completes_asynchronously**

- **Verifies**: Background thread completes snapshot and signals completion
- **Assertions**: Completion callback invoked, snapshot file created
- **Edge cases**: Thread panic handling, cancellation signals

**test_concurrent_snapshots_queued_or_rejected**

- **Verifies**: Behavior when multiple snapshot requests arrive
- **Assertions**: Either queues requests or rejects new ones while busy
- **Edge cases**: Rapid-fire snapshot requests, queue overflow

## 2. Thread Coordination Tests

**test_main_loop_continues_during_snapshot**

- **Verifies**: Event loop maintains tickrate during snapshot operation
- **Assertions**: Tick intervals remain consistent (±10%)
- **Edge cases**: CPU-intensive snapshots, system load spikes

**test_snapshot_thread_priority_lower_than_main**

- **Verifies**: Background thread doesn't starve main loop
- **Assertions**: Main loop responsiveness maintained
- **Edge cases**: Heavy I/O during snapshot, memory pressure

**test_thread_cleanup_on_drop**

- **Verifies**: Background threads properly cleaned up
- **Assertions**: No zombie threads, resources released
- **Edge cases**: Early database shutdown, panic in snapshot

## 3. Data Consistency Tests

**test_snapshot_consistent_state**

- **Verifies**: Snapshot captures atomic database state
- **Assertions**: Snapshot represents point-in-time consistency
- **Edge cases**: Concurrent writes during snapshot initiation

**test_buffer_swapping_during_snapshot**

- **Verifies**: `ArcSwap` buffer swaps don't corrupt snapshot
- **Assertions**: Snapshot uses consistent buffer reference
- **Edge cases**: Multiple `ArcSwap` updates during snapshot

**test_schema_included_in_snapshot**

- **Verifies**: Both data buffers and schema serialized
- **Assertions**: Complete database state preserved
- **Edge cases**: Schema modifications during snapshot

## 4. Resource Contention Tests

**test_memory_usage_during_snapshot**

- **Verifies**: Snapshot doesn't cause memory exhaustion
- **Assertions**: Peak memory within bounds, no OOM
- **Edge cases**: Large databases (>1GB), limited system memory

**test_disk_io_doesnt_block**

- **Verifies**: File I/O in background thread
- **Assertions**: Main loop unaffected by disk writes
- **Edge cases**: Slow disks, full filesystems

**test_cpu_contention_handling**

- **Verifies**: Background thread yields to main loop
- **Assertions**: Main loop maintains minimum tickrate
- **Edge cases**: CPU-bound snapshot processing

## 5. Error Handling Tests

**test_snapshot_failure_handling**

- **Verifies**: Failed snapshots don't crash system
- **Assertions**: Error reported, system continues
- **Edge cases**: Disk full, permission denied, corrupted buffers

**test_thread_panic_isolation**

- **Verifies**: Background thread panic contained
- **Assertions**: Main loop continues, error logged
- **Edge cases**: Double panics, memory corruption

**test_cancellation_support**

- **Verifies**: Snapshots can be cancelled
- **Assertions**: Thread stops cleanly, resources freed
- **Edge cases**: Mid-write cancellation, partial files

## 6. Performance Tests

**test_snapshot_latency_measurement**

- **Verifies**: Snapshot completion time tracked
- **Assertions**: Latency within SLA (e.g., <100ms for 1MB)
- **Edge cases**: Varying database sizes

**test_main_loop_jitter_measurement**

- **Verifies**: Tick jitter during snapshot
- **Assertions**: Jitter within acceptable bounds
- **Edge cases**: Concurrent API requests

**test_throughput_under_snapshot**

- **Verifies**: Database operations continue
- **Assertions**: CRUD operations succeed during snapshot
- **Edge cases**: High write load during snapshot

## Key Edge Cases to Consider:

1. **Concurrent modifications**: Writes happening while snapshot copies buffers
2. **Buffer lifetime**: Ensuring `Arc` references live long enough for snapshot
3. **Thread scheduling**: OS thread scheduling variations
4. **Resource exhaustion**: Memory, disk space, file descriptors
5. **Signal handling**: Interrupts, termination signals
6. **Recovery**: Partial snapshots, corrupted output files
7. **Rate limiting**: Preventing snapshot spam
8. **Priority inversion**: Background thread blocking main loop
9. **Memory ordering**: Ensuring proper synchronization between threads
10. **Clean shutdown**: Graceful termination during snapshot

Each test should use Rust's `std::thread`, `std::sync`, and potentially `crossbeam` or `tokio` for concurrency primitives. Mock file I/O where possible to isolate threading behavior from disk performance.
