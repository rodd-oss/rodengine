# Test Plan for Task EL-2: Handler Registration System

**Task ID**: task_el_2  
**Description**: Register handlers (API request processors, custom procedures) to be invoked each tick.  
**Context**: Part of relational in-memory database for online games, written in Rust. Event loop runs at 15–120 Hz tickrate.

**TRD Alignment Notes**:

- **Tickrate**: TRD §4 specifies configurable 15–120 Hz event loop for real‑time game integration
- **Concurrency**: Must use ArcSwap (§4) for lock‑free reads; thread safety assertion already references ArcSwap pattern
- **REST API**: Handler system must integrate with REST endpoints (§5) for external command processing
- **Transaction Context**: Custom procedure handlers should respect transaction boundaries (§3) when modifying data
- **Performance**: Handler registration/invocation overhead must respect event loop timing constraints (§4)

## 1. Basic Registration Tests

### `test_register_single_handler`

- **Verifies**: Single handler can be registered and invoked
- **Assertions**: Handler ID returned, handler invoked during tick, execution count increments
- **Edge cases**: Empty handler list before registration

### `test_register_multiple_handlers`

- **Verifies**: Multiple handlers can be registered and invoked in order
- **Assertions**: All handlers executed, execution order matches registration order (FIFO)
- **Edge cases**: Varying handler counts (2, 10, 100)

### `test_handler_registration_returns_id`

- **Verifies**: Registration returns unique handler ID
- **Assertions**: IDs are non-zero, unique across registrations, persistent until unregistration
- **Edge cases**: ID reuse after unregistration

## 2. Handler Invocation Tests

### `test_handlers_invoked_each_tick`

- **Verifies**: All registered handlers are called during each tick
- **Assertions**: Handler execution count equals tick count, no skipped invocations
- **Edge cases**: Multiple consecutive ticks, varying tick rates

### `test_handler_execution_order`

- **Verifies**: Handlers execute in registration order (FIFO)
- **Assertions**: Execution sequence matches registration sequence
- **Edge cases**: Mixed priority levels if supported

### `test_handler_receives_tick_context`

- **Verifies**: Handlers receive tick context (tick number, timestamp, etc.)
- **Assertions**: Context contains valid tick number, monotonic timestamp, loop state
- **Edge cases**: First tick (tick 0/1), timestamp wraparound

## 3. Edge Cases & Error Handling

### `test_duplicate_handler_registration`

- **Verifies**: Duplicate registrations are handled appropriately
- **Assertions**: Either reject duplicate or allow with new ID (specify policy)
- **Edge cases**: Same closure registered twice, same function pointer

### `test_register_handler_with_invalid_priority`

- **Verifies**: Validation of handler priority values
- **Assertions**: Invalid priorities rejected, valid priorities accepted
- **Edge cases**: Negative priorities, out-of-range values

### `test_handler_panic_does_not_crash_loop`

- **Verifies**: Handler panics are caught and don't crash event loop
- **Assertions**: Loop continues after handler panic, other handlers still execute
- **Edge cases**: Nested panics, panic during cleanup

### `test_handler_timeout_protection`

- **Verifies**: Handlers that hang/take too long are terminated
- **Assertions**: Long-running handlers don't block tick completion
- **Edge cases**: Infinite loops, blocking I/O in handlers

## 4. Handler Removal & Management

### `test_unregister_handler_by_id`

- **Verifies**: Handlers can be removed by ID
- **Assertions**: Handler stops being invoked, memory freed, ID becomes invalid
- **Edge cases**: Unregister during handler execution

### `test_unregister_nonexistent_handler`

- **Verifies**: Removing non-existent handler returns error
- **Assertions**: Returns Err/None, no side effects, invalid ID handling
- **Edge cases**: Previously valid but now unregistered ID

### `test_unregister_during_execution`

- **Verifies**: Handler can be unregistered while running
- **Assertions**: Current execution completes, handler not invoked next tick
- **Edge cases**: Unregister from within the handler itself

### `test_clear_all_handlers`

- **Verifies**: All handlers can be cleared at once
- **Assertions**: No handlers invoked after clear, memory reclaimed
- **Edge cases**: Clear during tick execution, empty handler list

## 5. Concurrent Registration Tests

### `test_concurrent_handler_registration`

- **Verifies**: Thread-safe registration from multiple threads
- **Assertions**: No data races, all handlers registered correctly
- **Edge cases**: High contention (many threads registering simultaneously)

### `test_concurrent_unregistration`

- **Verifies**: Thread-safe removal during handler execution
- **Assertions**: No deadlocks, handlers properly cleaned up
- **Edge cases**: Unregister while handler executing on another thread

### `test_handler_registration_during_tick`

- **Verifies**: Handlers can be registered while tick is executing
- **Assertions**: New handler invoked on next tick, not current tick
- **Edge cases**: Registration from within executing handler

## 6. Handler Types & Categories

### `test_api_request_handler_registration`

- **Verifies**: API request processors can be registered
- **Assertions**: API handlers receive request context, can return responses
- **Edge cases**: HTTP method routing, request parsing errors

### `test_custom_procedure_handler_registration`

- **Verifies**: Custom procedures can be registered
- **Assertions**: Procedure handlers execute within transaction context
- **Edge cases**: Procedure panics trigger rollback

### `test_handler_priority_ordering`

- **Verifies**: Handlers with different priorities execute in correct order
- **Assertions**: Higher priority executes before lower priority
- **Edge cases**: Equal priorities, priority inversion scenarios

### `test_handler_categories_separation`

- **Verifies**: Different handler types (API vs procedures) are managed separately
- **Assertions**: Separate registration APIs, independent lifecycle management
- **Edge cases**: Cross-category dependencies

## 7. Performance & Resource Tests

### `test_handler_registration_scalability`

- **Verifies**: System handles large number of handlers (1000+)
- **Assertions**: Registration time scales appropriately, memory usage reasonable
- **Edge cases**: Memory pressure, registration bottleneck

### `test_handler_execution_overhead`

- **Verifies**: Measure overhead of handler invocation per tick
- **Assertions**: Overhead minimal (< X microseconds per handler)
- **Edge cases**: Empty handlers, many small handlers

### `test_memory_leak_prevention`

- **Verifies**: Unregistered handlers don't leak memory
- **Assertions**: Memory usage returns to baseline after unregistration
- **Edge cases**: Cyclic references, closure captures

## 8. Integration with Event Loop

### `test_handler_registration_before_loop_start`

- **Verifies**: Handlers registered before loop starts are invoked
- **Assertions**: Pre-registered handlers execute from first tick
- **Edge cases**: Registration during loop initialization

### `test_handler_registration_after_loop_start`

- **Verifies**: Handlers can be added while loop is running
- **Assertions**: New handlers invoked starting next tick
- **Edge cases**: Registration during tick boundary

### `test_handler_stop_loop_propagation`

- **Verifies**: Handlers can request loop stop/restart
- **Assertions**: Stop signal propagated, loop terminates cleanly
- **Edge cases**: Multiple stop requests, restart after stop

## Key Assertions & Expected Behaviors

1. **Handler IDs**: Must be unique and persistent for handler lifetime
2. **Thread Safety**: Registration must be atomic and thread-safe (ArcSwap pattern)
3. **Execution Order**: Handlers must execute in deterministic order (registration order or priority-based)
4. **Error Isolation**: Handler failures must not affect other handlers or event loop
5. **Memory Management**: Proper cleanup when handlers removed, no leaks
6. **Tick Context**: Must include tick number, timestamp, and relevant state
7. **Async Support**: System should support both synchronous and asynchronous handlers
8. **Performance**: Registration/unregistration O(1) or O(log n) complexity

## Edge Cases to Consider

- Handler that registers another handler during its execution
- Handler that unregisters itself during execution
- Rapid registration/unregistration cycles (stress test)
- Handlers with extremely long execution times (> tick duration)
- Handlers that panic or return errors
- Maximum handler count limits (if any)
- Handler priority inversion scenarios
- Memory pressure during high-frequency handler registration
- Tick rate changes affecting handler scheduling
- Database state changes during handler execution
- Concurrent modification of shared handler data
