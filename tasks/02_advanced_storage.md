# Phase 2: Advanced Storage
**Estimated Time:** Weeks 5-8

## Overview
Enhance storage layer with delta tracking, atomic commit protocol, referential integrity, sparse component support, lock‑free write queue, memory efficiency improvements, and a complete field codec system.

## Dependencies
- Phase 1 completed (core engine modules)
- Basic CRUD operations functional

## Subtasks

### 2.1 Delta Tracking System
- **DeltaOp** enum: Insert, Update, Delete, CreateEntity, DeleteEntity (with table ID, entity ID, field offset, data)
- **DeltaTracker**: Record changes made in write buffer for each transaction
- **Delta Calculation**: Compare old vs new buffer to produce minimal delta set
- **Delta Storage**: Store deltas per transaction for replication and WAL

### 2.2 Atomic Commit Protocol
- **BufferManager::commit_batch()**: Atomically swap all table buffers simultaneously using atomic pointer operations
- **Version Bumping**: Global version counter (AtomicU64) incremented on each commit
- **Broadcast Notification**: Signal readers about new version (optional callback)
- **Isolation Guarantee**: Readers never see partial commits; all buffers updated at once

### 2.3 Referential Integrity Checks
- **Foreign Key Validation**: On insert/update, verify referenced entity exists
- **Cascade Operations**: Define behavior on entity delete (cascade, restrict, set null)
- **Constraint Registry**: Track foreign‑key relationships from schema
- **Integrity Violation Error**: Rollback transaction if constraint broken

### 2.4 Sparse Component Handling
- **SparseSet**: Bitmap indicating which entities have a given component
- **Sparse Storage Buffer**: Store components only for entities that have them, using indirect indexing
- **Archetype Tracking**: Group entities by component composition for cache‑friendly iteration
- **Dynamic Archetype Changes**: Adding/removing components moves entity between archetypes

### 2.5 Lock‑Free Write Queue (MPSC)
- **WriteOp** enum: Insert, Update, Delete, DeleteEntity, CommitBatch with response channel
- **WriteQueue**: MPSC unbounded channel (tokio::sync::mpsc) connecting application threads to single write thread
- **Write Thread**: Dedicated thread/task that processes WriteOps sequentially, applies to write buffer, logs to WAL, and commits batches
- **Response Handling**: Send result/error back via oneshot channel

### 2.6 Memory Efficient Buffering
- **Buffer Growth Strategy**: Exponential growth with configurable initial capacity and factor
- **Memory Pool**: Reuse deallocated buffers to reduce allocation pressure
- **Compaction**: Periodically defragment buffers (remove gaps from deleted records)
- **Memory Usage Metrics**: Track allocated vs used bytes per table

### 2.7 Field Codec System
- **FieldEncoder/Decoder**: Convert Rust types to/from bytes with proper alignment
- **Custom Type Support**: User‑defined structs and enums via Serialize/Deserialize
- **Zero‑Copy Casting**: Unsafe but validated casting from byte slices to typed references
- **Alignment Verification**: Runtime check that field offsets satisfy type alignment

### 2.8 Enhanced Transaction Engine
- **Transaction Batching**: Group multiple operations into a single transaction for performance
- **Rollback Support**: Revert write‑buffer changes if transaction fails
- **Timeout Handling**: Abort transactions that take too long
- **Concurrent Transaction Limiting**: Prevent write‑thread queue overflow

### 2.9 Benchmarking Suite
- **Criterion Benchmarks**: Measure insert/update/delete latency and throughput
- **Memory Benchmarks**: Track allocations and buffer growth
- **Concurrency Benchmarks**: Stress test with multiple reader/writer threads
- **Compare Baseline**: SQLite in‑memory, Redis, other embedded stores

## Acceptance Criteria
1. Delta tracker captures all changes per transaction; delta size proportional to changed data only
2. Atomic commit swaps all buffers without blocking readers; version counter increments
3. Foreign key constraints are enforced; violations rollback transaction
4. Sparse component storage works; adding/removing components moves entities between archetypes
5. Write queue processes operations sequentially; multiple application threads can submit writes concurrently without locks
6. Memory usage grows smoothly; no unnecessary allocations during steady‑state operation
7. Field codec serializes/deserializes primitive types, arrays, and custom structs correctly
8. Benchmarks show >100k ops/sec for single‑table inserts, <10μs latency for reads

## Output Artifacts
- Delta tracking module with unit tests
- Atomic commit integration tests proving isolation
- Foreign key constraint validation tests
- Sparse storage implementation with archetype migration example
- Write queue stress test demonstrating lock‑free behavior
- Criterion benchmark results
- Updated `AGENTS.md` with benchmark commands

## Notes
- Focus on performance and correctness together
- Use `std::sync::atomic` with appropriate ordering (Acquire/Release)
- Profile memory usage with `valgrind --tool=massif`
- Ensure sparse storage does not degrade iteration performance
