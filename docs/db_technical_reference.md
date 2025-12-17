# In-Memory Relational Database - Technical Reference Document

## 1. Architecture Overview

### 1.1 Core Components

```
Database
├── SchemaMap: HashMap<String, Table>
├── TypeRegistry: HashMap<String, TypeLayout>
├── ProcedureRegistry: HashMap<String, ProcedureFn>
├── RuntimeLoop: tokio::task::JoinHandle
└── PersistenceHandle: mpsc::Sender<FlushCommand>

Table
├── name: String
├── record_size: usize
├── buffer: ArcSwap<Vec<u8>>
├── fields: Vec<Field>
├── relations: Vec<Relation>
└── next_id: AtomicU64

Field
├── name: String
├── offset: usize
├── type_id: String
└── layout: TypeLayout

TypeLayout
├── size: usize
├── align: usize
├── pod: bool
└ serializer: fn(*const u8, &mut Vec<u8>) -> usize
```

### 1.2 Memory Layout

```
Table Buffer (Vec<u8>)
[ Record 0 ][ Record 1 ][ Record 2 ]...
    ↓
[ Field0 ][ Field1 ][ Field2 ]...   // Tightly packed, no padding (unless align > 1)
```

**Cache Line Optimization**: Records aligned to 64-byte boundaries where beneficial; field ordering prioritizes access patterns.

## 2. Type System

### 2.1 Built-in Types

| Type ID  | Size (bytes) | Alignment | Serializer      |
| -------- | ------------ | --------- | --------------- |
| `i8`     | 1            | 1         | copy_1          |
| `i16`    | 2            | 2         | copy_2          |
| `i32`    | 4            | 4         | copy_4          |
| `i64`    | 8            | 8         | copy_8          |
| `u8`     | 1            | 1         | copy_1          |
| `u16`    | 2            | 2         | copy_2          |
| `u32`    | 4            | 4         | copy_4          |
| `u64`    | 8            | 8         | copy_8          |
| `f32`    | 4            | 4         | copy_4          |
| `f64`    | 8            | 8         | copy_8          |
| `bool`   | 1            | 1         | copy_bool       |
| `string` | dynamic      | 1         | len_prefix_utf8 |

### 2.2 Custom Type Registration

```rust
pub unsafe fn register_type(
    &self,
    id: String,
    size: usize,
    align: usize,
    pod: bool,
    serializer: unsafe fn(*const u8, &mut Vec<u8>) -> usize,
) -> Result<(), TypeError>
```

**Constraints**: `size % align == 0`; POD types must be `Copy + 'static`; serializer must be thread-safe.

### 2.3 Composite Type Example: `3xf32`

```
Layout:
- size: 12 bytes
- align: 4 bytes
- Fields: x@0, y@4, z@8
- Serializer: memcpy 12 bytes
```

## 3. Storage Engine

### 3.1 Record Addressing

```rust
// Given record index
let offset = index * table.record_size;
let ptr = buffer.as_ptr().add(offset);
let record_ptr = ptr as *const RecordStruct;
```

**SAFETY**: RecordStruct must be `#[repr(C, packed)]` and match field layout exactly.

### 3.2 Buffer Management

```rust
pub struct AtomicBuffer {
    inner: ArcSwap<Vec<u8>>,
    capacity: AtomicUsize,
}
```

**Operations**:

- **Read**: `inner.load()` → `Arc<Vec<u8>>` (zero-copy)
- **Write**: Clone buffer, modify, `inner.store(new_arc)` (atomic swap)

### 3.3 Capacity Management

- Initial capacity: `max(1024, expected_records) * record_size`
- Growth factor: `2x` on capacity exhaustion
- Reallocation triggers full buffer copy; amortized O(1) per operation

## 4. Concurrency Model

### 4.1 ArcSwap Semantics

- **Readers**: Acquire `Arc<Vec<u8>>` via `load()`; buffer immutable for lifetime of `Arc`
- **Writers**: Hold exclusive `Vec<u8>` clone; publish via `store()`
- **Epoch-based**: Old buffers dropped when last `Arc` is released; no blocking

### 4.2 CRUD Operation Flow

**READ**:

```rust
let buffer = table.buffer.load(); // Arc<Vec<u8>>
let ptr = buffer.as_ptr().add(offset);
unsafe { &*ptr.as_ref() }
```

**CREATE**:

```rust
let mut new_buffer = (*table.buffer.load_full()).clone();
new_buffer.extend_from_slice(&serialized_record);
table.buffer.store(Arc::new(new_buffer));
table.next_id.fetch_add(1, Ordering::SeqCst);
```

**UPDATE**:

```rust
let mut new_buffer = (*table.buffer.load_full()).clone();
let slice = &mut new_buffer[offset..offset + record_size];
slice.copy_from_slice(&new_data);
table.buffer.store(Arc::new(new_buffer));
```

**DELETE** (soft delete flag or compacting rewrite):

```rust
// Flag approach: set is_deleted: bool field
// Compact approach: rebuild buffer without deleted record
```

### 4.3 Transaction Isolation

- **Read Committed**: Each read sees latest committed buffer
- **No Phantom Reads**: Buffer immutability guarantees stable snapshot per `Arc`
- **Write Conflicts**: Last writer wins; no optimistic concurrency control

## 5. Transaction System

### 5.1 Implicit Single-Op Transactions

All CRUD ops default to:

1. Clone buffer
2. Modify clone
3. Atomic store
4. Return success

### 5.2 Explicit Multi-Op Transactions

```rust
pub struct Transaction {
    staging: HashMap<String, StagingBuffer>,
    committed: AtomicBool,
}

pub struct StagingBuffer {
    table_name: String,
    buffer: Vec<u8>,
    changes: Vec<Change>,
}

pub enum Change {
    Create { offset: usize, data: Vec<u8> },
    Update { offset: usize, old: Range<usize>, new: Vec<u8> },
    Delete { offset: usize },
}
```

**Commit**: Apply all staging buffers via atomic swaps; if any fails, discard all.

### 5.3 Procedure Transactions

Procedures receive `&TransactionHandle`; all writes go to isolated staging buffer. Final `tx.commit()` performs single atomic publish.

## 6. REST API Specification

### 6.1 Endpoints & Payloads

**Create Table**:

```http
POST /tables/{name}
Content-Type: application/json
{
  "fields": [
    {"name": "id", "type": "u64"},
    {"name": "name", "type": "string"},
    {"name": "position", "type": "3xf32"}
  ]
}
Response: 201 Created { "table": "name", "record_size": 24 }
```

**Add Field** (blocking rewrite):

```http
POST /tables/{name}/fields
{"name": "active", "type": "bool"}
Response: 200 OK { "offset": 24, "record_size": 25 }
```

**Create Record**:

```http
POST /tables/{name}/records
{"values": [123, "entity", [1.0, 2.0, 3.0]]}
Response: 201 Created { "id": 123 }
```

**Read Record** (returns raw bytes as base64):

```http
GET /tables/{name}/records/123
Response: 200 OK
Content-Type: application/octet-stream
<body: raw bytes>
```

**Update Record**:

```http
PUT /tables/{name}/records/123
{"values": [123, "entity_new", [4.0, 5.0, 6.0]]}
Response: 204 No Content
```

**Delete Record**:

```http
DELETE /tables/{name}/records/123
Response: 204 No Content
```

**RPC Procedure**:

```http
POST /rpc/bulk_update
{"table": "entities", "filter_field": "active", "filter_value": false, "set_field": "active", "set_value": true}
Response: 200 OK { "affected": 42 }
```

## 7. Runtime Loop

### 7.1 Tick Structure

```rust
pub struct Runtime {
    tickrate: u32,
    tick_duration: Duration,
    api_rx: mpsc::Receiver<ApiRequest>,
    procedure_queue: VecDeque<ProcedureCall>,
    persistence_tx: mpsc::Sender<FlushCommand>,
}

pub enum TickPhase {
    Api,        // 30% of tick
    Procedures, // 50% of tick
    Persistence,// 20% of tick
}
```

### 7.2 Tick Execution

```rust
loop {
    let tick_start = Instant::now();

    // Phase 1: API requests (bounded execution)
    while let Ok(req) = api_rx.try_recv() {
        if tick_start.elapsed() > tick_duration * 0.3 { break; }
        handle_api(req);
    }

    // Phase 2: Procedures (parallel execution)
    let procedures = drain_procedure_queue();
    if !procedures.is_empty() {
        let chunk_time = tick_duration * 0.5 / procedures.len() as u32;
        procedures.par_iter().for_each(|p| {
            run_procedure(p, chunk_time);
        });
    }

    // Phase 3: Persistence (fire and forget)
    if tick_count % persistence_interval == 0 {
        persistence_tx.send(FlushAll);
    }

    // Sleep remainder
    if let Some(remaining) = tick_duration.checked_sub(tick_start.elapsed()) {
        thread::sleep(remaining);
    }
}
```

### 7.3 Tickrate Configuration

- **15 Hz**: `tick_duration = 66.66ms` - for heavy procedures
- **60 Hz**: `tick_duration = 16.66ms` - balanced
- **120 Hz**: `tick_duration = 8.33ms` - low-latency API

## 8. Persistence

### 8.1 Schema File (`schema.json`)

```json
{
  "version": 1,
  "tables": {
    "entities": {
      "record_size": 25,
      "fields": [
        { "name": "id", "type": "u64", "offset": 0 },
        { "name": "name", "type": "string", "offset": 8 },
        { "name": "position", "type": "3xf32", "offset": 9 },
        { "name": "active", "type": "bool", "offset": 21 }
      ],
      "relations": [
        {
          "to_table": "components",
          "from_field": "id",
          "to_field": "entity_id"
        }
      ]
    }
  },
  "custom_types": {
    "3xf32": { "size": 12, "align": 4, "pod": true }
  }
}
```

**Write Policy**: Synchronous write on DDL; atomic rename from `.tmp` to `schema.json`.

### 8.2 Data Files (`data/{table_name}.bin`)

- Format: Raw binary dump of `Vec<u8>`
- Flush Strategy: Copy-on-write to temp file, atomic rename
- Frequency: Configurable (default: every 10 ticks)

### 8.3 Recovery

1. Load `schema.json`
2. For each table, `mmap` data file into `Vec<u8>`
3. Verify record count: `file_size / record_size`
4. Restore `next_id` from max id in data

## 9. Procedure System

### 9.1 Procedure Signature

```rust
pub type ProcedureFn = fn(
    db: &Database,
    tx: &TransactionHandle,
    params: serde_json::Value,
) -> Result<serde_json::Value, ProcedureError>;
```

### 9.2 Parallel Iteration Pattern

```rust
fn bulk_update(db: &Database, tx: &TransactionHandle, params: Value) -> Result<Value> {
    let table = db.get_table(params["table"].as_str().unwrap())?;
    let buffer = table.buffer.load();
    let record_size = table.record_size;

    // Shard across cores
    let affected = buffer.par_chunks_exact(record_size)
        .enumerate()
        .filter_map(|(idx, chunk)| {
            let record = unsafe { &*(chunk.as_ptr() as *const Record) };
            if matches_filter(record, &params) {
                let mut new_record = *record;
                apply_update(&mut new_record, &params);
                Some((idx, new_record))
            } else {
                None
            }
        })
        .map(|(idx, new_record)| {
            tx.stage_update(table.name.clone(), idx * record_size, new_record);
            1
        })
        .sum();

    tx.commit()?;
    Ok(json!({ "affected": affected }))
}
```

### 9.3 Transactional Execution

- Procedures run in isolation; changes not visible until commit
- If procedure panics, `Drop` for `TransactionHandle` discards staging buffers
- Commit performs atomic swaps for all modified tables

## 10. API Server

### 10.1 Stack

- **HTTP**: Hyper
- **Serialization**: Serde JSON
- **Async**: Tokio runtime (single-threaded per core)
- **Routing**: Matchit router

### 10.2 Request Flow

```
HTTP Request → Router → Handler → Database Operation → ArcSwap Load → Unsafe Cast → Response
                ↓
         Rate Limiting (per tick quota)
                ↓
         Request Queue (bounded, drop on overflow)
```

### 10.3 Rate Limiting

- Max requests per tick: `tickrate * 10`
- Queue size: `tickrate * 100`
- Overflow: Return `503 Service Unavailable`

## 11. Error Handling

### 11.1 Error Types

```rust
pub enum DbError {
    TableNotFound(String),
    FieldNotFound(String, String),
    TypeMismatch { expected: String, got: String },
    InvalidOffset { table: String, offset: usize, max: usize },
    TransactionConflict(String),
    SerializationError(String),
    ProcedurePanic(String),
}
```

### 11.2 Panic Policy

- Procedure panics are caught with `std::panic::catch_unwind`
- Buffer clones on write prevent corruption from panics
- API handlers wrap panics in `500 Internal Server Error`

## 12. Performance Guarantees

### 12.1 Latency Budget

| Operation           | p50   | p99   | p99.9 |
| ------------------- | ----- | ----- | ----- |
| Read                | 200ns | 500ns | 1μs   |
| Write               | 1μs   | 5μs   | 10μs  |
| Procedure (1M rows) | 10ms  | 15ms  | 25ms  |

### 12.2 Allocation Profile

- **Hot Path**: Zero allocations (read, write)
- **Cold Path**: One allocation per buffer clone (write)
- **API**: One allocation per request (JSON parsing)

### 12.3 Cache Metrics

- **Records per Cache Line**: `64 / record_size` (minimum 2)
- **Expected L1 Hit Rate**: > 98% for sequential scans
- **False Sharing**: Prevented by 64-byte alignment of buffers

## 13. Safety & Correctness

### 13.1 Unsafe Code Requirements

- All unsafe blocks must have `// SAFETY:` comment
- Pointer casts must verify:
  - Alignment: `offset % align == 0`
  - Bounds: `offset + size <= buffer.len()`
  - Lifetime: buffer `Arc` must outlive reference
- Miri testing required for all unsafe code

### 13.2 Determinism

- Procedure parallel iteration must be deterministic (use `enumerate()`)
- Transaction commit order: explicit sort by table name

### 13.3 Memory Leaks

- `ArcSwap` uses weak references for old buffers; dropped when no readers
- Transaction staging buffers dropped on abort

## 14. Configuration

```rust
pub struct DbConfig {
    pub tickrate: u32,
    pub persistence_interval_ticks: u32,
    pub max_api_requests_per_tick: u32,
    pub initial_table_capacity: usize,
    pub data_dir: PathBuf,
    pub procedure_thread_pool_size: usize, // 0 = num_cpus
}
```

**Defaults**:

- `tickrate: 60`
- `persistence_interval_ticks: 10`
- `max_api_requests_per_tick: 600`
- `initial_table_capacity: 1024`

## 15. Testing Requirements

### 15.1 Unit Tests

- Type layout calculation
- ArcSwap correctness
- Transaction isolation scenarios
- Serializer roundtrips

### 15.2 Integration Tests

- Concurrent read/write correctness (loom)
- Procedure parallel iteration determinism
- Persistence atomicity (power-loss simulation)
- Cache line contention (perf)

### 15.3 Benchmarks

- `criterion` benchmarks for CRUD
- Cache miss profiling with `perf`
- Scalability to 100M records
- Procedure throughput per core

## 16. Build & Deployment

### 16.1 Cargo Features

```toml
[features]
default = ["rest_api"]
rest_api = ["hyper", "tokio", "matchit"]
persist = ["memmap2"]
proc_macro = ["procedure-macros"]
```

### 16.2 Binary Artifacts

- `db_server`: REST API server
- `db_tool`: Schema migration, data inspection
- `db_bench`: Performance benchmarks

---

**Implementation Sequence**: TypeRegistry → AtomicBuffer → Table CRUD → Transactions → RuntimeLoop → REST API → Persistence → Parallel Procedures → Testing Suite
