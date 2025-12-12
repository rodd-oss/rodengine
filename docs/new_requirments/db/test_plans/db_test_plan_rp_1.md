# Test Plan for task_rp_1 – Define RPC Protocol (JSON‑RPC or Custom) over HTTP

## Overview

Unit tests for defining an RPC protocol over HTTP for the relational in‑memory database. The protocol must handle remote procedure calls, support JSON‑RPC 2.0 or a custom format, and integrate with the database's event loop and transaction system.

## Test Categories

### 1. Protocol Definition & Parsing

| Test Name                     | Description                         | Verifies                            | Edge Cases                         | Assertions                                                |
| ----------------------------- | ----------------------------------- | ----------------------------------- | ---------------------------------- | --------------------------------------------------------- |
| `parse_jsonrpc_valid_request` | Parse valid JSON‑RPC 2.0 request    | Request fields extracted correctly  | All optional fields present        | `method == "createTable"`, `params` matches, `id` present |
| `parse_jsonrpc_notification`  | Parse JSON‑RPC notification (no id) | Notification handled separately     | Missing `id` field                 | `request.id.is_none()`, `request.is_notification()`       |
| `parse_jsonrpc_batch_request` | Parse batch of JSON‑RPC requests    | Multiple requests processed         | Empty batch, mixed notifications   | `batch.len() == N`, each request parsed                   |
| `parse_custom_protocol`       | Parse custom protocol format        | Alternative format supported        | Different field names, binary data | `method` and `params` extracted                           |
| `reject_malformed_json`       | Reject invalid JSON syntax          | Error returned for malformed JSON   | Truncated JSON, invalid UTF‑8      | `parse().is_err()`, error indicates JSON parse failure    |
| `reject_missing_method`       | Reject request without method       | Validation enforces required fields | Empty string, null method          | `parse().is_err()`, error indicates missing method        |
| `reject_invalid_params`       | Reject invalid params type          | Params must be object or array      | String params, number params       | `parse().is_err()`, error indicates invalid params        |

### 2. Request Validation

| Test Name                | Description                              | Verifies                           | Edge Cases                     | Assertions                                    |
| ------------------------ | ---------------------------------------- | ---------------------------------- | ------------------------------ | --------------------------------------------- |
| `validate_method_exists` | Check if method is registered            | Method registry lookup             | Unknown method, case mismatch  | `lookup("unknown").is_none()`                 |
| `validate_params_schema` | Validate params against method signature | Parameter count and types match    | Extra params, missing params   | `validate(params).is_ok()` for correct schema |
| `validate_version`       | Validate JSON‑RPC version                | Version must be "2.0"              | Missing version, wrong version | `parse("1.0").is_err()`                       |
| `validate_id_type`       | Validate id type (string/number/null)    | ID must be string, number, or null | Boolean id, array id           | `parse(id: true).is_err()`                    |

### 3. Response Generation

| Test Name                        | Description                        | Verifies                             | Edge Cases                     | Assertions                                              |
| -------------------------------- | ---------------------------------- | ------------------------------------ | ------------------------------ | ------------------------------------------------------- |
| `generate_success_response`      | Generate success response          | Result field populated, id preserved | Null result, empty object      | `response.result == value`, `response.id == request.id` |
| `generate_error_response`        | Generate error response            | Error object with code/message       | Custom error codes, data field | `response.error.code == -32601`, message present        |
| `generate_notification_response` | Generate response for notification | No response for notifications        | Notification with error        | `response.is_none()`                                    |
| `generate_batch_response`        | Generate batch response            | Responses match request order        | Mixed success/error in batch   | `responses.len() == requests.len()`                     |
| `preserve_jsonrpc_version`       | Response includes "jsonrpc": "2.0" | Version field always present         | Custom protocol responses      | `response.jsonrpc == "2.0"`                             |

### 4. Error Handling

| Test Name                | Description                           | Verifies                       | Edge Cases                       | Assertions                     |
| ------------------------ | ------------------------------------- | ------------------------------ | -------------------------------- | ------------------------------ |
| `method_not_found_error` | Method not found error (-32601)       | Correct error code and message | Method exists but not registered | `error.code == -32601`         |
| `invalid_params_error`   | Invalid params error (-32602)         | Parameter validation failure   | Type mismatch, missing required  | `error.code == -32602`         |
| `parse_error`            | Parse error (-32700)                  | Invalid JSON syntax            | UTF‑8 errors, depth limit        | `error.code == -32700`         |
| `internal_error`         | Internal error (-32603)               | Unhandled panic in handler     | Handler panics, OOM              | `error.code == -32603`         |
| `custom_error_codes`     | Custom error codes (‑32000 to ‑32099) | Application‑specific errors    | Database errors, validation      | `error.code in -32000..-32099` |

### 5. HTTP Integration

| Test Name                 | Description                            | Verifies                      | Edge Cases                   | Assertions                                    |
| ------------------------- | -------------------------------------- | ----------------------------- | ---------------------------- | --------------------------------------------- |
| `http_post_only`          | Accept only POST requests              | GET/PUT/DELETE rejected       | HEAD, OPTIONS requests       | `status == 405 Method Not Allowed`            |
| `content_type_json`       | Require Content‑Type: application/json | Other content types rejected  | Missing header, charset      | `status == 415 Unsupported Media Type`        |
| `request_body_size_limit` | Enforce request size limit             | Large payloads rejected       | Exactly at limit, over limit | `status == 413 Payload Too Large`             |
| `response_content_type`   | Response has correct Content‑Type      | application/json with charset | Error responses also JSON    | `header == "application/json; charset=utf-8"` |
| `cors_headers`            | Include CORS headers if configured     | Access‑Control‑Allow‑Origin   | Preflight requests           | `header present if enabled`                   |

### 6. Handler Dispatch & Execution

| Test Name                        | Description                                 | Verifies                                  | Edge Cases                        | Assertions                               |
| -------------------------------- | ------------------------------------------- | ----------------------------------------- | --------------------------------- | ---------------------------------------- |
| `dispatch_to_registered_handler` | Call registered handler function            | Handler receives correct params           | Async handlers, blocking handlers | `handler_called == true`, `params_match` |
| `handler_return_value`           | Handler return value becomes result         | Value serialized to JSON                  | Complex types, custom serializers | `response.result == serialized_value`    |
| `handler_panic_safety`           | Handler panic caught and converted to error | Internal error returned                   | Double panic in error handling    | `error.code == -32603`                   |
| `transaction_boundary`           | Handler executes within transaction         | Auto‑commit on success, rollback on error | Nested transactions               | `transaction.active_during_handler()`    |
| `concurrent_requests`            | Multiple requests processed concurrently    | Thread safety, no data races              | Same method called simultaneously | `results consistent`, `no deadlocks`     |

### 7. Protocol Extensions & Customization

| Test Name                      | Description                       | Verifies                    | Edge Cases                      | Assertions                     |
| ------------------------------ | --------------------------------- | --------------------------- | ------------------------------- | ------------------------------ |
| `extended_error_data`          | Custom error data field           | Additional error context    | Structured data, nested objects | `error.data == custom_value`   |
| `request_context`              | Pass request context to handlers  | Headers, client IP, auth    | Missing context fields          | `handler_receives_context()`   |
| `custom_serialization`         | Custom param/result serialization | Non‑JSON types supported    | Binary data, custom formats     | `roundtrip_serialization()`    |
| `protocol_version_negotiation` | Multiple protocol versions        | Version detection, fallback | Unsupported version             | `negotiates_correct_version()` |

### 8. Integration with Database Features

| Test Name                     | Description                       | Verifies                        | Edge Cases                | Assertions                                |
| ----------------------------- | --------------------------------- | ------------------------------- | ------------------------- | ----------------------------------------- |
| `schema_operations_via_rpc`   | Call schema methods via RPC       | Create table, add field         | Invalid schema operations | `table_created_successfully()`            |
| `crud_operations_via_rpc`     | Call CRUD methods via RPC         | Insert, read, update, delete    | Concurrent modifications  | `data_consistent_after_operations()`      |
| `procedure_execution_via_rpc` | Execute custom procedures via RPC | Procedure registered and called | Procedure panic, rollback | `procedure_executed_within_transaction()` |
| `event_loop_integration`      | RPC processed in event loop tick  | Handler scheduled, executed     | Tick rate boundaries      | `handler_executed_in_correct_tick()`      |

## Edge Cases to Consider

- Empty batch request (should return empty array response)
- Notification with error (should be silently ignored)
- ID as null (valid for notifications, invalid for requests)
- Extremely nested JSON structures (depth limits)
- Unicode in method names and error messages
- Keep‑alive HTTP connections with multiple RPC requests
- Request ID collision (client‑side responsibility)
- Handler that never returns (timeout handling)
- Mixed protocol versions in batch
- Custom error serialization failures

## Expected Behaviors

- JSON‑RPC 2.0 compliance for standard fields
- Graceful degradation for non‑compliant clients
- All errors returned as valid JSON‑RPC error responses
- No panics reach the HTTP layer
- Thread‑safe handler registration and execution
- Transaction boundaries respected for database operations
- Configurable limits (request size, recursion depth)
- Clear separation between protocol parsing and business logic
