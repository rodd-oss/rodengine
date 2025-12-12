# Test Plan for task_sl_1 – Vec<u8> Storage Buffer

## Overview

Unit tests for implementing a Vec<u8> storage buffer for a table that can be initialized with capacity. Part of relational in‑memory database for online games (Rust).

**Total Tests**: 22 (16 original + 6 code review validation tests)

**Implementation Status**: ✅ Complete with all code review fixes applied

**Key Safety Fixes**:

1. `new_zeroed()`: Fixed memory safety issue (was using unsafe `set_len` before `write_bytes`)
2. `as_slice()`/`as_mut_slice()`: Now return only initialized bytes (was exposing uninitialized memory)
3. `pointer_cast`: Fixed alignment issue (now uses `read_unaligned()`)
4. Added missing `new()` constructor for API consistency
5. Enhanced safety documentation for `as_ptr()` and `as_mut_ptr()`

## Test Categories

### 1. Basic Initialization

| Test Name           | Description                                 | Verifies                                              | Edge Cases                     | Assertions                                       |
| ------------------- | ------------------------------------------- | ----------------------------------------------------- | ------------------------------ | ------------------------------------------------ |
| `new_with_capacity` | Create buffer with given capacity           | `capacity()` returns requested capacity; `len() == 0` | Positive capacity (e.g., 1024) | `buffer.capacity() == 1024`, `buffer.is_empty()` |
| `default_capacity`  | Create buffer via `default()` (if provided) | Default capacity is 0 or a predefined minimum         | No explicit capacity           | `buffer.capacity() >= 0`                         |

### 2. Edge Capacity Values

| Test Name           | Description                                   | Verifies                                             | Edge Cases               | Assertions                                             |
| ------------------- | --------------------------------------------- | ---------------------------------------------------- | ------------------------ | ------------------------------------------------------ |
| `zero_capacity`     | Capacity = 0                                  | Buffer is valid and can be grown                     | Zero‑capacity allocation | `buffer.capacity() == 0`; `buffer.reserve(1)` succeeds |
| `max_capacity`      | Capacity near `usize::MAX` (if fallible init) | Allocation fails gracefully (e.g., returns `Result`) | OOM‑like scenarios       | `TableBuffer::try_with_capacity(usize::MAX).is_err()`  |
| `capacity_overflow` | Adding record size that overflows `usize`     | `reserve` panics or returns error                    | Arithmetic overflow      | `buffer.reserve(usize::MAX)` panics                    |

### 3. Memory Properties

| Test Name           | Description                                | Verifies                                                             | Edge Cases           | Assertions                                      |
| ------------------- | ------------------------------------------ | -------------------------------------------------------------------- | -------------------- | ----------------------------------------------- | --- | -------- |
| `contiguous_memory` | Buffer is a single contiguous block        | `as_ptr()` gives valid pointer; slice matches pointer range          | After reallocations  | `slice.as_ptr() == buffer.as_ptr()`             |
| `alignment`         | Underlying pointer meets minimum alignment | Alignment ≥ `align_of::<u8>()` (1) and optionally cache‑line aligned | Different capacities | `buffer.as_ptr() as usize % 64 == 0` (optional) |
| `zeroed_memory`     | Memory is zero‑initialized if specified    | All bytes are zero after creation                                    | Zero capacity        | `buffer.as_slice().iter().all(                  | &b  | b == 0)` |

### 4. Capacity Management

| Test Name                  | Description                           | Verifies                                       | Edge Cases                  | Assertions                          |
| -------------------------- | ------------------------------------- | ---------------------------------------------- | --------------------------- | ----------------------------------- |
| `reserve_exact`            | Reserve exact additional capacity     | Capacity increases by exactly requested amount | Already sufficient capacity | `before + additional == after`      |
| `shrink_to_fit`            | Reduce capacity to fit current length | Capacity ≤ length after shrink                 | Empty buffer                | `buffer.capacity() == buffer.len()` |
| `clear_preserves_capacity` | Clear does not affect capacity        | `capacity()` unchanged after `clear()`         | Full buffer                 | `cap_before == cap_after`           |

### 5. Concurrency & Safety

| Test Name          | Description                             | Verifies                   | Edge Cases       | Assertions                               |
| ------------------ | --------------------------------------- | -------------------------- | ---------------- | ---------------------------------------- |
| `send_sync`        | Buffer can be shared across threads     | `TableBuffer: Send + Sync` | Wrapped in `Arc` | `std::thread::spawn` passes              |
| `atomic_reference` | Buffer can be placed in `Arc`/`ArcSwap` | Reference counting works   | Concurrent loads | `Arc::strong_count(&arc) == 1` initially |

### 6. Integration with Future Tasks

| Test Name                | Description                               | Verifies                                                   | Edge Cases                             | Assertions                                                                          |
| ------------------------ | ----------------------------------------- | ---------------------------------------------------------- | -------------------------------------- | ----------------------------------------------------------------------------------- |
| `pointer_cast`           | Raw pointer can be cast to other types    | `as_ptr()` can be cast to `*const u32` after writing bytes | Properly aligned casts                 | `unsafe { (buffer.as_ptr() as *const u32).read_unaligned() }` returns correct value |
| `pointer_cast_unaligned` | Raw pointer casting with unaligned access | `as_ptr()` can be cast with `read_unaligned()`             | Offsets 1 and 3 (definitely unaligned) | `unsafe { (buffer.as_ptr().add(1) as *const u32).read_unaligned() }` works          |

### 7. Code Review Validation Tests

Tests added after code review to validate fixes for memory safety and API correctness issues.

| Test Name                                 | Description                                                                 | Verifies                                                                 | Edge Cases                     | Assertions                                                  |
| ----------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------ | ----------------------------------------------------------- |
| `new_zeroed_memory_safety`                | `new_zeroed` properly initializes all bytes (no unsafe set_len/write_bytes) | Memory safety fix: uses `vec![0; capacity]` instead of unsafe operations | Zero capacity                  | `buffer.len() == capacity`, all bytes zero                  |
| `as_slice_returns_only_initialized_bytes` | `as_slice()` returns only initialized bytes (not entire capacity)           | API safety fix: prevents UB from reading uninitialized memory            | Empty buffer, partially filled | `as_slice().len() == len()`, not `capacity()`               |
| `new_constructor`                         | `new()` constructor exists for API consistency                              | Added missing constructor                                                | Comparison with `default()`    | `TableBuffer::new()` equivalent to `TableBuffer::default()` |
| `safety_invariants_as_ptr`                | Safety invariants for `as_ptr()` documented and testable                    | Pointer validity, proper usage patterns                                  | After reallocation             | Pointer non-null, can read initialized bytes                |
| `safety_invariants_as_mut_ptr`            | Safety invariants for `as_mut_ptr()` documented and testable                | Exclusive access requirements, length updates                            | Writing through pointer        | Must update `len()` after writing through raw pointer       |

## Edge Cases to Consider

- Empty capacity (0 bytes) – buffer must still be usable.
- Capacity overflow when adding record size (should panic or error).
- Reallocation after exceeding capacity (Vec’s growth strategy).
- Alignment of underlying memory for future field‑type casts.
- Thread‑safe sharing (Send/Sync) for later ArcSwap usage.
- Zero‑copy guarantees – buffer must remain contiguous.
- **Memory safety**: `as_slice()` must not expose uninitialized memory.
- **API consistency**: All standard constructors should be available (`new()`, `new_with_capacity()`, etc.).
- **Unaligned access**: Pointer casting must handle misaligned accesses safely.

## Expected Behaviors

- `capacity()` always returns the allocated capacity (≥ requested).
- `len()` reflects number of bytes written (initially 0).
- Buffer is contiguous (`as_ptr()` gives pointer to start of allocated range).
- `as_slice()` returns only initialized bytes (up to `len()`), not entire capacity.
- `new_zeroed()` safely initializes all bytes to zero without unsafe operations.
- `new()` constructor available for API consistency.
- Pointer casting uses `read_unaligned()` for potentially misaligned accesses.
- All operations are safe (no UB) within the wrapper’s API.
- Safety invariants are documented and testable for `as_ptr()` and `as_mut_ptr()`.
