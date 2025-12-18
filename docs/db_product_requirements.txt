# In-Memory Relational Database PRD

## 1. Core Architecture

- **Language**: Rust
- **Storage Backend**: Single `Vec<u8>` per table containing tightly packed records
- **Memory Model**: Zero-copy, unsafe pointer casting from `Vec<u8>` slices to user-defined field structs
- **Cache Layout**: Fields within a record are contiguous; records within a table are contiguous
- **Concurrency**: Lock-free reads/writes via `arc-swap` on table buffers; no mutex locks in CRUD path

## 2. Data Model

- **Table**: Named collection of records with fixed schema
- **Field**: Typed column defined by name and data type
- **Relation**: Explicit foreign key connections between tables (all tables relation-capable)
- **Record**: Tightly packed byte sequence representing one row; accessed via unsafe cast to struct

## 3. Type System

- **Built-in Types**: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`, `String` (inline length-prefixed)
- **Custom Types**: Extensible composite types (e.g., `Vector3f32` as `3xf32`)
- **Type Registration**: Custom types must be `Pod` (Plain Old Data) and registerable at runtime

## 4. Storage Engine

- **Buffer Structure**: `ArcSwap<Vec<u8>>` per table for lock-free buffer swapping
- **Allocation Strategy**: Pre-allocate capacity; amortized growth with chunk doubling
- **Record Addressing**: Direct byte offset calculation: `offset = record_index * record_size`
- **Zero-Copy Guarantee**: All reads return `*const T` pointers into the live buffer; writes append to staging buffer

## 5. REST API Endpoints

```
POST   /tables/{name}              // Create table
DELETE /tables/{name}              // Delete table
POST   /tables/{name}/fields       // Add field: {name, type}
DELETE /tables/{name}/fields/{f}   // Remove field
POST   /relations                  // Create relation: {from_table, from_field, to_table, to_field}
DELETE /relations/{id}             // Delete relation

POST   /tables/{name}/records      // Create record: {field_values}
GET    /tables/{name}/records/{id} // Read record (returns raw bytes)
PUT    /tables/{name}/records/{id} // Update record (full replacement)
PATCH  /tables/{name}/records/{id} // Partial update
DELETE /tables/{name}/records/{id} // Delete record

POST   /rpc/{procedure_name}       // Execute procedure with JSON params
```

## 6. Transaction System

- **Atomicity**: All CRUD operations atomic by default via copy-on-write semantics
- **Transaction Model**: Implicit transactions per operation; explicit multi-op transactions via staging buffer
- **Commit**: Atomic pointer swap on `ArcSwap` at transaction end
- **Isolation**: Read-your-writes consistency; no phantom reads due to buffer immutability
- **Procedure Transactions**: Procedures run in isolated staging buffer; commit via single swap at completion

## 7. Concurrency & Parallelism

- **Reads**: Lock-free, operate on immutable `Arc` buffer from `arc-swap`
- **Writes**: Append to private staging buffer; publish atomically
- **Parallel Procedures**: Use Rayon or custom thread pool to shard table data across all CPU cores
- **Cache Maximization**: Procedure iterations must access contiguous memory regions per core

## 8. Runtime Loop

- **Tickrate**: Configurable 15 Hz to 120 Hz
- **Loop Responsibilities**:
  - Process API request queue
  - Execute pending procedures
  - Trigger async disk persistence
  - Handle schema updates
- **Tick Budget**: Each tick must complete within `1/tickrate` seconds; procedures may span ticks

## 9. Persistence

- **Schema**: Synchronously written to `schema.json` on any DDL operation
- **Data**: Asynchronously flushed to disk in background thread:
  - Write staging buffer to temp file
  - Atomic rename to replace live file
  - Format: Raw binary dump of `Vec<u8>` per table
- **Restore**: On startup, load schema from JSON, then memory-map data files into `Vec<u8>`

## 10. Procedure Execution

- **Definition**: Rust functions registered via `register_procedure(name, fn_ptr)`
- **Execution Context**: Receives `&Table`, transaction handle, and JSON params
- **Parallel Iteration**: Procedures can spawn per-core tasks using `table.chunks_exact(record_size)` across available cores
- **Transactional Guarantee**: All changes isolated in staging buffer; either full commit or discard

## 11. Performance Constraints

- **CRUD Latency**: < 1 microsecond for read, < 5 microseconds for write (single record)
- **Throughput**: 10M+ reads/sec/core, 1M+ writes/sec/core (contended)
- **Memory Overhead**: < 5% beyond raw data size
- **Allocation Count**: Zero allocations in hot path (reads, writes)

## 12. Safety & Limitations

- **Unsafe Code**: Required for pointer casting; must pass Miri tests and have explicit `SAFETY` comments
- **No SQL**: No query parser, optimizer, or ad-hoc filtering; all logic in procedures
- **Schema Migrations**: Adding/removing fields requires full table rewrite; blocking operation

## 13. Deliverables

- Library crate: `in_mem_db`
- Binary crate: `db_server` with REST API
- Example procedures demonstrating parallel iteration
- Benchmark suite: cache-miss profiling, contention tests
- `schema.json` format documentation

---

**Implementation Order**: Storage engine → Type system → Arc-swap concurrency → REST API → Transactions → Runtime loop → Persistence → Parallel procedures
