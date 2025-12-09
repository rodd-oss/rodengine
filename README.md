# ECSDb - High-Performance Entity Component System Database

ECSdb is a lock-free relational database engine optimized for ECS (Entity Component System) game engines, built in Rust. It provides zero-copy data access, CPU cache efficiency, atomic transactions, and seamless client synchronization through delta-based replication.

## Features

- **Lock-Free Atomic Operations**: No mutex overhead, using atomic pointer swaps
- **Double-Buffer Architecture**: Readers isolated from writers with delta-only write propagation
- **Zero-Copy Field Access**: Unsafe casting after bounds checking for maximum performance
- **Native Multi-Client Replication**: Delta-based sync with efficient patch application
- **Embedded Crate Deployment**: Use as a Rust library with optional Tauri+Vue web dashboard
- **TOML Schema Definitions**: Human-readable schema format with validation
- **ACID Transactions**: Write-ahead logging and crash recovery

## Project Status

Early development. Following the [development phases](./tasks/README.md).

## Getting Started

### Prerequisites

- Rust toolchain (rustc, cargo) >= 1.70
- Node.js & bun (for frontend dashboard)
- Git

### Building

```bash
# Clone the repository
git clone <repo-url>
cd rodengine

# Install frontend dependencies
bun install

# Build the workspace (database + Tauri app)
cargo build --workspace

# Run the Tauri development server
bun run tauri dev
```

### Database Development

```bash
# Build only the database crate
cargo build -p ecsdb

# Run database tests
cargo test -p ecsdb

# Run benchmarks
cargo bench -p ecsdb

# Check formatting
cargo fmt --check -p ecsdb

# Lint
cargo clippy -p ecsdb -- -D warnings
```

### Using Makefile

A `Makefile` is provided with common tasks:

```bash
make build      # Build workspace
make test       # Run all tests
make bench      # Run benchmarks
make doc        # Generate documentation
make lint       # Run clippy
make fmt        # Format code
```

## Architecture

See [ECS Database PRD](./docs/ECS_Database_PRD.md) for detailed requirements and architecture.

## Development Phases

1. **Phase 1: Core Engine** - Schema system, entity registry, component storage, double buffering
2. **Phase 2: Advanced Storage** - Delta tracking, atomic commits, sparse components, lock-free write queue
3. **Phase 3: Persistence** - Snapshots, WAL archival, async I/O, crash recovery
4. **Phase 4: Replication** - Multi-client sync, network protocol, conflict resolution
5. **Phase 5: Dashboard & Polish** - Tauri+Vue dashboard, schema editor, data viewer, query builder

## License

MIT OR Apache-2.0

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development workflow.
