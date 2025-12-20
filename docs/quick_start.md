# Quick Start Guide

## Prerequisites

- Rust 1.70 or later
- Cargo (Rust package manager)

## Installation

### From Source

```bash
# Clone the repository
git clone <repository-url>
cd rodengine

# Build the database server
cargo build --release --package db-server

# The binary will be available at:
# ./target/release/db-server
```

### Using Cargo

```bash
# Install directly from crates.io (when published)
cargo install db-server
```

## Starting the Server

### Basic Startup

```bash
# Start server on default port (8080)
./target/release/db-server

# Or specify a custom port
./target/release/db-server --port 3000

# With custom data directory
./target/release/db-server --data-dir ./mydata
```

### Configuration Options

```bash
# Full configuration example
./target/release/db-server \
  --port 8080 \
  --data-dir ./data \
  --tickrate 60 \
  --max-requests-per-tick 600 \
  --request-timeout-ms 30000 \
  --response-timeout-ms 30000
```

### Command Line Arguments

| Argument                  | Description                      | Default  |
| ------------------------- | -------------------------------- | -------- |
| `--port`                  | HTTP server port                 | `8080`   |
| `--data-dir`              | Data directory for persistence   | `./data` |
| `--tickrate`              | Runtime tickrate (15-120 Hz)     | `60`     |
| `--max-requests-per-tick` | Maximum API requests per tick    | `600`    |
| `--request-timeout-ms`    | Request timeout in milliseconds  | `30000`  |
| `--response-timeout-ms`   | Response timeout in milliseconds | `30000`  |

## Your First Database

### 1. Start the Server

```bash
./target/release/db-server --port 8080
```

You should see:

```text
Server listening on http://0.0.0.0:8080
```

### 2. Create Your First Table

```bash
curl -X POST http://localhost:8080/tables/users \
  -H "Content-Type: application/json" \
  -d '{
    "fields": [
      {"name": "id", "type": "u64"},
      {"name": "name", "type": "string"},
      {"name": "email", "type": "string"},
      {"name": "age", "type": "u32"},
      {"name": "active", "type": "bool"}
    ]
  }'
```

Response:

```json
{ "table": "users", "record_size": 281 }
```

### 3. Add Some Records

```bash
# Create first user
curl -X POST http://localhost:8080/tables/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [1, "Alice Smith", "alice@example.com", 30, true]
  }'

# Create second user
curl -X POST http://localhost:8080/tables/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [2, "Bob Johnson", "bob@example.com", 25, true]
  }'

# Create third user
curl -X POST http://localhost:8080/tables/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [3, "Charlie Brown", "charlie@example.com", 35, false]
  }'
```

Each request returns:

```json
{"id":1}
{"id":2}
{"id":3}
```

### 4. Query Records

```bash
# List all users
curl "http://localhost:8080/tables/users/records"

# Get only active users
curl "http://localhost:8080/tables/users/records?active=true"

# Get first 2 users
curl "http://localhost:8080/tables/users/records?limit=2"

# Get user by email
curl "http://localhost:8080/tables/users/records?email=alice@example.com"
```

### 5. Update a Record

```bash
# Update user's age (partial update)
curl -X PATCH http://localhost:8080/tables/users/records/1 \
  -H "Content-Type: application/json" \
  -d '{
    "updates": {
      "age": 31,
      "email": "alice.smith@example.com"
    }
  }'
```

### 6. Delete a Record

```bash
# Delete user with ID 3
curl -X DELETE http://localhost:8080/tables/users/records/3
```

## Working with Relations

### Create Related Tables

```bash
# Create posts table
curl -X POST http://localhost:8080/tables/posts \
  -H "Content-Type: application/json" \
  -d '{
    "fields": [
      {"name": "id", "type": "u64"},
      {"name": "user_id", "type": "u64"},
      {"name": "title", "type": "string"},
      {"name": "content", "type": "string"},
      {"name": "created_at", "type": "u64"}
    ]
  }'

# Create relation between users and posts
curl -X POST http://localhost:8080/relations \
  -H "Content-Type: application/json" \
  -d '{
    "from_table": "users",
    "from_field": "id",
    "to_table": "posts",
    "to_field": "user_id"
  }'
```

### Create Related Records

```bash
# Create a post for user 1
curl -X POST http://localhost:8080/tables/posts/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [1, 1, "My First Post", "Hello world!", 1672531200]
  }'

# Create another post for user 2
curl -X POST http://localhost:8080/tables/posts/records \
  -H "Content-Type: application/json" \
  -d '{
    "values": [2, 2, "Another Post", "Content here", 1672617600]
  }'
```

## Using Procedures (RPC)

### Register a Procedure (Programmatic)

Procedures must be registered programmatically. Here's a simple example:

```rust
use in_mem_db_core::database::Database;
use in_mem_db_runtime::Runtime;
use serde_json::json;

// Create database and runtime
let db = Database::new(Default::default());
let mut runtime = Runtime::new(db.clone(), Default::default(), api_rx, persistence_tx);

// Register a procedure
runtime.register_procedure("count_active_users", |db, tx, params| {
    let table_name = params["table"].as_str().unwrap();
    let table = db.get_table(table_name)?;
    let buffer = table.buffer.load();
    let record_size = table.record_size;

    let mut count = 0;
    for chunk in buffer.chunks_exact(record_size) {
        // Check if record is active (simplified)
        // In reality, you'd deserialize the bool field
        if chunk[280] != 0 {  // active field at offset 280
            count += 1;
        }
    }

    Ok(json!({ "count": count }))
});

// Start runtime
runtime.run()?;
```

### Execute a Procedure via API

```bash
# Execute the registered procedure
curl -X POST http://localhost:8080/rpc/count_active_users \
  -H "Content-Type: application/json" \
  -d '{
    "table": "users"
  }'
```

Response:

```json
{ "count": 2 }
```

## Schema Management

### List All Tables

```bash
curl http://localhost:8080/tables
```

Response:

```json
["users", "posts"]
```

### Add a Field to Existing Table

```bash
curl -X POST http://localhost:8080/tables/users/fields \
  -H "Content-Type: application/json" \
  -d '{
    "name": "phone",
    "type": "string"
  }'
```

**Note**: This is a blocking operation that rewrites all existing records.

### Remove a Field

```bash
curl -X DELETE http://localhost:8080/tables/users/fields/phone
```

**Note**: This is a blocking operation and data in the removed field is lost.

## Data Persistence

### Automatic Persistence

The database automatically persists data:

- **Schema**: Written to `data/schema.json` on every DDL operation
- **Data**: Asynchronously flushed to `data/{table}.bin` files

### Manual Save

The server automatically persists data. To ensure all data is saved before shutdown:

```bash
# Gracefully stop the server (Ctrl+C)
# The server will flush all pending writes before exiting
```

### Recovery

On restart, the server automatically loads data from the data directory:

```bash
# Start server with existing data directory
./target/release/db-server --data-dir ./data

# Server will load:
# - Schema from data/schema.json
# - Table data from data/*.bin files
```

## Performance Testing

### Using the Benchmark Tool

```bash
# Build the benchmark tool
cargo build --release --package db-bench

# Run benchmarks
./target/release/db-bench \
  --server http://localhost:8080 \
  --threads 4 \
  --operations 100000
```

### Simple Load Test with curl

```bash
# Create test table
curl -X POST http://localhost:8080/tables/test \
  -H "Content-Type: application/json" \
  -d '{"fields": [{"name": "id", "type": "u64"}, {"name": "value", "type": "u64"}]}'

# Generate load (create 1000 records)
for i in {1..1000}; do
  curl -X POST http://localhost:8080/tables/test/records \
    -H "Content-Type: application/json" \
    -d "{\"values\": [$i, $((i * 100))]}" &
done
wait
```

## Common Tasks

### Export Data

```bash
# Export all users to JSON
curl "http://localhost:8080/tables/users/records" > users.json

# Export with specific fields
curl "http://localhost:8080/tables/users/records?active=true&limit=100" > active_users.json
```

### Import Data

```bash
# Read JSON file and create records
jq -c '.[]' users.json | while read record; do
  curl -X POST http://localhost:8080/tables/users/records \
    -H "Content-Type: application/json" \
    -d "{\"values\": [$record]}"
done
```

### Monitor Server Status

```bash
# Check if server is running
curl -I http://localhost:8080/tables

# Get server metrics (if implemented)
curl http://localhost:8080/metrics
```

## Troubleshooting

### Server Won't Start

**Problem**: Port already in use

```bash
# Check what's using the port
lsof -i :8080

# Use a different port
./target/release/db-server --port 8081
```

**Problem**: Permission denied

```bash
# Run on a higher port (requires root for ports < 1024)
sudo ./target/release/db-server --port 80

# Or use a port > 1024
./target/release/db-server --port 3000
```

### Connection Refused

**Problem**: Server not running

```bash
# Check if server process is running
ps aux | grep db-server

# Start the server
./target/release/db-server
```

### Timeout Errors

**Problem**: Request taking too long

```bash
# Increase timeout
curl --max-time 60 http://localhost:8080/tables

# Check server load
# The server may be rate limiting or queue is full
```

### Data Not Persisting

**Problem**: Data directory permissions

```bash
# Check data directory permissions
ls -la ./data/

# Ensure write permissions
chmod +w ./data/
```

## Next Steps

1. **Explore the API**: Try all endpoints from the [API Reference](./api_reference.md)
2. **Write Procedures**: Create custom procedures for your use case
3. **Benchmark**: Test performance with your workload
4. **Monitor**: Implement monitoring for production use
5. **Scale**: Consider running multiple instances for higher throughput

## Getting Help

- **Documentation**: See [API Reference](./api_reference.md) for detailed endpoint documentation
- **Examples**: Check the `examples/` directory for code examples
- **Issues**: Report bugs or feature requests on GitHub
