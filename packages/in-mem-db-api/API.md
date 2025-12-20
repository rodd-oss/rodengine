# In-Memory Database REST API Reference

## Overview

The In-Memory Database provides a RESTful API for managing tables, records, relations, and executing procedures. All endpoints return JSON responses and use standard HTTP status codes.

## Base URL

All API endpoints are relative to the server address (e.g., `http://localhost:8080`).

## Authentication

Currently, the API does not require authentication. Future versions may add authentication middleware.

## Response Format

All API responses follow a consistent JSON format:

### Success Responses

```json
{
  "success": true,
  "data": {
    // Response data specific to the endpoint
  }
}
```

### Error Responses

```json
{
  "success": false,
  "error": {
    "code": "status_code",
    "message": "Error description",
    "details": "Optional additional error details"
  }
}
```

### Common Status Codes

- **200 OK**: Request succeeded
- **201 Created**: Resource created successfully
- **204 No Content**: Operation succeeded, no content to return
- **400 Bad Request**: Invalid request parameters or data
- **404 Not Found**: Resource not found
- **405 Method Not Allowed**: HTTP method not supported for endpoint
- **408 Request Timeout**: Request timed out
- **500 Internal Server Error**: Server-side error
- **503 Service Unavailable**: Server overloaded or unavailable

## Table Management

### Create Table

`POST /tables/{name}`

Creates a new table with the specified fields.

**Request Body:**

```json
{
  "fields": [
    { "name": "field1", "type": "u64" },
    { "name": "field2", "type": "string" },
    { "name": "field3", "type": "3xf32" }
  ]
}
```

**Response (201 Created):**

```json
{
  "success": true,
  "data": {
    "table": "table_name",
    "record_size": 24
  }
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/tables/users \
  -H "Content-Type: application/json" \
  -d '{"fields": [{"name": "id", "type": "u64"}, {"name": "name", "type": "string"}]}'
```

### Delete Table

`DELETE /tables/{name}`

Deletes a table and all its data.

**Response:** 204 No Content

**Example:**

```bash
curl -X DELETE http://localhost:8080/tables/users
```

### List Tables

`GET /tables`

Lists all tables in the database.

**Response (200 OK):**

```json
{
  "success": true,
  "data": {
    "tables": ["users", "posts", "comments"],
    "count": 3
  }
}
```

**Example:**

```bash
curl http://localhost:8080/tables
```

## Field Management

### Add Field

`POST /tables/{name}/fields`

Adds a field to an existing table. This is a blocking operation that rewrites all existing records.

**Request Body:**

```json
{
  "name": "field_name",
  "type": "field_type"
}
```

**Response (200 OK):**

```json
{
  "success": true,
  "data": {
    "offset": 24,
    "record_size": 25
  }
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/tables/users/fields \
  -H "Content-Type: application/json" \
  -d '{"name": "email", "type": "string"}'
```

### Remove Field

`DELETE /tables/{name}/fields/{field}`

Removes a field from a table. This is a blocking operation that rewrites all existing records.

**Response:** 204 No Content

**Example:**

```bash
curl -X DELETE http://localhost:8080/tables/users/fields/email
```

## Record Operations

### Create Record

`POST /tables/{name}/records`

Creates a new record in a table.

**Request Body:**

```json
{
  "values": [123, "entity_name", [1.0, 2.0, 3.0], true]
}
```

**Response (201 Created):**

```json
{
  "success": true,
  "data": {
    "id": 123
  }
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/tables/users/records \
  -H "Content-Type: application/json" \
  -d '{"values": [1, "John Doe", true]}'
```

### Read Record

`GET /tables/{name}/records/{id}`

Reads a record from a table.

**Response (200 OK):**

```json
{
  "success": true,
  "data": {
    "id": 123,
    "name": "entity_name",
    "position": [1.0, 2.0, 3.0],
    "active": true
  }
}
```

**Example:**

```bash
curl http://localhost:8080/tables/users/records/1
```

### Update Record (Full)

`PUT /tables/{name}/records/{id}`

Fully replaces a record with new values.

**Request Body:**

```json
{
  "values": [123, "updated_name", [4.0, 5.0, 6.0], false]
}
```

**Response:** 204 No Content

**Example:**

```bash
curl -X PUT http://localhost:8080/tables/users/records/1 \
  -H "Content-Type: application/json" \
  -d '{"values": [1, "Jane Doe", false]}'
```

### Update Record (Partial)

`PATCH /tables/{name}/records/{id}`

Partially updates specific fields of a record.

**Request Body:**

```json
{
  "updates": {
    "name": "updated_name",
    "active": false
  }
}
```

**Response:** 204 No Content

**Example:**

```bash
curl -X PATCH http://localhost:8080/tables/users/records/1 \
  -H "Content-Type: application/json" \
  -d '{"updates": {"name": "Updated Name", "active": false}}'
```

### Delete Record

`DELETE /tables/{name}/records/{id}`

Deletes a record (soft delete).

**Response:** 204 No Content

**Example:**

```bash
curl -X DELETE http://localhost:8080/tables/users/records/1
```

### Query Records

`GET /tables/{name}/records`

Queries records with filtering and pagination.

**Query Parameters:**

- `limit`: Maximum number of records to return
- `offset`: Number of records to skip
- `{field}`: Filter by field value (e.g., `active=true`)

**Response (200 OK):**

```json
{
  "success": true,
  "data": {
    "records": [
      {
        "id": 1,
        "name": "User1",
        "active": true
      },
      {
        "id": 2,
        "name": "User2",
        "active": false
      }
    ],
    "count": 2,
    "total": 100,
    "limit": 10,
    "offset": 0
  }
}
```

**Examples:**

```bash
# Get first 10 records
curl "http://localhost:8080/tables/users/records?limit=10"

# Get records 11-20
curl "http://localhost:8080/tables/users/records?limit=10&offset=10"

# Get active users
curl "http://localhost:8080/tables/users/records?active=true"

# Get specific user by name
curl "http://localhost:8080/tables/users/records?name=John%20Doe"
```

## Relation Management

### Create Relation

`POST /relations`

Creates a foreign key relation between tables.

**Request Body:**

```json
{
  "from_table": "users",
  "from_field": "id",
  "to_table": "posts",
  "to_field": "user_id"
}
```

**Response (201 Created):**

```json
{
  "success": true,
  "data": {
    "id": "users.id->posts.user_id"
  }
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/relations \
  -H "Content-Type: application/json" \
  -d '{"from_table": "users", "from_field": "id", "to_table": "posts", "to_field": "user_id"}'
```

### Delete Relation

`DELETE /relations/{id}`

Deletes a foreign key relation.

**Response:** 204 No Content

**Example:**

```bash
curl -X DELETE http://localhost:8080/relations/users.id->posts.user_id
```

## Procedure Execution (RPC)

### Execute Procedure

`POST /rpc/{name}`

Executes a registered procedure.

**Request Body:**

```json
{
  "table": "users",
  "filter_field": "active",
  "filter_value": false,
  "set_field": "active",
  "set_value": true
}
```

**Response (200 OK):**

```json
{
  "success": true,
  "data": {
    "affected": 42
  }
}
```

**Example:**

```bash
curl -X POST http://localhost:8080/rpc/bulk_update \
  -H "Content-Type: application/json" \
  -d '{"table": "users", "filter_field": "active", "filter_value": false, "set_field": "active", "set_value": true}'
```

## Data Types

### Built-in Types

| Type ID  | Description             | Size                  | Example Value                                   |
| -------- | ----------------------- | --------------------- | ----------------------------------------------- |
| `i8`     | 8-bit signed integer    | 1 byte                | `-128` to `127`                                 |
| `i16`    | 16-bit signed integer   | 2 bytes               | `-32768` to `32767`                             |
| `i32`    | 32-bit signed integer   | 4 bytes               | `-2147483648` to `2147483647`                   |
| `i64`    | 64-bit signed integer   | 8 bytes               | `-9223372036854775808` to `9223372036854775807` |
| `u8`     | 8-bit unsigned integer  | 1 byte                | `0` to `255`                                    |
| `u16`    | 16-bit unsigned integer | 2 bytes               | `0` to `65535`                                  |
| `u32`    | 32-bit unsigned integer | 4 bytes               | `0` to `4294967295`                             |
| `u64`    | 64-bit unsigned integer | 8 bytes               | `0` to `18446744073709551615`                   |
| `f32`    | 32-bit floating point   | 4 bytes               | `3.14159`                                       |
| `f64`    | 64-bit floating point   | 8 bytes               | `3.141592653589793`                             |
| `bool`   | Boolean                 | 1 byte                | `true` or `false`                               |
| `string` | Variable-length string  | 4-byte length + UTF-8 | `"Hello, World!"`                               |

### Custom Types

Custom types can be registered at runtime. Example composite type:

- `3xf32`: 3x 32-bit floats (12 bytes total) - `[1.0, 2.0, 3.0]`

## Rate Limiting

The API implements rate limiting based on the runtime tickrate:

- Maximum requests per tick: `tickrate * 10` (default: 600 requests/second at 60Hz)
- Queue size: `tickrate * 100` (default: 6000 requests)
- Overflow response: 503 Service Unavailable

## Timeouts

- Request timeout: 30 seconds (configurable)
- Response timeout: 30 seconds (configurable)

## Performance Characteristics

- **Read latency**: < 1 microsecond
- **Write latency**: < 5 microseconds
- **Read throughput**: > 10M operations/second/core
- **Write throughput**: > 1M operations/second/core
- **Memory overhead**: < 5% beyond raw data size

## Concurrency Model

- **Lock-free reads**: Multiple concurrent readers without blocking
- **Atomic writes**: Write operations are atomic via copy-on-write
- **Transaction isolation**: Read-committed isolation level
- **Procedure transactions**: Procedures run in isolated staging buffers

## Examples

### Complete Workflow Example

```bash
# 1. Create a table
curl -X POST http://localhost:8080/tables/products \
  -H "Content-Type: application/json" \
  -d '{"fields": [{"name": "id", "type": "u64"}, {"name": "name", "type": "string"}, {"name": "price", "type": "f64"}, {"name": "in_stock", "type": "bool"}]}'

# 2. Create records
curl -X POST http://localhost:8080/tables/products/records \
  -H "Content-Type: application/json" \
  -d '{"values": [1, "Laptop", 999.99, true]}'

curl -X POST http://localhost:8080/tables/products/records \
  -H "Content-Type: application/json" \
  -d '{"values": [2, "Mouse", 29.99, true]}'

curl -X POST http://localhost:8080/tables/products/records \
  -H "Content-Type: application/json" \
  -d '{"values": [3, "Keyboard", 79.99, false]}'

# 3. Query records
curl "http://localhost:8080/tables/products/records?in_stock=true"

# 4. Update a record
curl -X PATCH http://localhost:8080/tables/products/records/3 \
  -H "Content-Type: application/json" \
  -d '{"updates": {"in_stock": true, "price": 69.99}}'

# 5. Execute a procedure (if registered)
curl -X POST http://localhost:8080/rpc/apply_discount \
  -H "Content-Type: application/json" \
  -d '{"table": "products", "discount_percent": 10}'
```

## Notes

1. **Field Order**: When creating or updating records, values must be provided in the same order as the table's fields.
2. **Blocking Operations**: Adding/removing fields are blocking operations that rewrite all records in the table.
3. **Soft Deletes**: Record deletion is soft (marked as deleted) until compaction.
4. **Transaction Scope**: Each API operation is atomic. Procedures run in isolated transactions.
5. **Memory Layout**: Records are tightly packed in memory for cache efficiency.
