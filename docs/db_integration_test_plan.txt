# In-Memory Database - Top-Down Integration Test Plan

## 1. Basic CRUD Integration Tests

### 1.1 Full Table Lifecycle

```rust
fn test_table_create_delete_integration() {
    // Setup: Initialize empty Database with REST API server on port 0
    // Execute: POST /tables/test_table with field definitions
    // Assert: SchemaMap contains table, schema.json written to disk, ArcSwap buffer allocated
    // Execute: DELETE /tables/test_table
    // Assert: SchemaMap empty, schema.json updated, buffer dropped
    // Teardown: Shutdown server, cleanup data_dir
}
```

### 1.2 End-to-End Record CRUD

```rust
fn test_record_crud_full_path() {
    // Setup: Create table with u64 id, string name, 3xf32 position
    // Execute: POST /records with values [1, "entity", [0.0, 0.0, 0.0]]
    // Assert: Buffer contains 1 record, next_id == 2, record at offset 0 matches serialized data
    // Execute: GET /records/1
    // Assert: Response body is exact byte match to buffer slice 0..record_size
    // Execute: PUT /records/1 with updated values
    // Assert: Buffer swapped, offset 0 contains new data, old buffer dropped
    // Execute: DELETE /records/1
    // Assert: Buffer either has deleted flag or record removed (depending on impl)
    // Teardown: Drop table
}
```

### 1.3 Custom Type Serialization Roundtrip

```rust
fn test_custom_type_3xf32_integration() {
    // Setup: Register 3xf32 type, create table with single 3xf32 field
    // Execute: Create record with [1.0, 2.0, 3.0]
    // Assert: Buffer contains 12 bytes: [0x00, 0x80, 0x3F, ...] (IEEE 754)
    // Execute: Read record via GET /records/1
    // Assert: Deserialized Vector3 matches input exactly (bitwise)
    // Execute: Update to [NaN, -inf, +inf]
    // Assert: Buffer contains correct IEEE 754 representations
    // Teardown: Unregister type (requires schema.json update)
}
```

## 2. Transaction Integration Tests

### 2.1 Multi-Op Transaction Atomicity

```rust
fn test_transaction_all_or_nothing() {
    // Setup: Table with u64 field, initial value at id=1 is 0
    // Execute: Begin explicit transaction
    // - Stage: Update id=1 to 100
    // - Stage: Update id=2 (non-existent) to 200 (should fail)
    // - Attempt commit
    // Assert: No buffer swap occurred, id=1 still 0, transaction marked failed
    // Execute: Valid multi-op transaction (create 3 records)
    // Assert: All 3 appear atomically (no partial state visible to concurrent readers)
    // Teardown: Verify ArcSwap history has exactly 2 published buffers
}
```

### 2.2 Transaction Isolation Levels

```rust
fn test_read_committed_isolation() {
    // Setup: Table with single record (id=1, value=0), two API client handles
    // Execute: Client A begins transaction, updates value to 100 (not committed)
    // Execute: Client B reads record id=1
    // Assert: B sees value 0 (uncommitted changes not visible)
    // Execute: Client A commits
    // Execute: Client B reads again
    // Assert: B now sees value 100 (read committed)
    // Teardown: Check that Client B's first read returned immutable Arc that remains valid after swap
}
```

### 2.3 Concurrent Transaction Conflicts

```rust
fn test_last_writer_wins_semantics() {
    // Setup: Table with u64 counter field, initial value 0
    // Execute: Spawn 10 threads, each performing: read → increment → update → commit
    // Assert: Final value is 10 (all commits applied, no lost updates due to explicit intent)
    // Verify: ArcSwap load/store ordering prevents torn writes
    // Teardown: Count total successful commits via next_id counter
}
```

## 3. Concurrency Integration Tests

### 3.1 Lock-Free Read Scaling

```rust
fn test_parallel_read_cache_friendly() {
    // Setup: Populate table with 1M records (tightly packed, ~24MB total)
    // Execute: Spawn threads = num_cpus, each iterates over all records summing a u64 field
    // Assert: Sum is correct, no data races, each thread maintains cache line locality
    // Measure: No mutex contention in perf output, L1 miss rate < 2%
    // Verify: Each thread's Arc remains valid for entire iteration despite concurrent updates
    // Teardown: Check Arc reference counts drop to zero after iteration
}
```

### 3.2 Write Contention Under Load

```rust
fn test_write_contention_100k_ops() {
    // Setup: Table with 100 records, 64-byte record size (1 record per cache line)
    // Execute: Spawn 100 writers, each performs 1000 random updates to random records
    // Assert: All 100k updates eventually succeed, final state consistent
    // Verify: No deadlocks, no mutex usage in flamegraph
    // Measure: p99 write latency < 10μs despite contention
    // Teardown: Verify buffer version count matches total commits
}
```

### 3.3 ArcSwap Buffer Lifetime Management

```rust
fn test_arcswap_buffer_drop_integrity() {
    // Setup: Table with 1 record, spawn 1 slow reader (sleeps 100ms during iteration)
    // Execute: While reader holds Arc, perform 1000 rapid updates (each creates new buffer)
    // Assert: Old buffers not dropped until reader finishes (ArcSwap epoch semantics)
    // Verify: Memory usage grows during rapid updates, drops after reader release
    // Measure: Peak memory < 2x baseline (no unbounded growth)
    // Teardown: Confirm all old buffer Arcs dropped, no leaks
}
```

## 4. Procedure Integration Tests

### 4.1 RPC to Parallel Procedure Execution

```rust
fn test_rpc_procedure_full_pipeline() {
    // Setup: Register procedure `sum_field` that parallel-sums a u64 column
    // Execute: POST /rpc/sum_field with {"table": "test", "field": "value"}
    // Assert: Procedure spawned on RuntimeLoop, RuntimeLoop schedules during Procedure phase
    // Verify: Procedure uses Rayon to shard across cores, each core sums its chunk
    // Verify: Final reduction is correct, commit happens in same tick
    // Measure: Execution time ~ (data_size / num_cores / memory_bandwidth)
    // Teardown: Unregister procedure
}
```

### 4.2 Procedure Transaction Isolation

```rust
fn test_procedure_transactional_integrity() {
    // Setup: Table with 1M records, procedure that flips a bool field for all
    // Execute: Start procedure, immediately fire API writes to same table
    // Assert: API writes succeed independently (procedure runs on staging buffer)
    // Verify: Procedure's staging buffer not visible to API reads until commit
    // Execute: Force procedure panic mid-execution (via special parameter)
    // Assert: No records modified in main buffer (transaction aborted)
    // Teardown: Check staging buffer dropped on panic
}
```

### 4.3 Procedure Cache Locality Maximization

```rust
fn test_procedure_cache_hit_optimization() {
    // Setup: Table with 10M records (fits in L3 cache), record size 64 bytes
    // Execute: Procedure iterates with `par_chunks_exact` aligned to cache lines
    // Assert: Each CPU core processes contiguous region, no false sharing
    // Measure: Perf shows ~1 cache miss per 64 bytes (optimal)
    // Verify: Chunk boundaries respect 64-byte alignment, no record straddles cache lines
    // Teardown: Profile with `perf c2c` to verify no cross-cache-line contention
}
```

## 5. Runtime Loop Integration Tests

### 5.1 Tick Phase Timing Enforcement

```rust
fn test_tick_phase_timing_budget() {
    // Setup: Runtime with 60 Hz tick (16.66ms), mock API handler (5ms), mock procedure (8ms)
    // Execute: Run 100 ticks with continuous load
    // Assert: API phase never exceeds 5ms (30% of tick), Procedure phase never exceeds 8ms (50%)
    // Verify: When API exceeds budget, requests queued to next tick
    // Verify: When procedure exceeds budget, it continues in next tick (spanning ticks)
    // Measure: Tick duration variance < 1% (consistent timing)
    // Teardown: Shutdown runtime, verify no pending work
}
```

### 5.2 Rate Limiting and Backpressure

```rust
fn test_api_rate_limiting_integration() {
    // Setup: Runtime tickrate 15 Hz, max_api_requests_per_tick = 150 (10 per tick)
    // Execute: Flood POST /records at 1000 req/s sustained
    // Assert: 150 requests succeed per tick, remainder return 503
    // Verify: Queue size never exceeds tickrate * 100 (1500)
    // Execute: Reduce flood to 50 req/s
    // Assert: All requests succeed, queue drains
    // Teardown: Check metrics: dropped_requests counter matches expected
}
```

### 5.3 Procedure Spanning Multiple Ticks

```rust
fn test_procedure_tick_spanning() {
    // Setup: Runtime 120 Hz (8.33ms/tick), procedure that processes 10M records (takes ~50ms)
    // Execute: Fire RPC to start procedure
    // Assert: Procedure begins in tick N, runs for ~6 ticks
    // Verify: In each tick, runtime calls procedure with remaining chunk
    // Verify: API phase continues normally in ticks N+1..N+5 (procedure doesn't block)
    // Execute: Fire second procedure while first running
    // Assert: Second procedure queued, starts after first completes (tick N+6)
    // Teardown: Verify both procedures committed successfully
}
```

## 6. Persistence Integration Tests

### 6.1 Async Data Flush Integrity

```rust
fn test_async_persistence_atomicity() {
    // Setup: Table with 1000 records, persistence_interval_ticks = 5
    // Execute: Perform 100 rapid writes, wait for 6 ticks
    // Assert: Data file appears in data_dir, size matches buffer length
    // Verify: File written to temp (.bin.tmp) then renamed (atomic)
    // Execute: Kill process mid-flush (simulate power loss)
    // Assert: On recovery, either old or new data present (no corruption)
    // Teardown: Check file mtimes match tick boundaries
}
```

### 6.2 Schema Persistence and Recovery

```rust
fn test_schema_persistence_recovery() {
    // Setup: Create table with custom type, add field, create relation
    // Execute: Check schema.json contains all metadata (record_size, offsets, types)
    // Assert: File is valid JSON, custom types section includes 3xf32
    // Teardown: Drop Database instance
    // Execute: New Database instance with same data_dir
    // Assert: SchemaMap rebuilt from schema.json, TypeRegistry restored
    // Verify: Table buffer loaded from data file, record_size matches
    // Verify: Relations reconstructed, foreign key fields validated
    // Teardown: Compare recovered state to original
}
```

### 6.3 Parallel Persistence with Active Writes

```rust
fn test_persistence_during_write_burst() {
    // Setup: Table with 1M records, flush interval 1 tick (every 16ms)
    // Execute: Sustained write load of 10k writes/sec while persistence runs
    // Assert: No data loss, persisted file eventually catches up to latest version
    // Verify: ArcSwap old buffers held until flush completes (epoch pinning prevents UAF)
    // Measure: Write latency not impacted by background flush (separate thread)
    // Verify: Buffer versions stabilize after load stops (no unbounded growth)
    // Teardown: Final flush succeeds, file size matches final buffer
}
```

## 7. Relation Integration Tests

### 7.1 Foreign Key Integrity Across Tables

```rust
fn test_relation_integrity_across_tables() {
    // Setup: Create 'users' table (u64 id, string name), 'posts' table (u64 user_id, string content)
    // Execute: Create relation users.id -> posts.user_id
    // Assert: Relation stored in both tables' relation vectors
    // Execute: Create user id=1, create post with user_id=1
    // Assert: Both succeed (no referential integrity enforcement by default)
    // Execute: Delete user id=1
    // Assert: User removed; relation cascade behavior tested (depends on config)
    // Verify: Procedure can efficiently join via offset calculation: post.user_id_offset → user record_offset
    // Teardown: Delete relation first, then tables
}
```

### 7.2 Relation Query Performance

```rust
fn test_relation_scan_performance() {
    // Setup: 1M users, 10M posts, foreign key relation defined
    // Execute: Procedure that finds all posts for user_id=12345 via full scan
    // Assert: Correct posts returned, scan uses cache-friendly iteration
    // Measure: Time ~ O(posts) / num_cores, no random memory access
    // Verify: Each post's user_id field compared without deserialization (raw u64 comparison)
    // Teardown: Compare to indexed approach (if implemented later)
}
```

## 8. Full System Integration Tests

### 8.1 End-to-End Workflow Simulation

```rust
fn test_full_system_e2e_simulation() {
    // Setup: Runtime 60 Hz, persistence_interval 10, 4 CPU cores
    // Phase 1 (DDL):
    // - Create 3 tables via API
    // - Add custom type Vector4f32
    // - Create 2 relations
    // Assert: schema.json updated, buffers allocated

    // Phase 2 (Load):
    // - Spawn 10 writer threads: 1000 records each
    // - Spawn 5 reader threads: continuous scanning
    // Assert: No data races, readers see monotonically increasing record count

    // Phase 3 (Procedure):
    // - Fire RPC to run `aggregate_stats` procedure (parallel aggregation)
    // - Fire RPC to run `cascade_delete` procedure (cross-table deletion)
    // Assert: Both complete, results correct, committed atomically

    // Phase 4 (Persistence):
    // - Wait 11 ticks
    // - Simulate crash, restart
    // Assert: Schema recovered, data matches pre-crash state

    // Phase 5 (Cleanup):
    // - Delete all tables
    // Assert: Clean shutdown, no leaks
}
```

### 8.2 High-Frequency Tick Stress Test

```rust
fn test_120hz_tick_stress() {
    // Setup: Runtime 120 Hz (8.33ms/tick), max_api_requests_per_tick = 960
    // Execute: Sustained load: 1000 API req/s, 5 parallel procedures (each 2ms), persistence every tick
    // Assert: All ticks complete within 8.33ms ± 1%, no deadline overruns
    // Verify: API queue never overflows, procedure queue bounded at 10
    // Measure: CPU usage < 80% (leaves headroom)
    // Teardown: Gradual load reduction, verify graceful degradation
}
```

### 8.3 Memory Pressure and Buffer Growth

```rust
fn test_buffer_growth_under_memory_pressure() {
    // Setup: initial_table_capacity = 10, record_size = 1024 bytes
    // Execute: Insert 1M records (requires 1024 reallocations)
    // Assert: Each reallocation doubles capacity, growth is O(log n)
    // Verify: Old buffers dropped after last reader Arc released (no memory leak)
    // Measure: Peak memory < 2x final size (amortized efficiency)
    // Execute: Delete 500k records (if compacting delete implemented)
    // Assert: Buffer may or may not shrink (implementation dependent)
    // Teardown: Check allocator stats for fragmentation
}
```

## 9. Failure Mode Integration Tests

### 9.1 Procedure Panic Recovery

```rust
fn test_procedure_panic_isolation() {
    // Setup: Procedure that panics when params["panic"] == true
    // Execute: Fire RPC with panic=true
    // Assert: Runtime catches panic, returns 500 error, transaction not committed
    // Verify: Table buffer unchanged, staging buffer dropped without swap
    // Execute: Fire valid RPC, then panic RPC, then valid RPC
    // Assert: First and third succeed, middle fails, no corruption
    // Teardown: Verify thread pool remains functional after panics
}
```

### 9.2 Persistence Write Failure

```rust
fn test_persistence_io_error_handling() {
    // Setup: Fill disk to 100% (use tempfs with quota)
    // Execute: Perform writes that trigger flush
    // Assert: Flush fails with I/O error, logged, but API writes succeed
    // Verify: Runtime loop continues, subsequent flushes retried
    // Execute: Free disk space
    // Assert: Next flush succeeds, all pending data persisted
    // Teardown: Monitor for data loss or corruption
}
```

### 9.3 Schema Corruption Recovery

```rust
fn test_schema_corruption_detection() {
    // Setup: Manually edit schema.json to introduce error (duplicate field name)
    // Execute: Start Database with corrupted schema
    // Assert: Parse error on startup, graceful shutdown with error log
    // Execute: Corrupt custom type definition (size mismatch)
    // Assert: Type validation fails, startup aborted
    // Teardown: Restore valid schema, verify recovery works
}
```

## 10. Performance Regression Integration Tests

### 10.1 Baseline Throughput Check

```rust
fn test_baseline_crud_throughput() {
    // Setup: Table with 10k records, no other load
    // Execute: Measure sustained read throughput (req/s) for 10 seconds
    // Assert: > 10M reads/sec/core (baseline from PRD)
    // Execute: Measure sustained write throughput (creates)
    // Assert: > 1M writes/sec/core
    // Execute: Measure sustained update throughput
    // Assert: > 500k updates/sec/core (includes buffer clone)
    // Teardown: Store results for regression comparison
}
```

### 10.2 Procedure Scaling Linear Regression

```rust
fn test_procedure_linear_scaling() {
    // Setup: Table with 10M records, procedure that reads u64 field
    // Execute: Run procedure with 1, 2, 4, 8 cores (via thread pool config)
    // Assert: Execution time halves as cores double (linear scaling)
    // Measure: Strong scaling efficiency > 90% up to physical core count
    // Verify: No contention in ArcSwap during read-only procedure
    // Teardown: Log scaling curve for trend analysis
}
```

### 10.3 Cache Line Contention Detection

```rust
fn test_false_sharing_prevention() {
    // Setup: Table with 64-byte records (exact cache line size), 1M records
    // Execute: Spawn 2 writers on adjacent records (record 0 and 1)
    // Assert: No performance degradation vs writing to distant records
    // Measure: Perf c2c shows no cross-cache-line invalidations
    // Execute: Test with records spanning cache lines (if misaligned)
    // Assert: Performance drop > 50% (healthy sensitivity to alignment)
    // Teardown: Document record size recommendations
}
```

---

**Test Execution Order**: Run 1-2-3 (basic) → 4-5 (advanced) → 6-7 (persistence/relations) → 8 (full system) → 9 (failure) → 10 (performance). Each test must be independent and runnable in isolation.
