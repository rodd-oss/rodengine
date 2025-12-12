# Test Plan for task_rp_2: POST /rpc Endpoint

## Overview

Tests for exposing POST /rpc endpoint that dispatches to registered handlers in the relational in-memory database for online games. RPC complements the primary REST API for schema and data operations as specified in TRD.

## 1. Basic Functionality Tests

**test_rpc_endpoint_registered_handler**

- Verifies: POST /rpc correctly dispatches to a registered handler
- Input: Valid JSON-RPC request with method name matching registered handler
- Expected: Handler executes, returns proper JSON-RPC response
- Edge cases: None

**test_rpc_endpoint_unregistered_handler**

- Verifies: POST /rpc returns error for unregistered handler
- Input: JSON-RPC request with non-existent method name
- Expected: JSON-RPC error response with "Method not found"
- Edge cases: Method name with special characters, empty method name

**test_rpc_endpoint_invalid_json**

- Verifies: POST /rpc handles malformed JSON
- Input: Invalid JSON body
- Expected: HTTP 400 Bad Request or JSON-RPC parse error
- Edge cases: Empty body, truncated JSON, invalid UTF-8

## 2. Parameter Validation Tests

**test_rpc_params_validation**

- Verifies: Handler receives and validates parameters correctly
- Input: JSON-RPC request with params object matching handler signature
- Expected: Handler processes params, returns result
- Edge cases: Missing required params, wrong param types, extra params

**test_rpc_params_positional_vs_named**

- Verifies: Support for both positional and named parameters
- Input: JSON-RPC request with params as array (positional) and object (named)
- Expected: Both formats work correctly
- Edge cases: Mixed formats, empty params

## 3. Concurrency & Thread Safety Tests

**test_rpc_concurrent_requests**

- Verifies: Multiple concurrent RPC requests don't interfere
- Input: Multiple threads sending simultaneous RPC requests
- Expected: All requests complete successfully, responses match
- Edge cases: Same handler called concurrently, different handlers concurrently

**test_rpc_handler_panic_safety**

- Verifies: Handler panic doesn't crash server
- Input: RPC request to handler that panics
- Expected: JSON-RPC error response, server continues running
- Edge cases: Panic with message, unwrap panic, assertion failure

## 4. Integration & State Tests

**test_rpc_handler_database_access**

- Verifies: RPC handlers can access database state
- Input: RPC request that reads/writes to database tables
- Expected: Handler executes within database transaction context
- Edge cases: Concurrent database modifications, transaction rollback

**test_rpc_handler_returns_complex_types**

- Verifies: Handlers can return complex types (structs, arrays)
- Input: RPC request expecting structured response
- Expected: Complex data serialized to JSON correctly
- Edge cases: Nested structures, custom types, circular references

## 5. Performance & Edge Cases

**test_rpc_large_payload**

- Verifies: Handles large request/response payloads
- Input: JSON-RPC with large params object or returning large data
- Expected: Request processes successfully within reasonable time
- Edge cases: Memory limits, timeout handling

**test_rpc_handler_registration_dynamic**

- Verifies: Handlers can be registered/deregistered at runtime
- Input: Register handler, make RPC call, deregister handler, make same call
- Expected: First call succeeds, second returns "Method not found"
- Edge cases: Race condition during registration/deregistration

**test_rpc_batch_requests**

- Verifies: Support for JSON-RPC batch requests (if implemented)
- Input: Array of multiple JSON-RPC requests in single POST
- Expected: Array of responses in same order
- Edge cases: Mixed success/error responses, empty batch

## 6. Security & Validation Tests

**test_rpc_method_name_injection**

- Verifies: Method names are validated against injection attacks
- Input: Method names with path traversal, SQL injection patterns
- Expected: Method not found or validation error
- Edge cases: Unicode trickery, null bytes, extremely long names

**test_rpc_rate_limiting**

- Verifies: Rate limiting if implemented (future consideration)
- Input: Rapid sequence of RPC requests
- Expected: Later requests throttled or rejected
- Edge cases: Different clients, burst patterns

## Assertions & Expected Behaviors

- HTTP status codes: 200 for valid JSON-RPC, 400 for invalid JSON
- JSON-RPC 2.0 compliance: `jsonrpc: "2.0"`, `id` preserved
- Error objects: `code`, `message`, optional `data`
- Handler execution: Within database transaction context
- Thread safety: No data races, proper synchronization
- Memory safety: No leaks, proper cleanup after handler panic
