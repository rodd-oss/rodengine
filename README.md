# RodEngine - High-Performance In-Memory Relational Database

RodEngine is a high-performance, lock-free in-memory relational database written in Rust. It features a REST API, transactional consistency, and parallel procedure execution.

## Features

- **Lock-free reads**: Zero-copy access with `ArcSwap` for concurrent readers
- **Atomic writes**: Copy-on-write semantics with atomic buffer swaps
- **REST API**: Full HTTP/JSON API for all operations
- **Transaction isolation**: Read-committed isolation with staging buffers
- **Parallel procedures**: Rayon-powered parallel execution across CPU cores
- **Persistence**: Async disk flushing with atomic file operations
- **Custom types**: Extensible type system with runtime registration
- **Relations**: Foreign key relationships with cascade behaviors

## Performance

- **Read latency**: < 1 microsecond
- **Write latency**: < 5 microseconds
- **Read throughput**: > 10M operations/second/core
- **Write throughput**: > 1M operations/second/core
- **Memory overhead**: < 5% beyond raw data size

## Quick Start

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd rodengine

# Build the database server
cargo build --release --package db-server
```

### Start the Server

```bash
# Start on default port (8080)
./target/release/db-server

# Or with custom configuration
./target/release/db-server --port 3000 --tickrate 120
```

### Create Your First Table

```bash
curl -X POST http://localhost:8080/tables/users \
  -H "Content-Type: application/json" \
  -d '{
    "fields": [
      {"name": "id", "type": "u64"},
      {"name": "name", "type": "string"},
      {"name": "email", "type": "string"}
    ]
  }'
```

### Create a Record

```bash
curl -X POST http://localhost:8080/tables/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [1, "Alice Smith", "alice@example.com"]
  }'
```

### Query Records

```bash
curl "http://localhost:8080/tables/users/records"
```

## Documentation

- **[Quick Start Guide](./docs/quick_start.md)**: Get started in minutes
- **[API Reference](./docs/api_reference.md)**: Complete endpoint documentation
- **[Technical Reference](./docs/db_technical_reference.txt)**: Architecture and implementation details
- **[Product Requirements](./docs/db_product_requirements.txt)**: Feature specifications and constraints

## Project Structure

```text
rodengine/
├── packages/
│   ├── in-mem-db-core/     # Core database engine
│   ├── in-mem-db-api/      # REST API server
│   └── in-mem-db-runtime/  # Runtime loop and procedure execution
├── apps/
│   ├── db-server/          # Database server binary
│   ├── db-tool/            # Command-line tool
│   └── db-bench/           # Benchmark suite
├── docs/                   # Documentation
├── examples/               # Usage examples
└── benches/               # Performance benchmarks
```

## Building from Source

### Prerequisites

- Rust 1.70 or later
- Cargo

### Build All Components

```bash
# Build everything
cargo build --release

# Build specific components
cargo build --release --package db-server
cargo build --release --package db-bench
cargo build --release --package db-tool
```

### Run Tests

```bash
# Run all tests
cargo test --all

# Run specific test suite
cargo test --package in-mem-db-core
cargo test --package in-mem-db-api
```

### Run Benchmarks

```bash
# Run performance benchmarks
cargo bench --all

# Run specific benchmark
cargo bench --package in-mem-db-core
```

## API Overview

The database provides a comprehensive REST API:

### Table Management

- `POST /tables/{name}` - Create table
- `DELETE /tables/{name}` - Delete table
- `GET /tables` - List all tables

### Field Management

- `POST /tables/{name}/fields` - Add field
- `DELETE /tables/{name}/fields/{field}` - Remove field

### Record Operations

- `POST /tables/{name}/records` - Create record
- `GET /tables/{name}/records` - Query records
- `GET /tables/{name}/records/{id}` - Read record
- `PUT /tables/{name}/records/{id}` - Update record (full)
- `PATCH /tables/{name}/records/{id}` - Update record (partial)
- `DELETE /tables/{name}/records/{id}` - Delete record

### Relations

- `POST /relations` - Create relation
- `DELETE /relations/{id}` - Delete relation

### Procedures

- `POST /rpc/{name}` - Execute procedure

## Configuration

### Server Configuration

| Parameter               | Description                   | Default  |
| ----------------------- | ----------------------------- | -------- |
| `port`                  | HTTP server port              | `8080`   |
| `data-dir`              | Data directory                | `./data` |
| `tickrate`              | Runtime frequency (15-120 Hz) | `60`     |
| `max-requests-per-tick` | Rate limit                    | `600`    |
| `request-timeout-ms`    | Request timeout               | `30000`  |
| `response-timeout-ms`   | Response timeout              | `30000`  |

### Database Configuration

See `in_mem_db_core::config::DbConfig` for all configuration options.

## Examples

Check the `examples/` directory for complete usage examples:

- `query_records.rs`: Demonstrates querying with filters and pagination
- More examples coming soon...

## License

License information to be added

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test --all`
5. Format code: `cargo fmt --all`
6. Check linting: `cargo clippy --all-targets -- -D warnings`
7. Submit a pull request

## Support

- **Documentation**: See the `docs/` directory
- **Issues**: Report bugs on GitHub
- **Questions**: Open a discussion on GitHub
