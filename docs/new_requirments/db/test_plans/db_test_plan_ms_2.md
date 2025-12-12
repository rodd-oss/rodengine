# Test Plan for Task MS-2: Bounds Checking for Record Indices and Field Indices

## Context

- In-memory relational database with `Vec<u8>` storage.
- Records tightly packed; field access via offsets.
- Need to validate record and field indices before unsafe pointer operations.

## Assumptions

- Table has `record_count()` and `field_count()` methods.
- Access functions: `get_record(record_index) -> Option<&[u8]>` and `get_field(record_index, field_index) -> Option<&T>`.
- Bounds checking may return `Result<_, OutOfBoundsError>` or panic via `assert!`. We'll test both.

## Test Categories

### 1. Record Index Bounds

**test_record_index_valid**  
Verifies that a valid record index returns a non‑null reference/record.

- Setup: Table with N records (N > 0).
- Action: Access record at index `0`, `N-1`, and a middle index.
- Assert: Record slice length equals record size; data matches expected.

**test_record_index_out_of_bounds**  
Verifies that an out‑of‑bounds record index triggers an error/panic.

- Cases:
  - `record_index == record_count` (one past last)
  - `record_index > record_count`
  - `record_index == usize::MAX` (overflow)
- Expect: `None` / `Err(OutOfBounds)` / panic.

**test_record_index_empty_table**  
Verifies behavior when table has zero records.

- Setup: Table with zero records (capacity > 0).
- Action: Access any index (e.g., 0).
- Expect: Out‑of‑bounds error.

### 2. Field Index Bounds

**test_field_index_valid**  
Verifies that a valid field index returns correct field reference.

- Setup: Table with M fields (M > 0).
- Action: Access field at index `0`, `M-1`, and a middle index for a given valid record.
- Assert: Field value matches expected type and value.

**test_field_index_out_of_bounds**  
Verifies that an out‑of‑bounds field index triggers error/panic.

- Cases:
  - `field_index == field_count`
  - `field_index > field_count`
  - `field_index == usize::MAX`
- Expect: Error.

**test_field_index_zero_fields**  
Edge case: table with zero fields (should be prevented by schema? but test defensively).

- Setup: Table with zero fields (record size = 0).
- Action: Access field index 0.
- Expect: Out‑of‑bounds error.

### 3. Combined Bounds

**test_valid_record_invalid_field**  
Valid record index but invalid field index.

- Setup: Table with at least one record and at least one field.
- Action: Access `(record_index=0, field_index=field_count)`.
- Expect: Field‑out‑of‑bounds error.

**test_invalid_record_valid_field**  
Invalid record index but valid field index.

- Setup: As above.
- Action: Access `(record_index=record_count, field_index=0)`.
- Expect: Record‑out‑of‑bounds error.

**test_both_indices_out_of_bounds**  
Both indices invalid.

- Action: Access `(record_index=record_count, field_index=field_count)`.
- Expect: Record‑out‑of‑bounds error (first check).

### 4. Edge Cases & Stress

**test_index_after_record_deletion**  
If deletion creates a “hole” (future feature), ensure bounds still refer to logical indices.

- Setup: Insert N records, delete middle record.
- Action: Access record at index `deleted_index` (should be invalid).
- Expect: Out‑of‑bounds or maybe `None`.
- Note: May be out of scope for current task.

**test_large_indices_near_capacity**  
Table near maximum capacity (e.g., `usize::MAX / record_size`).

- Setup: Allocate maximum possible records (may be limited by memory).
- Action: Access last valid record.
- Expect: Success.
- Action: Access one past last.
- Expect: Error.

**test_negative_indices_if_signed**  
If API uses signed integers (i32), negative indices should be caught.

- Expect: Compile‑time error or runtime bounds check.

### 5. Integration with Unsafe Operations

**test_bounds_check_before_unsafe_access**  
Ensure that bounds checking occurs before any unsafe pointer arithmetic.

- Can be verified via code review; but we can write a test that passes invalid indices and confirms no undefined behavior (e.g., using `catch_unwind` to see if panic occurs before unsafe block).
- Use `#[should_panic]` or `Result` testing.

### 6. Error Messages & Types

**test_error_type_includes_context**  
Out‑of‑bounds error should contain which index failed (record vs field) and the allowed range.

- Assert: Error variant `OutOfBounds::RecordIndex { index, max }` or `OutOfBounds::FieldIndex { index, max }`.
- Useful for debugging.

### 7. Performance Impact

**test_bounds_check_not_skipped_in_release**  
Ensure bounds checks are present in release builds (if safety is required).

- Can be done via `#[cfg(debug_assertions)]` attribute; but we may want checks always.
- Write test that calls with invalid index in release mode and expects panic/error.

## Implementation Notes

- Use `cargo test` with `--test-threads=1` for panic tests.
- Use `#[should_panic]` if panic is expected.
- If returning `Result`, use `assert!(matches!(result, Err(OutOfBounds::RecordIndex { .. })))`.
- Mock storage buffer with dummy data to avoid actual allocations where possible.

## Test Coverage Goals

- All bounds‑checking code paths (record, field, combined).
- Edge cases (empty, zero fields, max indices).
- Error propagation.
- No unsafe access without prior validation.

## Dependencies

- Need Table struct with fields and storage (tasks SL‑1 through SL‑5).
- Need field offset calculation (task SL‑3).
- Need record size (task SL‑3).
- Need basic record write/read (tasks SL‑4, SL‑5).
