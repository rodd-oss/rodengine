# ECS Database - Detailed Architecture & Implementation Guide

## 1. Core Architecture Overview

### 1.1 High-Level System Design

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Code                         │
│              (Game Engine, Systems, Queries)                │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Database Public API Layer                       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ db.table::<Transform>().insert(entity_id, transform) │   │
│  │ db.table::<Health>().update(entity_id, health_delta) │   │
│  │ db.transaction()?.delete_entity(entity_id)?.commit()? │   │
│  └──────────────────────────────────────────────────────┘   │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┴────────────┬──────────────────┐
        │                         │                  │
        ▼                         ▼                  ▼
   ┌─────────────┐     ┌──────────────────┐  ┌──────────────┐
   │ Write Queue │     │  Schema Manager  │  │ Replication  │
   │  (MPSC)     │     │  (TOML Parse)    │  │  Broadcaster │
   └──────┬──────┘     └──────────────────┘  └──────┬───────┘
          │                                          │
          ▼                                          │
   ┌──────────────────────────────────────────────┐ │
   │         Write Thread (Single)                │ │
   │  ┌────────────────────────────────────────┐ │ │
   │  │ Transaction Engine                     │ │ │
   │  │ • WAL Logger                           │ │ │
   │  │ • Delta Tracker                        │ │ │
   │  │ • Conflict Resolver                    │ │ │
   │  │ • Version Bumper                       │ │ │
   │  └────────────────────────────────────────┘ │ │
   └──────────────────────────────────────────────┘ │
          │                                          │
          ▼                                          │
   ┌──────────────────────────────────────────────┐ │
   │     Double Buffer Manager                    │ │
   │  ┌────────────────────────────────────────┐  │ │
   │  │ Table 1: Read Buffer (Arc<Vec<u8>>)    │  │ │
   │  │ Table 1: Write Buffer (Vec<u8>)        │  │ │
   │  │ Table 1: Delta Log (Vec<DeltaOp>)      │  │ │
   │  │ ...                                     │  │ │
   │  │ Table N: Read Buffer (Arc<Vec<u8>>)    │  │ │
   │  │ Table N: Write Buffer (Vec<u8>)        │  │ │
   │  │ Table N: Delta Log (Vec<DeltaOp>)      │  │ │
   │  └────────────────────────────────────────┘  │ │
   │                                               │ │
   │  Atomic Swap Point:                          │ │
   │  All read buffers updated simultaneously ────┼─┘
   │  via atomic pointer operations                │
   └──────────────────────────────────────────────┘
          │                                    │
          ▼                                    ▼
   ┌──────────────────────┐         ┌─────────────────┐
   │ Async I/O Workers    │         │ Network Thread  │
   │ (Tokio Tasks)        │         │ (Client Sync)   │
   │ • Snapshot Writer    │         │ • Protocol Enc. │
   │ • WAL Archiver       │         │ • Broadcast Que │
   │ • Compactor          │         └─────────────────┘
   └──────────────────────┘
```

### 1.2 Module Organization

```
ecsdb/
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── error.rs               # Error types and Result
│   ├── config.rs              # Configuration structures
│   │
│   ├── schema/
│   │   ├── mod.rs             # Schema public API
│   │   ├── parser.rs          # TOML schema parsing
│   │   ├── validator.rs       # Schema validation
│   │   ├── types.rs           # Type definitions
│   │   └── migrations.rs      # Schema versioning
│   │
│   ├── storage/
│   │   ├── mod.rs             # Storage public API
│   │   ├── buffer.rs          # Vec<u8> buffer management
│   │   ├── field_codec.rs     # Field serialization/casting
│   │   ├── layout.rs          # Memory layout calculation
│   │   └── sparse.rs          # Sparse component storage
│   │
│   ├── entity/
│   │   ├── mod.rs             # Entity public API
│   │   ├── registry.rs        # Entity ID generation
│   │   ├── version.rs         # Version tracking
│   │   └── archetype.rs       # Archetype management
│   │
│   ├── transaction/
│   │   ├── mod.rs             # Transaction public API
│   │   ├── engine.rs          # Transaction state machine
│   │   ├── wal.rs             # Write-ahead logging
│   │   ├── log.rs             # Operation log
│   │   └── state.rs           # Transaction state types
│   │
│   ├── replication/
│   │   ├── mod.rs             # Replication public API
│   │   ├── delta.rs           # Delta calculation
│   │   ├── encoder.rs         # Binary serialization
│   │   ├── protocol.rs        # Network protocol
│   │   └── broadcast.rs       # Delta broadcasting
│   │
│   ├── persistence/
│   │   ├── mod.rs             # Persistence public API
│   │   ├── snapshot.rs        # Snapshot creation/restore
│   │   ├── archive.rs         # WAL archive management
│   │   ├── compaction.rs      # Compaction worker
│   │   └── io.rs              # Async I/O utilities
│   │
│   ├── query/
│   │   ├── mod.rs             # Query API
│   │   ├── builder.rs         # Fluent query builder
│   │   ├── filter.rs          # Filter predicates
│   │   └── join.rs            # Multi-table joins
│   │
│   ├── index/
│   │   ├── mod.rs             # Index public API
│   │   ├── btree.rs           # B-tree index
│   │   ├── spatial.rs         # Spatial index (quadtree)
│   │   └── graph.rs           # Relation graph index
│   │
│   └── db.rs                  # Database handle (main API)
│
├── tests/
│   ├── integration_tests.rs   # End-to-end tests
│   ├── concurrency_tests.rs   # Lock-free tests
│   ├── replication_tests.rs   # Client sync tests
│   └── performance_tests.rs   # Benchmarks
│
├── benches/
│   ├── inserts.rs             # Insert benchmarks
│   ├── reads.rs               # Read benchmarks
│   ├── updates.rs             # Update benchmarks
│   └── replication.rs         # Replication benchmarks
│
├── examples/
│   ├── basic_usage.rs         # Simple example
│   ├── game_engine.rs         # Game integration example
│   ├── replication.rs         # Multi-client example
│   └── schema_editor.rs       # Schema creation example
│
└── Cargo.toml
```

---

## 2. Data Structure Details

### 2.1 Entity Table Structure

```rust
pub struct EntityRegistry {
    // Packed entity records: [id: u64][version: u32][archetype_hash: u64]
    entities: Vec<u8>,
    
    // Map entity_id → offset in entities buffer
    id_index: HashMap<u64, usize>,
    
    // Reused entity slots (id, next_version)
    freelist: Vec<(u64, u32)>,
    
    // Next entity ID to allocate
    next_id: u64,
}

// Entity Record Layout (16 bytes per entity):
// Offset 0:  u64 entity_id
// Offset 8:  u32 version
// Offset 12: u32 archetype_hash
```

### 2.2 Component Table Structure

```rust
pub struct ComponentTable<T> {
    // Packed component records
    data: Vec<u8>,
    
    // Map entity_id → offset in data buffer
    entity_index: HashMap<u64, usize>,
    
    // Field metadata (for unsafe casting)
    field_offsets: Vec<usize>,
    field_sizes: Vec<usize>,
    
    // Total bytes per record
    record_size: usize,
    
    // Number of components stored
    count: u64,
}

// Component Record Layout (Example for Transform):
// [entity_id: u64][x: f32][y: f32][z: f32][dirty_flag: u32]
//         0         8    12    16    20    24
```

### 2.3 Double Buffer Implementation

```rust
pub struct DoubleBuffer {
    // Read buffer (Arc allows sharing across threads)
    read_buffer: Arc<AtomicPtr<Vec<u8>>>,
    
    // Write buffer (only accessed by write thread)
    write_buffer: Vec<u8>,
    
    // Shadow read buffer (used after swap)
    shadow_buffer: Vec<u8>,
    
    // Delta operations recorded in this batch
    deltas: Vec<DeltaOp>,
    
    // Transaction version number
    version: u64,
}

pub enum DeltaOp {
    Insert { 
        table: TableId, 
        entity_id: u64, 
        data: Vec<u8> 
    },
    Update { 
        table: TableId, 
        entity_id: u64, 
        field_offset: usize, 
        data: Vec<u8> 
    },
    Delete { 
        table: TableId, 
        entity_id: u64 
    },
    CreateEntity { 
        entity_id: u64 
    },
    DeleteEntity { 
        entity_id: u64 
    },
}
```

### 2.4 Write Queue Architecture

```rust
// MPSC channel for lock-free write operations
pub struct WriteQueue {
    sender: mpsc::UnboundedSender<WriteOp>,
    receiver: mpsc::UnboundedReceiver<WriteOp>,
}

pub enum WriteOp {
    Insert {
        table_id: TableId,
        entity_id: u64,
        data: Vec<u8>,
        response: oneshot::Sender<Result<()>>,
    },
    Update {
        table_id: TableId,
        entity_id: u64,
        fields: Vec<(usize, Vec<u8>)>,
        response: oneshot::Sender<Result<()>>,
    },
    Delete {
        table_id: TableId,
        entity_id: u64,
        response: oneshot::Sender<Result<()>>,
    },
    DeleteEntity {
        entity_id: u64,
        response: oneshot::Sender<Result<()>>,
    },
    CommitBatch {
        response: oneshot::Sender<Result<u64>>, // Returns version
    },
}
```

### 2.5 Schema TOML Structure

```toml
[database]
name = "game_world"
version = "1.0.0"

# Entity table (auto-generated)
[tables.entities]
description = "Central entity registry"

[[tables.entities.fields]]
name = "id"
type = "u64"
primary_key = true

[[tables.entities.fields]]
name = "version"
type = "u32"

# Component table
[tables.transform]
description = "Position and rotation"
parent_table = "entities"

[[tables.transform.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.transform.fields]]
name = "position"
type = "vec3"  # Custom type: [f32; 3]

[[tables.transform.fields]]
name = "rotation"
type = "quat"  # Custom type: [f32; 4]

[[tables.transform.fields]]
name = "dirty"
type = "bool"

[tables.health]
description = "Entity health system"
parent_table = "entities"

[[tables.health.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.health.fields]]
name = "hp"
type = "u32"

[[tables.health.fields]]
name = "max_hp"
type = "u32"

# Enum type definition
[enums.faction]
variants = ["neutral", "player", "enemy", "npc"]

# Custom type definition
[custom_types.vec3]
fields = [
    { name = "x", type = "f32" },
    { name = "y", type = "f32" },
    { name = "z", type = "f32" }
]
```

---

## 3. Algorithm Details

### 3.1 Zero-Copy Field Access

```rust
impl<T: Sized> ComponentTable<T> {
    /// Unsafe but zero-copy field access
    /// User must ensure lifetime 'a is valid only while
    /// the underlying buffer is not modified
    pub unsafe fn get_field<'a, F>(
        &'a self,
        entity_id: u64,
        field_offset: usize,
    ) -> Option<&'a F>
    where
        F: Sized,
    {
        let record_offset = self.entity_index.get(&entity_id)?;
        let field_ptr = self.data.as_ptr()
            .add(*record_offset + field_offset) as *const F;
        
        // Safety check: ensure field is aligned
        if field_ptr as usize % std::mem::align_of::<F>() != 0 {
            return None; // Alignment violation
        }
        
        Some(&*field_ptr)
    }
    
    /// Safe wrapper using lifetime bounds
    pub fn get_field_safe<'a, F>(
        &'a self,
        entity_id: u64,
        field_offset: usize,
    ) -> Option<&'a F>
    where
        F: Sized,
    {
        unsafe { self.get_field(entity_id, field_offset) }
    }
}
```

### 3.2 Atomic Double Buffer Swap

```rust
pub struct BufferManager {
    tables: HashMap<TableId, DoubleBuffer>,
    version: Arc<AtomicU64>,
}

impl BufferManager {
    /// Atomically swap all read/write buffers
    /// All readers see change simultaneously
    pub fn commit_batch(&mut self) -> Result<u64> {
        let new_version = self.version.fetch_add(1, 
            std::sync::atomic::Ordering::Release);
        
        for (_, double_buf) in self.tables.iter_mut() {
            // Clone write buffer to shadow
            double_buf.shadow_buffer = double_buf.write_buffer.clone();
            
            // Atomic swap: shadow becomes new read buffer
            // (In practice, use Arc<AtomicPtr<Vec<u8>>> for true lock-free swap)
            unsafe {
                let read_ptr = double_buf.read_buffer.load(
                    std::sync::atomic::Ordering::Acquire);
                let shadow_ptr = &mut double_buf.shadow_buffer as *mut Vec<u8>;
                
                double_buf.read_buffer.store(shadow_ptr as *mut Vec<u8>,
                    std::sync::atomic::Ordering::Release);
            }
        }
        
        self.broadcast_deltas(new_version)?;
        
        Ok(new_version)
    }
}

// Thread-safe reference to read buffer
pub struct BufferSnapshot {
    data: Arc<Vec<u8>>,
    version: u64,
}
```

### 3.3 Delta Calculation Algorithm

```rust
pub fn calculate_deltas(
    old_buffer: &[u8],
    new_buffer: &[u8],
    record_size: usize,
) -> Vec<DeltaOp> {
    let mut deltas = Vec::new();
    
    // Compare records at chunk level
    let old_records = old_buffer.chunks_exact(record_size);
    let new_records = new_buffer.chunks_exact(record_size);
    
    for (idx, (old_rec, new_rec)) in 
        old_records.zip(new_records).enumerate() 
    {
        // Find differing fields using word-level comparison
        for (old_word, new_word) in 
            old_rec.chunks(8).zip(new_rec.chunks(8)) 
        {
            if old_word != new_word {
                // Found delta, record only the changed bytes
                deltas.push(DeltaOp::Update {
                    table: TableId(0),
                    entity_id: idx as u64,
                    field_offset: 0,
                    data: new_word.to_vec(),
                });
            }
        }
    }
    
    deltas
}
```

### 3.4 Write-Ahead Log (WAL) Format

```rust
pub struct WALEntry {
    // Timestamp for recovery ordering
    timestamp: u64,
    
    // Transaction ID for grouping
    txn_id: u64,
    
    // Operation type
    op: WriteOp,
    
    // Checksum for corruption detection
    checksum: u32,
}

impl WALEntry {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Header: [magic: 4][version: 2][flags: 2]
        buf.extend_from_slice(b"WALE");
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        
        // Metadata
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.txn_id.to_le_bytes());
        
        // Operation (variable length)
        let op_bytes = bincode::serialize(&self.op).unwrap();
        buf.extend_from_slice(&(op_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&op_bytes);
        
        // Checksum (CRC32)
        let checksum = crc32(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());
        
        buf
    }
}
```

---

## 4. Concurrency Model Details

### 4.1 Lock-Free Transaction Processing

```rust
pub struct Database {
    // Single-writer, multi-reader pattern
    write_queue: mpsc::UnboundedSender<WriteOp>,
    write_thread: JoinHandle<()>,
    
    // Atomic version tracking
    version: Arc<AtomicU64>,
    
    // Readable buffers (Arc for cheap cloning)
    buffers: Arc<RwLock<BufferManager>>,
    
    // Schema (immutable after startup)
    schema: Arc<Schema>,
}

impl Database {
    pub async fn insert<T: Component>(
        &self,
        entity_id: u64,
        component: T,
    ) -> Result<()> {
        // 1. Serialize to bytes (cheap, no lock)
        let data = T::encode(&component)?;
        
        // 2. Queue write operation (cheap, lock-free)
        let (tx, rx) = oneshot::channel();
        self.write_queue.send(WriteOp::Insert {
            table_id: T::TABLE_ID,
            entity_id,
            data,
            response: tx,
        })?;
        
        // 3. Wait for write thread to process (application blocks here)
        //    but no lock held in database
        rx.await??;
        
        Ok(())
    }
}
```

### 4.2 Write Thread Main Loop

```rust
async fn write_thread_main(
    mut rx: mpsc::UnboundedReceiver<WriteOp>,
    mut buffer_mgr: BufferManager,
    mut wal: WALWriter,
) {
    let mut batch = Vec::new();
    let mut batch_timer = Instant::now();
    const BATCH_TIMEOUT_MS: u64 = 10;
    const BATCH_SIZE: usize = 1000;
    
    loop {
        // Receive with timeout to handle batching
        match tokio::time::timeout(
            Duration::from_millis(BATCH_TIMEOUT_MS),
            rx.recv()
        ).await {
            Ok(Some(op)) => {
                batch.push(op);
                
                // Flush on batch size or timeout
                if batch.len() >= BATCH_SIZE 
                    || batch_timer.elapsed().as_millis() > BATCH_TIMEOUT_MS as u128 
                {
                    flush_batch(&mut batch, &mut buffer_mgr, &mut wal)?;
                    batch_timer = Instant::now();
                }
            }
            Ok(None) => break, // Channel closed
            Err(_) => {
                // Timeout: flush accumulated batch
                if !batch.is_empty() {
                    flush_batch(&mut batch, &mut buffer_mgr, &mut wal)?;
                    batch_timer = Instant::now();
                }
            }
        }
    }
}

fn flush_batch(
    batch: &mut Vec<WriteOp>,
    buffer_mgr: &mut BufferManager,
    wal: &mut WALWriter,
) -> Result<()> {
    let txn_id = generate_txn_id();
    
    // Step 1: Log to WAL (synchronous for durability)
    for op in batch.iter() {
        let entry = WALEntry {
            timestamp: current_timestamp(),
            txn_id,
            op: op.clone(),
            checksum: 0,
        };
        wal.append(entry)?;
    }
    
    // Step 2: Apply to write buffers (in-memory, fast)
    for op in batch.iter() {
        match op {
            WriteOp::Insert { table_id, entity_id, data, .. } => {
                buffer_mgr.insert(*table_id, *entity_id, data)?;
            }
            // ... other operations
        }
    }
    
    // Step 3: Atomic commit (all tables swapped simultaneously)
    let version = buffer_mgr.commit_batch()?;
    
    // Step 4: Send responses (these are cheap)
    for op in batch.drain(..) {
        if let WriteOp::Insert { response, .. } = op {
            let _ = response.send(Ok(()));
        }
    }
    
    // Step 5: Queue async disk write (non-blocking)
    tokio::spawn(async move {
        if let Err(e) = wal.flush_to_disk().await {
            eprintln!("WAL disk write failed: {}", e);
        }
    });
    
    Ok(())
}
```

### 4.3 Read-Side Concurrency

```rust
impl Database {
    pub fn read<T: Component, F, R>(
        &self,
        entity_id: u64,
        f: F,
    ) -> Result<R>
    where
        F: FnOnce(&T) -> Result<R>,
    {
        // 1. Acquire read lock (shared, non-blocking if write thread not holding)
        let buffers = self.buffers.read().unwrap();
        
        // 2. Get snapshot of current buffer
        let table = buffers.get_table::<T>()?;
        
        // 3. Zero-copy field access
        unsafe {
            let component = table.get_field::<T>(entity_id, 0)?;
            
            // 4. User function operates on borrowed reference
            f(component)
        }
        
        // 5. Drop snapshot, release read lock
        // Multiple readers can be here simultaneously!
    }
    
    /// Bulk read operation (high-throughput path)
    pub fn scan<T: Component, F>(
        &self,
        mut f: F,
    ) -> Result<()>
    where
        F: FnMut(u64, &T) -> Result<()>,
    {
        let buffers = self.buffers.read().unwrap();
        let table = buffers.get_table::<T>()?;
        
        // Iterate buffer sequentially (cache-friendly!)
        for (entity_id, offset) in table.entity_index.iter() {
            let component = unsafe {
                &*(table.data.as_ptr().add(*offset) as *const T)
            };
            f(*entity_id, component)?;
        }
        
        Ok(())
    }
}
```

---

## 5. Replication Protocol

### 5.1 Delta Sync Protocol

```
[Client Connect]
    ↓
[Server: Accept Connection]
    ↓
[Server → Client: Initial Sync Message]
    • Schema definition (TOML)
    • Full database snapshot (all tables)
    • Current version number
    ↓
[Client: Load Schema & Apply Snapshot]
    ↓
[Client → Server: Ready Message + Version]
    ↓
[Server: Add Client to Broadcast List]
    ↓
[Server: On Each Commit]
    • Serialize delta batch
    • Compress (optional)
    • Broadcast to all connected clients
    ↓
[Client: Receive Delta]
    ↓
[Client: Apply Delta to Local Buffer]
    ↓
[Client → Server: ACK with Version]
    ↓
[Server: Remove from Retransmit Queue]
```

### 5.2 Delta Encoding Format

```
Binary Layout:
┌─────────────────────────────────────────────────┐
│ Frame Header (8 bytes)                          │
│ ├─ Magic: "DLTA" (4)                           │
│ ├─ Version: u16 (2)                            │
│ └─ Flags: u16 (2)                              │
├─────────────────────────────────────────────────┤
│ Metadata (16 bytes)                             │
│ ├─ DB Version: u64 (8)                         │
│ ├─ Timestamp: u64 (8)                          │
├─────────────────────────────────────────────────┤
│ Delta Count: u32 (4)                            │
├─────────────────────────────────────────────────┤
│ Delta Operations (variable length)              │
│ For each delta:                                 │
│ ├─ Op Type: u8 (1)                             │
│ ├─ Table ID: u16 (2)                           │
│ ├─ Entity ID: u64 (8)                          │
│ ├─ Data Length: u32 (4)                        │
│ └─ Data: [u8] (variable)                       │
├─────────────────────────────────────────────────┤
│ Checksum: u32 (4)                               │
└─────────────────────────────────────────────────┘
```

### 5.3 Conflict Resolution

```rust
pub enum ConflictResolution {
    /// Server always wins (authoritative)
    ServerAuthoritative,
    
    /// Last-write-wins with timestamp
    LastWriteWins,
    
    /// Client-side merge function
    Custom(fn(ServerValue, ClientValue) -> MergeResult),
}

pub struct ServerValue {
    version: u64,
    timestamp: u64,
    data: Vec<u8>,
}

pub struct ClientValue {
    local_version: u64,
    timestamp: u64,
    data: Vec<u8>,
}

pub enum MergeResult {
    UseServer,
    UseClient,
    Merge(Vec<u8>),
}
```

---

## 6. Implementation Priorities

### Phase 1: Core Foundation (Critical Path)
1. **Schema system** - TOML parsing and validation
2. **Entity registry** - ID generation and versioning  
3. **Storage layer** - Vec<u8> buffers and field codecs
4. **Basic CRUD** - Insert/update/delete for single operations
5. **Testing** - Unit tests for each module

### Phase 2: Advanced Storage (Performance)
1. **Double buffer** - Read/write separation
2. **Transaction engine** - Atomic commits
3. **WAL** - Write-ahead logging for recovery
4. **Lock-free queue** - MPSC channel for writes
5. **Benchmarks** - Criterion benchmarks

### Phase 3: Production Ready (Reliability)
1. **Error handling** - Comprehensive error types
2. **Recovery** - WAL replay on startup
3. **Snapshot/restore** - Point-in-time recovery
4. **Async I/O** - Non-blocking disk operations
5. **Integration tests** - End-to-end scenarios

### Phase 4: Advanced Features (Scaling)
1. **Replication protocol** - Multi-client sync
2. **Delta encoding** - Efficient serialization
3. **Indices** - Query optimization
4. **Query API** - Type-safe queries
5. **Distributed tests** - Multi-node scenarios

### Phase 5: User Experience (Polish)
1. **Dashboard** - Tauri + Vue 3 UI
2. **Documentation** - API docs and guide
3. **Examples** - Game engine integration
4. **Performance tuning** - Profile and optimize
5. **Release packaging** - crates.io, binary releases

---

## 7. Key Implementation Decisions

### 7.1 Why Lock-Free?
- **No mutex overhead**: Mutexes serialize all operations, killing concurrency
- **Predictable latency**: No chance of priority inversion or lock contention
- **Scalability**: Performance doesn't degrade with more cores
- **Alternative**: RwLock for schema reads (rarely written)

### 7.2 Why Double Buffer?
- **Isolation**: Readers never see partial writes
- **No stop-the-world pauses**: Commit is atomic pointer swap
- **Efficient deltas**: Only changed data in write buffer
- **Alternative**: Copy-on-write for fine-grained updates (slower)

### 7.3 Why MPSC for Writes?
- **Single writer principle**: No concurrent write conflicts
- **Deterministic ordering**: Operations processed in FIFO order
- **Backpressure**: Sender blocks if buffer fills (flow control)
- **Alternative**: Multiple writers with serialization (adds complexity)

### 7.4 Why Vec<u8> Storage?
- **Cache friendly**: Contiguous memory, predictable layout
- **Zero-copy casting**: Direct pointer arithmetic
- **Compact**: No per-field metadata overhead
- **Alternative**: Custom allocator (higher complexity, marginal gains)

### 7.5 Why TOML Schema?
- **Human readable**: Developers can hand-edit if needed
- **Tooling**: Existing parsers and editors
- **Versioning**: Easy to diff and track changes
- **Alternative**: protobuf/flatbuffers (less ergonomic)

---

## 8. Performance Optimization Techniques

### 8.1 Reducing Allocations
- Preallocate buffers based on expected entity count
- Use object pools for delta operations
- Reuse serialization buffers across transactions
- Benchmark with `valgrind --tool=massif`

### 8.2 Cache Efficiency
- Pack related fields together in record
- Store archetype entities in same buffer region
- Prefetch entity indices during iteration
- Use memory layout visualization tools

### 8.3 Minimizing CPU Usage
- Batch multiple operations before commit
- Use intrinsics for bulk copying (SIMD where possible)
- Reduce syscalls via buffered disk I/O
- Profile with `perf` to identify hotspots

### 8.4 Network Efficiency
- Compress deltas with zstd for large changes
- Bundle multiple deltas into single packet
- Implement flow control to prevent buffer bloat
- Measure bandwidth with network simulation tools

---

## 9. Testing Strategy

### 9.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_field_access() {
        // Verify no extra allocations during read
    }

    #[test]
    fn test_atomic_commit() {
        // Verify all readers see committed state
    }

    #[test]
    fn test_lock_free_writes() {
        // Verify MPSC doesn't deadlock
    }
}
```

### 9.2 Concurrency Tests
- Stress test with 100+ threads reading/writing
- Verify memory safety with MIRI (Rust interpreter)
- Use ThreadSanitizer to detect data races
- Test buffer swap atomicity

### 9.3 Integration Tests
- Full CRUD cycle with constraints
- Transaction rollback on error
- Multi-client replication sync
- Recovery from crash simulation

### 9.4 Benchmarks
- Criterion benchmarks for all hot paths
- Compare against Redis, SQLite for baseline
- Profile memory usage and allocations
- Measure replication latency

---

## 10. Deployment & Distribution

### 10.1 As Library (crate)
```toml
[dependencies]
ecsdb = "0.1.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

### 10.2 As Embedded Database
```rust
let db = Database::new()
    .schema_file("schema.toml")?
    .enable_replication(8080)?
    .build()?;

// Use db in your game engine
```

### 10.3 Dashboard Distribution
- Distribute as Tauri desktop app
- Support Windows, macOS, Linux
- Electron alternative for web version
- Docker container for server deployment

---

## 11. Migration Path

### From Existing Systems
- **From SQLite**: Bulk export, transform schema, import via API
- **From Redis**: Migrate hash structures to components
- **From Custom ECS**: Rewrite query loops to use database API
- **Zero downtime**: Run in parallel, gradually migrate tables

### Schema Evolution
- Add nullable fields (backward compatible)
- Deprecate fields without removal (preserve data)
- Versioned migrations (script schema changes)
- Online schema updates (don't block readers)
