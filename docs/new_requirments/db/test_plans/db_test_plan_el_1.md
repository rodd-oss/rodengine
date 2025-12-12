# Test Plan: Main Database Loop (task_el_1)

## Task Description

Implement main database loop that runs at configurable tickrate (15–120 Hz).

## Context

Part of relational in-memory database for online games, written in Rust. Loop executes handlers (API calls, custom procedures) each tick.

## TRD Alignment Notes

- **Tickrate**: TRD §4 specifies configurable 15–120 Hz event loop for real‑time game integration
- **Concurrency**: Must use ArcSwap (§4) for lock‑free reads; test case 7 already covers this
- **REST API**: Loop must integrate with REST endpoints (§5) for external commands
- **Performance**: Loop timing affects overall database responsiveness (§4)
- **Atomicity**: Handlers should respect transaction boundaries (§3) when modifying data

## Test Cases

### 1. **test_tickrate_configuration**

**Verifies**: Loop accepts and respects tickrate configuration within valid range (15-120 Hz)
**Edge cases**:

- Minimum boundary (15 Hz)
- Maximum boundary (120 Hz)
- Invalid values (<15 Hz, >120 Hz, 0 Hz, negative values)
- Non-integer frequencies
  **Assertions**:
- Loop runs at configured frequency (±5% tolerance)
- Invalid configurations return appropriate error
- Default tickrate falls within valid range

### 2. **test_loop_start_stop**

**Verifies**: Loop can be cleanly started and stopped
**Edge cases**:

- Multiple start attempts
- Stop before start
- Stop during execution
- Restart after stop
  **Assertions**:
- Loop starts without panic
- Loop stops within reasonable time
- Resources cleaned up after stop
- No memory leaks on restart

### 3. **test_handler_registration**

**Verifies**: Handlers can be registered and invoked each tick
**Edge cases**:

- Register/remove handlers during loop execution
- Multiple handlers with different priorities
- Handler panics (should not crash loop)
- Empty handler list
  **Assertions**:
- Handlers called in registration order/priority
- Handler removal takes effect immediately
- Panicking handlers logged but loop continues

### 4. **test_tick_execution_timing**

**Verifies**: Loop maintains consistent tick timing
**Edge cases**:

- Handler execution time exceeds tick duration
- System load spikes
- Very fast (120 Hz) vs slow (15 Hz) tickrates
  **Assertions**:
- Average tick duration matches configured period
- Jitter within acceptable bounds (<10% of period)
- Loop compensates for overruns (skip or delay)

### 5. **test_concurrent_access**

**Verifies**: Loop safely handles concurrent operations
**Edge cases**:

- Register handlers while loop running
- Modify tickrate while running
- Concurrent stop requests
  **Assertions**:
- Thread-safe handler registration
- Atomic tickrate changes
- Graceful shutdown with pending operations

### 6. **test_resource_usage**

**Verifies**: Loop has predictable resource consumption
**Edge cases**:

- Long-running loops (hours)
- Many registered handlers
- High frequency (120 Hz) operation
  **Assertions**:
- CPU usage scales with tickrate
- Memory stable over time
- No unbounded resource growth

### 7. **test_integration_with_arcswap**

**Verifies**: Loop works with ArcSwap buffer system
**Edge cases**:

- Buffer swaps during tick execution
- Concurrent reads/writes from handlers
- Transaction rollbacks
  **Assertions**:
- Handlers see consistent buffer state per tick
- No data races with ArcSwap operations
- Atomic operations complete within tick

### 8. **test_error_handling**

**Verifies**: Loop handles errors gracefully
**Edge cases**:

- Handler returns error
- System call failures (timer, thread)
- Out of memory conditions
  **Assertions**:
- Errors logged but loop continues
- Critical failures trigger controlled shutdown
- Error statistics tracked per handler

### 9. **test_performance_metrics**

**Verifies**: Loop provides performance telemetry
**Edge cases**:

- Empty ticks (no work)
- Saturated ticks (max work)
- Varying workload patterns
  **Assertions**:
- Tick duration metrics available
- Handler execution times tracked
- Throughput calculations accurate

### 10. **test_configuration_persistence**

**Verifies**: Loop configuration can be saved/restored
**Edge cases**:

- Partial configuration
- Invalid saved state
- Version migration
  **Assertions**:
- Configuration serializable
- Restored loop behaves identically
- Backward compatibility maintained
