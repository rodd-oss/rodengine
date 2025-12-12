# Test Plan for `task_sl_3`: Calculate Record Size with Tight Packing

## 1. Core Unit Tests

| Test Name                      | Description                              | Verification                            | Expected Behavior                            |
| ------------------------------ | ---------------------------------------- | --------------------------------------- | -------------------------------------------- |
| `test_empty_fields`            | Empty field list                         | Record size is 0                        | `calculate_record_size(&[]) == 0`            |
| `test_single_primitive`        | Single primitive field (i32, f64, bool)  | Size equals `size_of::<T>()`            | e.g., `[i32]` → 4 bytes                      |
| `test_multiple_same_type`      | Multiple fields of same type             | Size = n × `size_of::<T>()`             | `[i32, i32, i32]` → 12 bytes                 |
| `test_mixed_primitives`        | Mixed primitive types (i8, i32, f64)     | Size equals sum of individual sizes     | `[i8, i32, f64]` → 1 + 4 + 8 = 13 bytes      |
| `test_custom_composite`        | Custom composite type (e.g., `3xf32`)    | Size equals defined composite size      | `[Vec3]` where `Vec3 = 3×f32` → 12 bytes     |
| `test_field_order_independent` | Different field orders produce same size | Commutativity of summation              | `[i32, f64]` == `[f64, i32]` (both 12 bytes) |
| `test_large_record`            | Many fields (100+)                       | Size calculation doesn't overflow usize | Sum fits within `usize` bounds               |
| `test_zero_sized_types`        | Zero‑byte types (e.g., `()`)             | Zero‑sized types contribute 0 bytes     | `[(), i32, ()]` → 4 bytes                    |

## 2. Edge‑Case & Validation Tests

| Test Name                       | Description                                            | Verification                         | Expected Behavior                      |
| ------------------------------- | ------------------------------------------------------ | ------------------------------------ | -------------------------------------- |
| `test_alignment_no_padding`     | Fields with alignment >1 (e.g., `u64` align=8)         | No padding inserted between fields   | `[u8, u64]` → 1 + 8 = 9 bytes (not 16) |
| `test_overflow_panic`           | Sum exceeds `usize::MAX`                               | Function panics or returns `None`    | Panic with clear error message         |
| `test_negative_offset_overflow` | Large number of fields causing offset overflow         | Offset addition checked for overflow | Panic or saturate at `usize::MAX`      |
| `test_memory_layout_match`      | Compare calculated size with actual `Vec<u8>` capacity | Size matches allocated buffer length | `Vec::with_capacity(size)` succeeds    |

## 3. Edge Cases to Consider

- **Empty field list** – should return 0, not panic.
- **Mixed‑size primitives** – ensure no implicit padding between differently‑aligned types.
- **Custom types with internal padding** – if a composite type has internal padding, does the whole type count as its `size_of`?
- **Very large records** – ensure no intermediate overflow in summation loop.
- **Alignment vs packing** – confirm that “tight packing” means `align = 1` for the whole record, not per‑field alignment.
- **Platform‑specific sizes** – use `size_of` rather than hard‑coded byte counts.
- **Zero‑sized fields** – they should not affect offsets of subsequent fields.

## 4. Assertions & Expected Behaviors

- **Panic conditions**: overflow of `usize`, invalid field type definitions.
- **Return type**: `usize` (record size in bytes).
- **Invariant**: `size_of::<Record>()` (if the record were a Rust struct) may differ due to Rust’s default alignment; the calculated size must reflect the tightly‑packed, padding‑free layout.
- **Validation**: After implementing `task_sl_4` (writing records), the calculated size should exactly match the number of bytes consumed per record in the storage buffer.
