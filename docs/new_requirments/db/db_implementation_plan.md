# Database Implementation Plan

## Overview

Relational in-memory database for online games with REST API, zero-copy access, and cache-efficient storage.

## Architecture

### Packages (`packages/`)

- **db-core**: Storage engine, schema, data model (foundation)
- **db-types**: Shared types, field definitions, utilities
- **db-runtime**: Synchronous event loop (15-120 Hz), parallel procedures
- **db-api**: REST/RPC API using axum + tokio
- **db-persistence**: Disk snapshots, JSON schema serialization

### Applications (`apps/`)

- **db-server**: Main database server executable
- **db-cli**: Command-line interface (HTTP client)
- **db-test-runner**: TDD test execution harness

## Technology Stack

- **Language**: Rust
- **Async Runtime**: Tokio
- **Web Framework**: Axum
- **Concurrency**: ArcSwap for lock-free buffer swapping
- **Parallelism**: Rayon for data-parallel operations
- **Serialization**: Serde + JSON
- **Logging**: Tracing crate

## Implementation Phases (12 Weeks)

### Phase 1: Foundation (Week 1-2) - `db-core`

- Storage buffer (`Vec<u8>`) with capacity management
- Zero-copy field accessors returning `&T`
- Memory safety validation and bounds checking
- Table/field schema definitions

### Phase 2: Data Model (Week 3-4) - `db-types` + `db-core`

- Custom composite types (e.g., `Vec3` as `3xf32`)
- Relations between tables with referential integrity
- JSON schema serialization/deserialization

### Phase 3: Concurrency (Week 5-6) - `db-core`

- ArcSwap buffer wrapping for atomic swaps
- Lock-free read API with `ArcSwap::load`
- Atomic CRUD operations with transaction log

### Phase 4: Runtime (Week 7-8) - `db-runtime`

- Synchronous event loop with configurable tickrate (15-120 Hz)
- Handler registration system for API calls and procedures
- Parallel iteration over table data using rayon
- Cache efficiency optimizations (tight packing)

### Phase 5: API Layer (Week 9-10) - `db-api`

- REST endpoints for schema management (tables, fields, relations)
- CRUD operations with JSON request/response
- JSON-RPC over HTTP for custom procedures
- Transactional procedure execution

### Phase 6: Persistence & Integration (Week 11-12)

- Binary snapshots to disk with background saving
- Recovery from snapshots with integrity validation
- Main server executable with configuration
- CLI tool for administration
- Monorepo workspace integration

## Key Design Principles

### Storage

- `Vec<u8>` per table with unsafe casting for field access
- Tight packing: fields within records, records within tables
- Zero-copy: accessors return references, not copies
- Contiguous memory for CPU cache locality

### Concurrency

- ArcSwap for atomic buffer updates
- Readers never blocked by writers
- Each CRUD operation atomic (all-or-nothing)
- Transaction log for rollback on failure

### Performance

- Cache-friendly data layout (no padding)
- Parallel iteration across CPU cores
- Configurable tickrate (15-120 Hz) for game loops
- Minimal allocations, maximum reuse

### API

- RESTful endpoints for schema and data operations
- JSON-RPC for custom procedures
- Type-safe validation against schema
- Error handling with clear messages

## Testing Strategy

- **TDD Approach**: Follow 50+ test plans in `test_plans/`
- **Unit Tests**: Each package has `tests/` directory
- **Integration**: `db-test-runner` executes test plans
- **Benchmarks**: Performance testing for cache efficiency
- **Concurrency**: Stress testing with parallel operations

## Development Workflow

1. Read test plan from `test_plans/`
2. Write failing test
3. Implement minimal code to pass test
4. Refactor while keeping tests passing
5. Repeat for each test case

## Dependencies

```toml
# Root workspace dependencies
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
arc-swap = "1.0"
rayon = "1.0"
thiserror = "1.0"
tracing = "0.1"
```

## Event Loop Design

```rust
// Synchronous loop with precise timing
let tick_duration = Duration::from_secs_f64(1.0 / tick_rate as f64);
while running {
    let start = Instant::now();

    // Execute registered handlers
    for handler in &handlers {
        handler();
    }

    let elapsed = start.elapsed();
    if elapsed < tick_duration {
        std::thread::sleep(tick_duration - elapsed);
    }
    // Overruns continue immediately (skip sleep)
}
```

## Next Steps

1. Update root `Cargo.toml` with workspace configuration
2. Create package directories and `Cargo.toml` files
3. Implement Phase 1: Storage buffer foundation
4. Follow TDD using test plans
5. Build upward through architecture layers

## Success Metrics

- All 50+ test plans passing
- Zero-copy access verified
- Cache efficiency benchmarks met
- Atomic operations under concurrency
- REST API complete and documented
- Event loop running at configured tickrate
