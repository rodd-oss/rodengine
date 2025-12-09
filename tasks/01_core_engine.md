# Phase 1: Core Engine
**Estimated Time:** Weeks 1-4

## Overview
Build the foundational modules for the ECS database: schema system, entity registry, component storage, basic CRUD operations, double buffering, transaction state machine, and write-ahead logging.

## Dependencies
- Rust toolchain (cargo, rustc)
- Basic Tauri+Vue project setup (already exists)

## Subtasks

### 1.1 Project Structure Setup
- Create Rust library crate `ecsdb` inside `src-tauri/` or as separate workspace
- Define module structure per architecture doc: `schema`, `storage`, `entity`, `transaction`, `error`
- Add dependencies to Cargo.toml: tokio, serde, toml, bincode, thiserror, dashmap, parking_lot, uuid, zstd, bytes
- Set up dev dependencies: criterion, proptest, tokio-test

### 1.2 Error Handling (`error.rs`)
- Define `EcsDbError` enum with variants for entity not found, component not found, schema errors, transaction errors, I/O errors, serialization errors, field type mismatches, alignment errors, channel closed, timeout
- Implement `thiserror::Error` derive
- Define `Result<T>` alias

### 1.3 Schema System (`schema/`)
- **Types (`types.rs`)**: Define `FieldType` enum (primitives, arrays, enums, structs, custom), `FieldDefinition`, `TableDefinition`, `DatabaseSchema`
- Implement size calculation and alignment methods
- **Parser (`parser.rs`)**: Parse TOML schema files into `DatabaseSchema`
- Support custom types, enums, tables, fields, foreign keys, parent tables
- **Validator (`validator.rs`)**: Validate schema consistency, foreign key references, type compatibility
- **Migrations (`migrations.rs`)**: Placeholder for future schema versioning

### 1.4 Entity Registry (`entity/`)
- **Entity ID and Version**: Newtypes `EntityId(u64)`, `EntityVersion(u32)`
- **EntityRecord**: id, version, archetype_hash
- **EntityRegistry**: packed entity buffer, id→offset index, freelist for reuse, next ID counter
- Methods: `create_entity(archetype_hash)`, `delete_entity(entity_id)`, `get_entity(entity_id)`

### 1.5 Storage Layer (`storage/`)
- **Buffer Management (`buffer.rs`)**: `StorageBuffer` struct with read buffer (atomic pointer), write buffer, staging buffer, record count, record size
- Implement `insert`, `update`, `read`, `commit` (atomic swap), `grow`
- **Field Codec (`field_codec.rs`)**: Serialize/deserialize Rust types to bytes, alignment handling, unsafe casting wrappers
- **Memory Layout (`layout.rs`)**: Calculate field offsets, padding, record size based on schema
- **Sparse Storage (`sparse.rs`)**: Bitmap indices for optional components (future)

### 1.6 Basic CRUD Operations
- **Database Handle (`db.rs`)**: Main entry point, holds schema, entity registry, write queue
- **Table Interface**: Generic `Table<T>` for type-safe component operations
- Implement `insert`, `update`, `delete`, `get` for single components
- Ensure zero-copy reads via unsafe casting after bounds checking

### 1.7 Double Buffer Implementation
- **DoubleBuffer** struct: read buffer (Arc<AtomicPtr<Vec<u8>>>), write buffer, shadow buffer, deltas, version
- **Atomic Swap**: Commit mechanism that swaps all table buffers simultaneously
- **BufferManager**: Manage multiple table buffers, coordinate commits

### 1.8 Transaction State Machine (`transaction/`)
- **TransactionOp** enum: Insert, Update, Delete (with table ID, entity ID, data)
- **Transaction** struct: collection of ops, response channel
- **TransactionEngine**: Process transactions, maintain WAL, bump version
- **Write‑Ahead Logging (`wal.rs`)**: Log operations before applying to buffer; define `WALEntry` with timestamp, txn ID, checksum

### 1.9 Initial Integration with Tauri
- Expose minimal database API via Tauri command (e.g., load schema, create entity)
- Update Vue frontend to display schema info (optional)
- Ensure library compiles and links with Tauri app

## Acceptance Criteria
1. Schema TOML file can be parsed and validated without errors
2. Entity registry can create, delete, and retrieve entities with unique IDs and versioning
3. Component tables store data in contiguous byte buffers; insertion and retrieval work
4. Double buffer commit atomically swaps read/write buffers; readers see consistent snapshots
5. Transaction log records operations; simple transaction (insert + commit) succeeds
6. All modules have unit tests (>80% coverage)
7. No unsafe code violations (run MIRI on tests)
8. Library integrates with Tauri and can be invoked from Vue frontend

## Output Artifacts
- Rust crate `ecsdb` with modules as described
- Example schema TOML file
- Unit test suite
- Basic Tauri command that loads a schema and creates an entity
- Updated `AGENTS.md` with build/test commands for the database

## Notes
- Focus on correctness over performance in this phase
- Use `unsafe` only where necessary (zero‑copy casting) and document safety invariants
- Follow Rust naming conventions and clippy lints
- Write doc comments for all public APIs
