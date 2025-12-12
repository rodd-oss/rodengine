# Database Implementation Quick Reference

## Package Structure

```text
packages/
├── db-core/          # Storage engine, schema, data model
├── db-types/         # Shared types, field definitions
├── db-runtime/       # Event loop, parallel procedures
├── db-api/           # REST/RPC API (axum + tokio)
└── db-persistence/   # Disk snapshots, JSON schema

apps/
├── db-server/        # Main database server
├── db-cli/           # Command-line interface
└── db-test-runner/   # TDD test execution
```

## Key Dependencies

```toml
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
arc-swap = "1.0"
rayon = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
```

## Core Principles

1. **Zero-copy**: Field accessors return `&T`, not copies
2. **Cache-friendly**: Tight packing, no padding, contiguous memory
3. **Lock-free**: ArcSwap for atomic buffer updates
4. **Atomic operations**: Each CRUD operation all-or-nothing
5. **Parallel iteration**: Rayon for data-parallel procedures

## Test-Driven Development

- 50+ test plans in `test_plans/` directory
- Each task has corresponding test plan (e.g., `sl_1` → `db_test_plan_sl_1.md`)
- Follow TDD workflow: read test → write failing test → implement → refactor

## Event Loop Design

```rust
// Synchronous loop, 15-120 Hz configurable
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
}
```

## REST API Endpoints

### Schema Management

- `POST /table` - Create table
- `DELETE /table/{name}` - Delete table
- `POST /table/{name}/field` - Add field
- `DELETE /table/{name}/field/{fieldName}` - Remove field
- `POST /relation` - Create relation
- `DELETE /relation/{id}` - Delete relation

### CRUD Operations

- `POST /table/{name}/record` - Insert record
- `GET /table/{name}/record/{id}` - Retrieve record
- `PUT /table/{name}/record/{id}` - Update record
- `DELETE /table/{name}/record/{id}` - Delete record
- `GET /table/{name}/records` - List records (pagination)

### RPC

- `POST /rpc` - JSON-RPC endpoint

## Storage Layout

- Each table: `Vec<u8>` buffer
- Records: Tightly packed, no padding between fields
- Fields: Tightly packed within records
- Access: Unsafe pointer casting with bounds checking

## Concurrency Model

- Readers: `ArcSwap::load()` for current buffer reference
- Writers: Atomic buffer swap with `ArcSwap`
- No mutex locks for read operations
- Transaction log for rollback on failure

## Build Commands

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Build specific package
cargo build -p db-core

# Run specific test
cargo test -p db-core --test buffer_tests

# Check formatting
cargo fmt -- --check

# Clippy linting
cargo clippy --workspace -- -D warnings
```

## Development Workflow

1. Start with Phase 1 (Week 1-2): `db-core` foundation
2. Follow weekly breakdown in `weekly_breakdown.md`
3. Use test plans as implementation guide
4. Build upward through architecture layers
5. Validate with `db-test-runner` application
