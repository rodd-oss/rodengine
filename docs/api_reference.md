# API Reference

## Endpoint Summary

| Method     | Path                            | Description             | Status Codes       |
| ---------- | ------------------------------- | ----------------------- | ------------------ |
| **GET**    | `/tables`                       | List all tables         | 200, 500           |
| **POST**   | `/tables/{name}`                | Create table            | 201, 400, 500      |
| **DELETE** | `/tables/{name}`                | Delete table            | 204, 400, 500      |
| **POST**   | `/tables/{name}/fields`         | Add field to table      | 200, 400, 500      |
| **DELETE** | `/tables/{name}/fields/{field}` | Remove field from table | 204, 400, 500      |
| **POST**   | `/tables/{name}/records`        | Create record           | 201, 400, 500      |
| **GET**    | `/tables/{name}/records`        | Query records           | 200, 400, 500      |
| **GET**    | `/tables/{name}/records/{id}`   | Read record             | 200, 400, 404, 500 |
| **PUT**    | `/tables/{name}/records/{id}`   | Update record (full)    | 204, 400, 404, 500 |
| **PATCH**  | `/tables/{name}/records/{id}`   | Update record (partial) | 204, 400, 404, 500 |
| **DELETE** | `/tables/{name}/records/{id}`   | Delete record           | 204, 400, 404, 500 |
| **POST**   | `/relations`                    | Create relation         | 201, 400, 500      |
| **DELETE** | `/relations/{id}`               | Delete relation         | 204, 400, 500      |
| **POST**   | `/rpc/{name}`                   | Execute procedure       | 200, 400, 404, 500 |

## Detailed Endpoint Specifications

### GET /tables

**Description**: Lists all tables in the database.

**Response (200 OK)**:

```json
["users", "posts", "comments"]
```

**Errors**:

- **500 Internal Server Error**: Runtime error or channel communication failure

### POST /tables/{name}

**Description**: Creates a new table with the specified fields.

**Path Parameters**:

- `name` (string): Table name

**Request Body**:

```json
{
  "fields": [
    {
      "name": "field_name",
      "type": "field_type"
    }
  ]
}
```

**Response (201 Created)**:

```json
{
  "table": "table_name",
  "record_size": 24
}
```

**Errors**:

- **400 Bad Request**: Invalid field definitions, unknown type, or table already exists
- **500 Internal Server Error**: Runtime error or channel communication failure

### DELETE /tables/{name}

**Description**: Deletes a table and all its data.

**Path Parameters**:

- `name` (string): Table name

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Table not found
- **500 Internal Server Error**: Runtime error or channel communication failure

### POST /tables/{name}/fields

**Description**: Adds a field to an existing table.

**Path Parameters**:

- `name` (string): Table name

**Request Body**:

```json
{
  "name": "field_name",
  "type": "field_type"
}
```

**Response (200 OK)**:

```json
{
  "offset": 24,
  "record_size": 25
}
```

**Errors**:

- **400 Bad Request**: Table not found, field already exists, or unknown type
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: This is a blocking operation that rewrites all existing records.

### DELETE /tables/{name}/fields/{field}

**Description**: Removes a field from a table.

**Path Parameters**:

- `name` (string): Table name
- `field` (string): Field name

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Table not found or field not found
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: This is a blocking operation that rewrites all existing records.

### POST /tables/{name}/records

**Description**: Creates a new record in a table.

**Path Parameters**:

- `name` (string): Table name

**Request Body**:

```json
{
  "values": [value1, value2, value3, ...]
}
```

**Response (201 Created)**:

```json
{
  "id": 123
}
```

**Errors**:

- **400 Bad Request**: Table not found, wrong number of values, or type mismatch
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: Values must be provided in the same order as table fields.

### GET /tables/{name}/records

**Description**: Queries records with filtering and pagination.

**Path Parameters**:

- `name` (string): Table name

**Query Parameters**:

- `limit` (integer, optional): Maximum number of records to return
- `offset` (integer, optional): Number of records to skip
- `{field}` (any, optional): Filter by field value (e.g., `active=true`)

**Response (200 OK)**:

```json
[
  {
    "field1": "value1",
    "field2": "value2",
    ...
  }
]
```

**Errors**:

- **400 Bad Request**: Table not found or invalid query parameters
- **500 Internal Server Error**: Runtime error or channel communication failure

### GET /tables/{name}/records/{id}

**Description**: Reads a record from a table.

**Path Parameters**:

- `name` (string): Table name
- `id` (integer): Record ID

**Response (200 OK)**:

```json
{
  "field1": "value1",
  "field2": "value2",
  ...
}
```

**Errors**:

- **400 Bad Request**: Table not found or invalid record ID
- **404 Not Found**: Record not found
- **500 Internal Server Error**: Runtime error or channel communication failure

### PUT /tables/{name}/records/{id}

**Description**: Fully replaces a record with new values.

**Path Parameters**:

- `name` (string): Table name
- `id` (integer): Record ID

**Request Body**:

```json
{
  "values": [value1, value2, value3, ...]
}
```

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Table not found, invalid record ID, wrong number of values, or type mismatch
- **404 Not Found**: Record not found
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: This replaces the entire record with new values.

### PATCH /tables/{name}/records/{id}

**Description**: Partially updates specific fields of a record.

**Path Parameters**:

- `name` (string): Table name
- `id` (integer): Record ID

**Request Body**:

```json
{
  "updates": {
    "field1": "new_value1",
    "field2": "new_value2"
  }
}
```

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Table not found, invalid record ID, field not found, or type mismatch
- **404 Not Found**: Record not found
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: Only specified fields are updated.

### DELETE /tables/{name}/records/{id}

**Description**: Deletes a record (soft delete).

**Path Parameters**:

- `name` (string): Table name
- `id` (integer): Record ID

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Table not found or invalid record ID
- **404 Not Found**: Record not found
- **500 Internal Server Error**: Runtime error or channel communication failure

**Notes**: This is a soft delete (record is marked as deleted).

### POST /relations

**Description**: Creates a foreign key relation between tables.

**Request Body**:

```json
{
  "from_table": "source_table",
  "from_field": "source_field",
  "to_table": "target_table",
  "to_field": "target_field"
}
```

**Response (201 Created)**:

```json
{
  "id": "relation_id"
}
```

**Errors**:

- **400 Bad Request**: Table not found, field not found, or invalid relation
- **500 Internal Server Error**: Runtime error or channel communication failure

### DELETE /relations/{id}

**Description**: Deletes a foreign key relation.

**Path Parameters**:

- `id` (string): Relation ID

**Response**: 204 No Content

**Errors**:

- **400 Bad Request**: Relation not found
- **500 Internal Server Error**: Runtime error or channel communication failure

### POST /rpc/{name}

**Description**: Executes a registered procedure.

**Path Parameters**:

- `name` (string): Procedure name

**Request Body**:

```json
{
  "param1": "value1",
  "param2": "value2",
  ...
}
```

**Response (200 OK)**:

```json
{
  "result_field": "result_value"
}
```

**Errors**:

- **400 Bad Request**: Procedure not found or invalid parameters
- **404 Not Found**: Procedure not registered
- **500 Internal Server Error**: Procedure execution error or runtime error

**Notes**: Procedures run in isolated transactions with atomic commit.

## Data Types in JSON

### Scalar Types

| Type                      | JSON Representation | Example   |
| ------------------------- | ------------------- | --------- |
| `i8`, `i16`, `i32`, `i64` | Number              | `42`      |
| `u8`, `u16`, `u32`, `u64` | Number              | `42`      |
| `f32`, `f64`              | Number              | `3.14159` |
| `bool`                    | Boolean             | `true`    |
| `string`                  | String              | `"Hello"` |

### Composite Types

| Type         | JSON Representation   | Example           |
| ------------ | --------------------- | ----------------- |
| `3xf32`      | Array of 3 numbers    | `[1.0, 2.0, 3.0]` |
| Custom types | Depends on serializer | Varies            |

## Error Response Format

All error responses follow this format:

```json
{
  "error": "status_code",
  "message": "Human-readable error description"
}
```

### Error Examples

**400 Bad Request**:

```json
{
  "error": "400",
  "message": "Table 'users' not found"
}
```

**404 Not Found**:

```json
{
  "error": "404",
  "message": "Record with ID 123 not found in table 'users'"
}
```

**500 Internal Server Error**:

```json
{
  "error": "500",
  "message": "Runtime error: Failed to allocate buffer"
}
```

## Rate Limiting

The API implements per-tick rate limiting:

- **Maximum requests per tick**: `tickrate * 10` (default: 600 at 60Hz)
- **Queue size**: `tickrate * 100` (default: 6000)
- **Overflow response**: 503 Service Unavailable

**503 Response Example**:

```json
{
  "error": "503",
  "message": "Service Unavailable: Request queue full"
}
```

## Timeout Configuration

- **Request timeout**: 30 seconds (configurable via `DbConfig`)
- **Response timeout**: 30 seconds (configurable via `DbConfig`)

**408 Request Timeout Example**:

```json
{
  "error": "408",
  "message": "Request Timeout"
}
```

## Performance Notes

1. **Read operations** are lock-free and return within 1 microsecond
2. **Write operations** are atomic via copy-on-write and complete within 5 microseconds
3. **Field addition/removal** are blocking operations that rewrite the entire table
4. **Procedure execution** runs in parallel across CPU cores when possible
5. **Query operations** perform full table scans (no indexes)

## Best Practices

1. **Batch operations**: Use procedures for bulk operations instead of individual API calls
2. **Field ordering**: Keep frequently accessed fields together for cache efficiency
3. **Record size**: Aim for record sizes that are multiples of 64 bytes for cache line alignment
4. **Error handling**: Always check HTTP status codes and parse error responses
5. **Rate limiting**: Implement client-side backoff when receiving 503 responses
