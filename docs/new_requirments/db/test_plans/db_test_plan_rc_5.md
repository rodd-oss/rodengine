# Test Plan for task_rc_5 – GET /table/{name}/records with Pagination

## Overview

Unit tests for exposing a paginated GET endpoint to list all records from a table. Part of relational in‑memory database REST API (Rust).

## Test Categories

### 1. Basic Pagination Functionality

| Test Name                         | Description                      | Verifies                                                  | Edge Cases                              | Assertions                               |
| --------------------------------- | -------------------------------- | --------------------------------------------------------- | --------------------------------------- | ---------------------------------------- |
| `list_records_default_pagination` | GET without pagination params    | Returns first page with default limit                     | Empty table                             | Status 200, empty array, correct headers |
| `list_records_with_limit_offset`  | GET with limit and offset params | Returns correct slice of records                          | Offset beyond total records             | Status 200, correct record count         |
| `list_records_pagination_headers` | Verify response headers          | Includes `X-Total-Count`, `X-Page-Limit`, `X-Page-Offset` | All headers present with correct values | Headers match actual data                |

### 2. Edge Cases & Error Handling

| Test Name                        | Description                                              | Verifies                               | Edge Cases                     | Assertions                               |
| -------------------------------- | -------------------------------------------------------- | -------------------------------------- | ------------------------------ | ---------------------------------------- |
| `list_records_empty_table`       | Table exists but has no records                          | Returns empty array with total count 0 | Zero records                   | `X-Total-Count: 0`, empty JSON array     |
| `list_records_nonexistent_table` | Table doesn't exist                                      | Returns 404 Not Found                  | Invalid table name             | Status 404, error message                |
| `list_records_invalid_limit`     | Limit parameter invalid (e.g., 0, negative, non-numeric) | Returns 400 Bad Request                | Limit=0, limit=-1, limit="abc" | Status 400, validation error             |
| `list_records_invalid_offset`    | Offset parameter invalid (negative, non-numeric)         | Returns 400 Bad Request                | Offset=-1, offset="xyz"        | Status 400, validation error             |
| `list_records_large_offset`      | Offset exceeds total records                             | Returns empty array                    | Offset = total_records + 1     | Status 200, empty array, correct headers |
| `list_records_zero_limit`        | Limit=0 (if allowed)                                     | Returns empty array                    | Edge case for empty result     | Status 200, empty array                  |

### 3. Pagination Boundary Conditions

| Test Name                          | Description                              | Verifies                            | Edge Cases            | Assertions                            |
| ---------------------------------- | ---------------------------------------- | ----------------------------------- | --------------------- | ------------------------------------- |
| `list_records_exact_page_boundary` | Total records divisible by page size     | Last page returns exact limit       | No partial page       | Correct record count on last page     |
| `list_records_partial_last_page`   | Total records not divisible by page size | Last page returns remaining records | Partial final page    | Correct record count (< limit)        |
| `list_records_single_record`       | Table with exactly one record            | Pagination works with minimal data  | Limit > total records | Returns single record                 |
| `list_records_limit_exceeds_total` | Limit larger than total records          | Returns all records                 | One-page result       | Returns all records, offset respected |

### 4. Data Integrity & Ordering

| Test Name                              | Description                                | Verifies                                       | Edge Cases                           | Assertions                      |
| -------------------------------------- | ------------------------------------------ | ---------------------------------------------- | ------------------------------------ | ------------------------------- |
| `list_records_consistent_ordering`     | Records returned in consistent order       | Same order on repeated calls                   | Insertion order or primary key order | Sequence matches expected order |
| `list_records_data_integrity`          | Record data matches stored values          | All field values preserved                     | Complex types, custom types          | JSON matches original insert    |
| `list_records_after_concurrent_writes` | Pagination stable during concurrent writes | ArcSwap buffer swap doesn't corrupt pagination | Writers active during read           | No panics, consistent results   |

### 5. Performance & Concurrency

| Test Name                       | Description                             | Verifies                                      | Edge Cases                    | Assertions                      |
| ------------------------------- | --------------------------------------- | --------------------------------------------- | ----------------------------- | ------------------------------- |
| `list_records_large_dataset`    | Table with many records (e.g., 10k+)    | Pagination handles large datasets efficiently | Memory usage, response time   | No OOM, reasonable latency      |
| `list_records_concurrent_reads` | Multiple concurrent pagination requests | Lock-free reads work correctly                | Parallel readers              | All requests succeed            |
| `list_records_during_write`     | Pagination while write in progress      | Readers see consistent snapshot               | ArcSwap load gives old buffer | No torn reads, consistent state |

### 6. Integration with Storage Layer

| Test Name                           | Description                     | Verifies                               | Edge Cases              | Assertions                       |
| ----------------------------------- | ------------------------------- | -------------------------------------- | ----------------------- | -------------------------------- |
| `list_records_zero_copy_validation` | Data returned references buffer | Zero-copy access maintained            | Reference lifetimes     | No unnecessary cloning           |
| `list_records_buffer_swap_safety`   | Safe during buffer swap         | ArcSwap::load provides consistent view | Mid-swap requests       | No use-after-free, no panics     |
| `list_records_memory_layout`        | Correct field offsets used      | Records decoded from packed buffer     | Tight packing preserved | Field values match binary layout |

## Edge Cases to Consider

- Empty table (zero records)
- Single record table
- Table with exactly page_size records
- Offset = total_records (should return empty)
- Negative/zero/non-numeric pagination parameters
- Very large limit values (should have max cap)
- Concurrent reads during writes
- Buffer reallocation during pagination
- Custom/composite field types in records
- UTF-8 table names with special characters

## Expected Behaviors

- Default pagination: limit=50, offset=0 (configurable)
- `X-Total-Count` header always present
- Records returned in insertion order or defined order
- Zero-copy access where possible
- Lock-free reads via ArcSwap::load
- Proper HTTP status codes (200, 400, 404)
- JSON response format matches record schema
- Memory safe even with concurrent modifications
