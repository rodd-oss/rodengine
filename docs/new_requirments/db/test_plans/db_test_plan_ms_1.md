# Test Plan for `task_ms_1`: Validate field offsets and sizes

**Task ID**: task_ms_1  
**Description**: Validate field offsets and sizes to prevent out‑of‑bounds access.  
**Context**: Part of a relational in‑memory database for online games, written in Rust. Storage uses `Vec<u8>` with tight packing, zero‑copy, and atomic operations.

---

## 1. Test Names & Descriptions

| Test Name                          | Brief Description                                                                                    |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `test_field_offset_within_record`  | Verify that a field’s offset + size does not exceed the record size.                                 |
| `test_record_size_calculation`     | Ensure record size equals the sum of all field sizes (tight packing).                                |
| `test_buffer_bounds_for_record`    | Check that a record’s start offset + record size ≤ buffer length.                                    |
| `test_field_alignment_respected`   | Confirm that each field’s offset meets its type’s alignment requirement (if alignment > 1).          |
| `test_zero_sized_field`            | Handle fields with size = 0 (e.g., ZST) without breaking offset arithmetic.                          |
| `test_offset_overflow`             | Detect overflow when computing `offset + size` (usize).                                              |
| `test_overlapping_fields`          | Reject schemas where fields would overlap in memory.                                                 |
| `test_negative_offset`             | Ensure offset is non‑negative (usize).                                                               |
| `test_large_offset_exceeds_buffer` | Validate that an offset near `usize::MAX` does not cause wrap‑around.                                |
| `test_field_access_ptr_in_bounds`  | When casting to `*const T`, verify the resulting pointer stays within the buffer’s allocated region. |
| `test_multi_record_field_access`   | Access fields across multiple records, ensuring each stays within its own record slice.              |
| `test_dynamic_buffer_growth`       | After buffer capacity changes, re‑validate all offsets and sizes.                                    |

---

## 2. What Each Test Verifies

- **`test_field_offset_within_record`**  
  Verifies that a field’s defined offset and size keep the field entirely inside the record boundary.

- **`test_record_size_calculation`**  
  Verifies that the computed record size matches the sum of individual field sizes (no hidden padding).

- **`test_buffer_bounds_for_record`**  
  Verifies that a record index maps to a byte range that lies completely inside the storage `Vec<u8>`.

- **`test_field_alignment_respected`**  
  Verifies that each field’s offset is a multiple of its alignment (if alignment > 1).  
  _Note_: Tight packing may set alignment = 1; test should skip when alignment = 1.

- **`test_zero_sized_field`**  
  Verifies that zero‑sized fields do not affect offset arithmetic (offset of next field = current offset).

- **`test_offset_overflow`**  
  Verifies that `offset.checked_add(size).is_some()` and does not panic or wrap.

- **`test_overlapping_fields`**  
  Verifies that the schema validation rejects fields whose byte ranges intersect (unless size = 0).

- **`test_negative_offset`**  
  Verifies that offset is a `usize` (already guaranteed by type system) and ≥ 0.

- **`test_large_offset_exceeds_buffer`**  
  Verifies that an offset close to `usize::MAX` is caught before pointer arithmetic (e.g., using `checked_add`).

- **`test_field_access_ptr_in_bounds`**  
  Verifies that the unsafe pointer cast for reading a field does not produce a pointer that dangles or points outside the allocation.

- **`test_multi_record_field_access`**  
  Verifies that iterating over records and accessing fields in each record does not cross record boundaries.

- **`test_dynamic_buffer_growth`**  
  Verifies that after the buffer is reallocated (e.g., capacity increased), all existing offsets remain valid relative to the new buffer length.

---

## 3. Edge Cases to Consider

- **Offset overflow**: `offset + size` exceeds `usize::MAX` (should be caught with `checked_add`).
- **Size zero**: Zero‑sized types (ZST) should not affect offsets; overlapping allowed.
- **Alignment mismatch**: If alignment > 1 but offset not aligned, access may be UB.
- **Overlapping fields**: Two fields sharing any byte (except ZST) indicates bug in schema.
- **Out‑of‑bounds access**: Offset or size that would read/write beyond buffer length.
- **Record index out of bounds**: Multiplying record index by record size may overflow.
- **Buffer empty** (length = 0): Any non‑zero record size should be invalid.
- **Large records**: Record size > buffer length (should be caught on insertion).
- **Custom composite types**: Vector of floats (`3xf32`) may have size = 12, alignment = 4.
- **Concurrent buffer swap** (ArcSwap): Validation must happen on the currently loaded buffer snapshot.

---

## 4. Assertions / Expected Behaviors

- For valid offsets/sizes: validation passes (returns `Ok` or `true`).
- For invalid offsets/sizes: validation fails (returns `Err` with descriptive error or `false`).
- Overflow detection: use `checked_add`, `checked_mul`; panic is not acceptable.
- Pointer‑bounds checks: use `ptr.addr()` and compare with buffer slice start/end addresses.
- Record‑boundary checks: `record_start + record_size ≤ buffer.len()`.
- Field‑boundary checks: `field_offset + field_size ≤ record_size`.
- Alignment checks: `field_offset % alignment == 0`.
- Overlap detection: ranges `[offset, offset+size)` intersect (size > 0).

---

## 5. Implementation Notes

- Tests should be placed in the Rust crate under `packages/db` (once created).
- Use `#[test]` and `#[should_panic]` where appropriate.
- Leverage `assert!`, `assert_eq!`, `assert_matches!` for validation.
- For unsafe pointer validation, consider helper functions that wrap `std::ptr::range`.
- Edge‑case data can be generated with property‑based testing (e.g., `proptest`).
- Ensure tests run with `cargo test` and pass under `--release` (no debug‑only checks).
