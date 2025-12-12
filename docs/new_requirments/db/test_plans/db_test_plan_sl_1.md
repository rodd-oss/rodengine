# Test Plan for task_sl_1 – Vec<u8> Storage Buffer

## Overview

Unit tests for implementing a Vec<u8> storage buffer for a table that can be initialized with capacity. Part of relational in‑memory database for online games (Rust).

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

| Test Name                  | Description                                | Verifies                                                             | Edge Cases           | Assertions                                      |
| -------------------------- | ------------------------------------------ | -------------------------------------------------------------------- | -------------------- | ----------------------------------------------- | --- | -------- |
| `contiguous_memory`        | Buffer is a single contiguous block        | `as_slice()` yields contiguous bytes                                 | After reallocations  | `buffer.as_slice().len() == buffer.capacity()`  |
| `alignment`                | Underlying pointer meets minimum alignment | Alignment ≥ `align_of::<u8>()` (1) and optionally cache‑line aligned | Different capacities | `buffer.as_ptr() as usize % 64 == 0` (optional) |
| `zeroed_memory` (optional) | Memory is zero‑initialized if specified    | All bytes are zero after creation                                    | Zero capacity        | `buffer.as_slice().iter().all(                  | &b  | b == 0)` |

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

| Test Name      | Description                            | Verifies                                                   | Edge Cases                                 | Assertions                                                   |
| -------------- | -------------------------------------- | ---------------------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------ |
| `pointer_cast` | Raw pointer can be cast to other types | `as_ptr()` can be cast to `*const u32` after writing bytes | Misaligned casts (should be handled later) | `unsafe { *(buffer.as_ptr() as *const u32) }` does not panic |

## Edge Cases to Consider

- Empty capacity (0 bytes) – buffer must still be usable.
- Capacity overflow when adding record size (should panic or error).
- Reallocation after exceeding capacity (Vec’s growth strategy).
- Alignment of underlying memory for future field‑type casts.
- Thread‑safe sharing (Send/Sync) for later ArcSwap usage.
- Zero‑copy guarantees – buffer must remain contiguous.

## Expected Behaviors

- `capacity()` always returns the allocated capacity (≥ requested).
- `len()` reflects number of bytes written (initially 0).
- Buffer is contiguous (`as_slice()` yields whole allocated range).
- No unnecessary copying or padding introduced.
- All operations are safe (no UB) within the wrapper’s API.
