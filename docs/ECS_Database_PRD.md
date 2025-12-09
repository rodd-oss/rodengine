# ECS Relational Database - Product Requirements Document (PRD)

## 1. Executive Summary

Build a high-performance, lock-free relational database engine optimized for ECS (Entity Component System) game engines in Rust. The system prioritizes zero-copy data access, CPU cache efficiency, atomic transactions, and seamless client synchronization through delta-based replication.

---

## 2. Product Vision

**Title:** ECSDb - High-Performance Entity Component System Database

**Mission:** Provide game developers with a fast, developer-friendly database that understands ECS architecture natively, eliminating impedance mismatch between database operations and game engine requirements.

**Key Differentiators:**
- Lock-free atomic operations without mutex overhead
- Double-buffer architecture with delta-only write propagation
- Zero-copy field access via unsafe casting
- Native multi-client replication
- Embedded crate deployment with optional web dashboard

---

## 3. Core Requirements

### 3.1 Data Model

#### 3.1.1 Schema Structure
- **Entities Table**: Central registry of all game entities with unique IDs and versions
- **Component Tables**: One table per component type, storing dense arrays of component data
- **Relation Definitions**: Foreign key constraints linking components to entities
- **Metadata Tables**: Schema definitions, field type information, and cardinality hints

#### 3.1.2 Storage Format
- **Row Format**: Vec<u8> buffer with fixed-width field packing
- **Field Alignment**: Fields packed contiguously without padding waste
- **Memory Layout**: Column-oriented for cache efficiency within related entity groups
- **Casting Strategy**: User-defined fields cast via unsafe pointer operations after bounds checking

#### 3.1.3 Relations Support
- **One-to-Many**: Entity → Multiple Components
- **Many-to-One**: Components → Single Entity
- **Optional Relations**: Sparse component storage for entities without certain components
- **Relation Integrity**: Referential constraints enforced at transaction boundaries

### 3.2 Transactional Guarantees

#### 3.2.1 ACID Properties
- **Atomicity**: All-or-nothing for CRUD operations using write-ahead logging
- **Consistency**: Schema validation and referential integrity checks pre-commit
- **Isolation**: Double-buffer model isolates readers from writers
- **Durability**: Parallel disk persistence with snapshot mechanism

#### 3.2.2 Transaction Model
- **Default Behavior**: All operations wrapped in implicit transactions
- **Explicit Transactions**: Multi-operation grouping with rollback capability
- **No Locks**: Lock-free via atomic operations and message passing
- **Conflict Resolution**: Last-write-wins with vector clock tracking for distributed writes

### 3.3 Memory Architecture

#### 3.3.1 Double Buffer System
- **Read Buffer**: Current committed state, accessible to all readers
- **Write Buffer**: In-progress changes, atomic swap on transaction commit
- **Delta Storage**: Write buffer tracks only changed rows/fields (not full table copy)
- **Copy-on-Write Semantics**: Shared immutable snapshots avoid duplication

#### 3.3.2 Zero-Copy Access
- **Direct Casting**: User types cast from raw buffer pointers post-validation
- **Lifetime Management**: Borrowed references tied to buffer lifetime
- **Type Safety**: Compile-time field offset validation where possible
- **Alignment Guarantees**: Unsafe code verified for correct memory access

#### 3.3.3 Memory Efficiency
- **Dense Packing**: No field padding; custom alignment handling
- **Sparse Components**: Entities missing components use bitmap indices
- **Allocation Minimization**: Pre-allocated buffer growth with exponential scaling
- **Reuse Pools**: Deleted entity IDs recycled with version counters

### 3.4 Concurrency Model

#### 3.4.1 Lock-Free Design
- **Atomic Swaps**: Readers switch buffers via atomic pointer swap
- **Message Passing**: Write operations queue through MPSC channel
- **Single Writer**: Dedicated write thread processes transaction log sequentially
- **Memory Ordering**: Acquire/Release semantics for cross-thread visibility

#### 3.4.2 Transaction Processing
- **Write-Ahead Logging (WAL)**: Operations logged before buffer modification
- **Batch Processing**: Accumulated writes committed in configurable intervals
- **Atomic Commit**: All-or-nothing buffer swap with version bump
- **Rollback Capability**: WAL replay from checkpoint on failure

### 3.5 Replication & Synchronization

#### 3.5.1 Client Replication
- **Delta-Based Sync**: Only changed rows/fields transmitted to clients
- **Full Initial Sync**: Complete dataset on first client connection
- **Incremental Updates**: Efficient patch application for remote changes
- **Conflict Resolution**: Server-authoritative with timestamp/version tracking

#### 3.5.2 Network Protocol
- **Binary Serialization**: Compact binary format for over-wire transmission
- **Compression**: Optional zstd compression for large deltas
- **Batching**: Multiple deltas grouped into single network frame
- **Acknowledgment**: Client ACK prevents redundant retransmission

#### 3.5.3 Consistency Model
- **Eventual Consistency**: Clients eventually catch up to server state
- **Causal Ordering**: Related operations maintain dependency order
- **Broadcast Ordering**: Updates from single session strictly ordered
- **Idempotent Operations**: Safe replay of duplicate delta packets

### 3.6 Persistence

#### 3.6.1 Disk Storage
- **Async I/O**: Non-blocking writes using Tokio background tasks
- **Snapshot Format**: Point-in-time database dumps for quick recovery
- **WAL Archival**: Transaction logs preserved for point-in-time recovery
- **Compaction**: Periodic merging of snapshots and WAL

#### 3.6.2 Schema Persistence
- **TOML Format**: Human-readable schema definitions
- **Version Tracking**: Schema evolution with migration tracking
- **Metadata Storage**: Table structure, field types, constraints
- **Embedded Configuration**: Schema compiled into binary or loaded at runtime

---

## 4. Feature Set

### 4.1 Core Features (MVP)

| Feature | Description | Priority |
|---------|-------------|----------|
| **Entity Management** | Create, read, update, delete entities with version tracking | P0 |
| **Component Storage** | Dense component tables with zero-copy access | P0 |
| **Entity-Component Relations** | Foreign key references maintaining referential integrity | P0 |
| **Atomic Transactions** | Lock-free ACID transactions with WAL | P0 |
| **Double Buffering** | Read/write buffer separation with atomic swaps | P0 |
| **Delta Tracking** | Efficient change log for replication | P0 |
| **In-Memory Operation** | Full database in RAM with zero serialization overhead | P0 |
| **Disk Persistence** | Asynchronous snapshot and WAL writing | P0 |
| **TOML Schema** | Schema definition and migration in TOML format | P0 |
| **Rust API** | Type-safe, ergonomic database operations | P0 |

### 4.2 Advanced Features (Post-MVP)

| Feature | Description | Priority |
|---------|-------------|----------|
| **Client Replication** | Multi-client delta sync via network | P1 |
| **Web Dashboard** | Tauri 2 + Vue 3 schema editor and data viewer | P1 |
| **Batch Operations** | Bulk insert/update/delete with transaction grouping | P1 |
| **Query Optimization** | Spatial indices, relation graph indexing | P2 |
| **Hot Reload** | Schema changes without full database restart | P2 |
| **Backup/Restore** | Point-in-time recovery from snapshots | P2 |
| **Metrics Collection** | Performance monitoring and latency tracking | P2 |
| **Event Streaming** | Pub/sub for component changes | P2 |

---

## 5. Technical Architecture

### 5.1 System Overview

```
┌─────────────────────────────────────────────────────┐
│         Application Layer (Game Engine)             │
├─────────────────────────────────────────────────────┤
│              Rust Database API                      │
├─────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────┐   │
│  │     Transaction Engine (Single Writer)      │   │
│  │  ├─ WAL Logger                              │   │
│  │  ├─ Delta Tracker                           │   │
│  │  ├─ Conflict Resolver                       │   │
│  │  └─ Replication Broadcaster                 │   │
│  └─────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────┐   │
│  │      Double Buffer Storage Layer            │   │
│  │  ┌──────────────────────────────────────┐   │   │
│  │  │ Active Read Buffer (Vec<u8> per tbl) │   │   │
│  │  └──────────────────────────────────────┘   │   │
│  │  ┌──────────────────────────────────────┐   │   │
│  │  │ Write Buffer (Delta changes)         │   │   │
│  │  └──────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────┐   │
│  │    Persistence Layer (Async I/O)            │   │
│  │  ├─ Snapshot Manager                        │   │
│  │  ├─ WAL Archive                             │   │
│  │  └─ Compaction Worker                       │   │
│  └─────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────┤
│  Schema Manager (TOML Parser & Validator)          │
├─────────────────────────────────────────────────────┤
│  Network Layer (Optional: Client Replication)      │
├─────────────────────────────────────────────────────┤
│  Dashboard UI (Optional: Tauri + Vue 3)            │
└─────────────────────────────────────────────────────┘
```

### 5.2 Component Architecture

#### 5.2.1 Core Modules

**`core::schema`**
- Schema parsing and validation
- TOML serialization/deserialization
- Field type system and constraints
- Migration tracking

**`core::storage`**
- Vec<u8> buffer management
- Memory layout calculation
- Field offset computation
- Alignment guarantee verification

**`core::entity`**
- Entity ID generation and versioning
- Entity registry table
- Component archetype tracking
- Sparse set membership

**`core::transaction`**
- Transaction state machine
- Write-ahead logging
- Conflict detection
- Atomic commit protocol

**`core::replication`**
- Delta calculation and encoding
- Binary serialization format
- Broadcast message queuing
- Client synchronization protocol

**`core::persistence`**
- Snapshot creation and restoration
- WAL file management
- Async I/O coordination
- Compaction scheduling

#### 5.2.2 API Layer

**Database Handle**
- Entry point for all operations
- Configuration and initialization
- Table access and iteration
- Transaction management

**Table<T> Interface**
- Type-safe component table wrapper
- Row insertion/update/deletion
- Entity filtering and queries
- Batch operations

**Transaction Builder**
- Fluent API for multi-step operations
- Automatic rollback on panic
- Resource cleanup guarantees

#### 5.2.3 Storage Backend

**Buffer Manager**
- Allocates and resizes Vec<u8> buffers
- Tracks active/inactive buffer state
- Manages buffer swap coordination
- Monitors memory pressure

**Field Codec**
- Converts user types to/from bytes
- Handles alignment and padding
- Validates field layout at runtime
- Provides unsafe casting wrappers

### 5.3 Data Flow

#### 5.3.1 Write Path

```
Application
    ↓
API Call (Insert/Update/Delete)
    ↓
Transaction Builder
    ↓
Serialize to WAL Entry
    ↓
Queue to Write Channel
    ↓
Write Thread (Receives via MPSC)
    ↓
Modify Write Buffer
    ↓
Update Delta Tracker
    ↓
[Commit Batch Trigger or Explicit Commit]
    ↓
Atomic Swap: Write Buffer → Read Buffer
    ↓
Bump Version Number
    ↓
Broadcast Deltas to Replication Clients
    ↓
Queue Snapshot/WAL Async Flush
    ↓
Return Success to Application
```

#### 5.3.2 Read Path

```
Application
    ↓
Query API Call
    ↓
Acquire Read Buffer Reference
    ↓
Calculate Field Offset
    ↓
Unsafe Cast to User Type (T)
    ↓
Return Borrowed Reference (Lifetime Tied to Buffer)
    ↓
Application Uses Data
    ↓
Drop Borrowed Reference → Release Buffer Lock
```

#### 5.3.3 Replication Path

```
Write Commit (Server)
    ↓
Delta Tracker Records Changes
    ↓
Serialize Delta Batch
    ↓
Broadcast to All Connected Clients
    ↓
Client Receives Delta
    ↓
Validate Against Local Schema
    ↓
Apply Changes to Local Buffer
    ↓
Send ACK Back to Server
```

### 5.4 Concurrency Strategy

#### 5.4.1 Lock-Free Architecture

**Read Operations:**
- Multiple readers access read buffer simultaneously
- No locks; atomic reference counting ensures buffer validity
- Readers progress independently

**Write Operations:**
- Single writer thread processes all mutations
- Writes queued via MPSC channel (lock-free queue)
- Application threads don't block; returns immediately

**Commit Phase:**
- Atomic pointer swap for buffer switch (1 CPU instruction)
- All readers atomically see new committed state
- Version number increment signals update availability

#### 5.4.2 Memory Ordering Guarantees

- **Acquire Ordering** on read buffer pointer load (ensures delta visibility)
- **Release Ordering** on buffer swap (ensures all writes visible to readers)
- **Sequential Consistency** within single transaction (per-thread program order)

### 5.5 Schema System

#### 5.5.1 TOML Schema Format

```toml
[database]
name = "game_db"
version = "1.0.0"

[tables.entities]
primary_key = "id"
[[tables.entities.fields]]
name = "id"
type = "u64"
constraint = "unique"

[[tables.entities.fields]]
name = "version"
type = "u32"
auto_increment = true

[tables.transform]
parent_table = "entities"
[[tables.transform.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"

[[tables.transform.fields]]
name = "position_x"
type = "f32"

[[tables.transform.fields]]
name = "position_y"
type = "f32"

[[tables.transform.fields]]
name = "position_z"
type = "f32"

[tables.health]
parent_table = "entities"
[[tables.health.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"

[[tables.health.fields]]
name = "hp"
type = "u32"

[[tables.health.fields]]
name = "max_hp"
type = "u32"
```

#### 5.5.2 Type System

Supported types:
- **Primitives**: u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, bool
- **Fixed Arrays**: `[T; N]` (stored inline)
- **Enums**: Enumerated types with discriminant
- **Structs**: User-defined composite types (if layout is well-defined)
- **Strings**: Fixed-size byte arrays or references

#### 5.5.3 Schema Evolution

- **Schema Versioning**: Track version number with each table definition
- **Migration Tracking**: Log transformations applied to schema
- **Backward Compatibility**: Support field additions and deprecations
- **Zero-Downtime Updates**: Hot schema reload without stopping writes

---

## 6. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| **Insert Latency** | <1μs | Single row, non-batch |
| **Update Latency** | <1μs | Field-level updates |
| **Delete Latency** | <1μs | Logical delete with version |
| **Read Latency** | <100ns | Zero-copy access with cast |
| **Query Throughput** | >1M ops/sec | Sequential table scans |
| **Replication Lag** | <10ms | Delta broadcast to clients |
| **Snapshot Time** | <100ms | Full database dump |
| **Memory Overhead** | <10% | Schema, metadata, indices |
| **Disk Write Latency** | Background | Non-blocking async I/O |

---

## 7. Development Phases

### Phase 1: Core Engine (Weeks 1-4)
- [ ] Schema parser and TOML support
- [ ] Entity table with ID generation
- [ ] Component storage with Vec<u8> buffers
- [ ] Basic CRUD operations
- [ ] Double buffer implementation
- [ ] Transaction state machine
- [ ] Write-ahead logging

### Phase 2: Advanced Storage (Weeks 5-8)
- [ ] Delta tracking system
- [ ] Atomic commit protocol
- [ ] Referential integrity checks
- [ ] Sparse component handling
- [ ] Lock-free write queue (MPSC)
- [ ] Memory efficient buffering
- [ ] Field codec system

### Phase 3: Persistence (Weeks 9-12)
- [ ] Snapshot creation/restoration
- [ ] WAL archival and replay
- [ ] Async I/O integration (Tokio)
- [ ] Compaction worker
- [ ] Crash recovery
- [ ] Benchmark suite

### Phase 4: Replication (Weeks 13-16)
- [ ] Client connection management
- [ ] Delta serialization format
- [ ] Network broadcast mechanism
- [ ] Conflict resolution
- [ ] Full-sync protocol
- [ ] Incremental sync
- [ ] Client library

### Phase 5: Dashboard & Polish (Weeks 17-20)
- [ ] Tauri 2 app skeleton
- [ ] Vue 3 schema editor
- [ ] Data viewer component
- [ ] Query builder UI
- [ ] Integration testing
- [ ] Performance profiling
- [ ] Documentation

---

## 8. Non-Functional Requirements

### 8.1 Code Quality
- **Language**: Pure Rust, no unsafe except verified critical paths
- **Testing**: Unit tests >80% coverage, integration tests for all features
- **Documentation**: Inline code docs, API guide, architecture ADRs
- **Benchmarking**: Criterion benchmarks for all hot paths

### 8.2 Deployment
- **Library Distribution**: Published to crates.io
- **Embedded Deployment**: Zero external dependencies beyond Tokio
- **Dashboard Distribution**: Standalone Tauri executable
- **Configuration**: Environment variables + TOML config files

### 8.3 Maintainability
- **Module Organization**: Clear separation of concerns
- **Error Handling**: Custom error types with context
- **Logging**: Tracing integration for observability
- **Version Compatibility**: Semver compliance

---

## 9. Success Criteria

- [ ] MVP database performs 1M ops/sec with <1μs latency
- [ ] Zero-copy reads verified via profiling (no extra allocations)
- [ ] Atomic transactions pass ACID compliance tests
- [ ] Lock-free write queue passes contention benchmarks
- [ ] Multi-client replication syncs within 10ms
- [ ] Schema TOML format matches spec, auto-validates
- [ ] Dashboard launches with Tauri, loads/edits schemas
- [ ] All major code paths covered by tests
- [ ] Public documentation complete and code examples work

---

## 10. Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| **Memory safety issues** | Medium | High | Extensive testing of unsafe code, use MIRI |
| **Lock-free complexity** | Medium | High | Thorough code review, concurrency tests |
| **Delta serialization bugs** | Medium | Medium | Property-based testing, fuzzing |
| **Schema evolution issues** | Low | High | Comprehensive migration tests |
| **Replication inconsistency** | Medium | High | Conflict resolution tests, server authority |
| **Performance regression** | Medium | High | Continuous benchmarking in CI |
