# Test Plan for task_ce_2: Ensure table storage buffer is allocated as single contiguous Vec<u8>

## Overview

Tests to verify that table storage buffers are allocated as single contiguous `Vec<u8>` blocks for cache efficiency and zero-copy access in the relational in-memory database.

## Test Suite

### 1. `test_buffer_is_contiguous_vec`

**Description**: Verify that the table storage buffer is indeed a single `Vec<u8>` instance.
**Verifies**: The buffer type is `Vec<u8>` and not a collection of smaller buffers.
**Assertions**:

- Buffer is instance of `Vec<u8>`
- `buffer.as_ptr()` returns a single pointer to contiguous memory
- `buffer.len()` equals total allocated bytes

### 2. `test_buffer_contiguity_across_growth`

**Description**: Test that buffer remains contiguous when capacity grows.
**Verifies**: Reallocation preserves single contiguous allocation.
**Edge Cases**:

- Initial capacity vs actual size
- Multiple growth operations
- Large capacity expansions
  **Assertions**:
- After `reserve()` or `reserve_exact()`, buffer remains `Vec<u8>`
- Memory address may change after reallocation, but buffer remains single contiguous block
- `capacity()` increases appropriately

### 3. `test_buffer_alignment_and_packing`

**Description**: Verify buffer supports tight packing requirements.
**Verifies**: Buffer can be cast to field types without padding issues.
**Edge Cases**:

- Different field alignments
- Mixed-size fields
- Custom composite types
  **Assertions**:
- Buffer pointer alignment meets requirements for all field types
- Record offsets within buffer are properly aligned
- No implicit padding between fields or records

### 4. `test_buffer_zero_copy_access`

**Description**: Test that buffer supports zero-copy field access.
**Verifies**: References to fields point directly into buffer memory.
**Assertions**:

- Field accessors return `&T` references
- Reference addresses are within buffer memory range
- Multiple concurrent references to different fields are valid

### 5. `test_buffer_capacity_vs_size`

**Description**: Verify capacity management respects contiguous allocation.
**Verifies**: `capacity()` and `len()` work correctly with contiguous buffer.
**Edge Cases**:

- Empty buffer (capacity > 0, len = 0)
- Full buffer (capacity = len)
- Overallocation strategies
  **Assertions**:
- `capacity() >= len()` always true
- Buffer remains contiguous when shrinking with `shrink_to_fit()`
- `try_reserve()` preserves contiguity

### 6. `test_buffer_memory_safety`

**Description**: Ensure unsafe operations on buffer are memory safe.
**Verifies**: Pointer arithmetic stays within bounds.
**Edge Cases**:

- Edge of buffer access
- Invalid record indices
- Field offset calculations
  **Assertions**:
- All pointer casts use valid offsets
- `buffer.as_ptr().add(offset)` stays within `[ptr, ptr + len)`
- No out-of-bounds access even with maximum record count

### 7. `test_buffer_concurrent_access`

**Description**: Test buffer works with ArcSwap for concurrent access.
**Verifies**: Contiguous buffer can be atomically swapped.
**Edge Cases**:

- Concurrent reads during buffer swap
- Buffer replacement with different capacity
- Old buffer cleanup
  **Assertions**:
- `ArcSwap::store()` accepts `Arc<Vec<u8>>`
- Loaded buffer remains contiguous
- Old buffers remain valid for existing references

### 8. `test_buffer_performance_characteristics`

**Description**: Verify buffer meets cache efficiency requirements.
**Verifies**: Memory layout supports cache-friendly access patterns.
**Edge Cases**:

- Cache line alignment (64 bytes)
- Sequential vs random access
- Prefetching behavior
  **Assertions**:
- Record stride is predictable
- Field access within same cache line when possible
- Buffer supports SIMD operations where applicable

## Edge Cases to Consider

1. **Empty tables**: Buffer with capacity but zero records
2. **Maximum capacity**: Near `usize::MAX` allocations
3. **Alignment requirements**: Different architectures (x86, ARM)
4. **Reallocation strategies**: Exponential vs linear growth
5. **Memory fragmentation**: Ensuring single allocation avoids fragmentation
6. **Platform-specific behavior**: Different `Vec<u8>` implementations
7. **Zero-sized types**: Handling empty records or zero-sized fields
8. **Concurrent modifications**: Buffer replacement while iterating

## Expected Behaviors

- Buffer is always a single `Vec<u8>` allocation
- No linked lists, arrays of buffers, or fragmented allocations
- Memory addresses are contiguous (no gaps)
- Supports all required field types and alignments
- Compatible with unsafe pointer casting for zero-copy access
- Works with ArcSwap for atomic buffer replacement
- Maintains cache efficiency through tight packing
