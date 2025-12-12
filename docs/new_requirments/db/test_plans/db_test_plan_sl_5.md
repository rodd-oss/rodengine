# Test Plan for task_sl_5: Read records from buffer via unsafe pointer casting and validate data integrity

## 1. Basic Reading Tests

- **`test_read_single_record_i32`** – Verifies that a single i32 field is correctly read from buffer at offset 0.
- **`test_read_multiple_fields_mixed_types`** – Reads a record containing i32, f32, bool, u64 fields and asserts each matches written values.
- **`test_read_multiple_records`** – Writes several records, reads each by index, ensures data matches.

## 2. Edge Cases & Bounds

- **`test_read_out_of_bounds_index_panics`** – Attempt to read record index beyond buffer capacity; expects panic or error.
- **`test_read_partial_record_near_end_of_buffer`** – Record size fits but last field’s bytes extend beyond buffer; must detect out‑of‑bounds.
- **`test_zero_sized_record`** – Record with zero size (no fields) can be read without crash.
- **`test_buffer_length_not_multiple_of_record_size`** – Buffer length not a multiple of record size; reading last record should fail validation.

## 3. Alignment & Memory Safety

- **`test_unaligned_buffer_access`** – Buffer pointer not aligned to field’s alignment requirement (e.g., u64 on 4‑byte boundary); reading must either realign or panic.
- **`test_misaligned_field_offsets`** – Field offsets that are not naturally aligned; ensure reads still produce correct values (if packing allows) or trap.
- **`test_validate_alignment_of_custom_type`** – Custom composite type (e.g., `3xf32`) must be aligned to its largest element.

## 4. Data Integrity Validation

- **`test_bool_validation_rejects_invalid_bytes`** – Bytes other than 0 or 1 for bool field cause validation error.
- **`test_float_nan_and_infinity_allowed`** – NaN/Infinity float values are considered valid (no validation rejection).
- **`test_custom_type_validation`** – Custom type may have invariants (e.g., vector length positive); validation function checks them.
- **`test_corrupted_buffer_detected`** – Fill buffer with random bytes; reading any record should either produce garbage (if no validation) or trigger validation failure.

## 5. Concurrency & Atomicity (future‑proof)

- **`test_read_while_buffer_swapped`** – With ArcSwap buffer, reading from old buffer while a new buffer is swapped must still be safe (no data races).
- **`test_concurrent_reads_no_tearing`** – Multiple threads reading same record concurrently must observe consistent values.

## 6. Integration with Write (task_sl_4)

- **`test_write_then_read_roundtrip`** – Write a record via task_sl_4’s function, then read it back; values must match.
- **`test_overwrite_record_then_read`** – Update a record’s bytes directly, read via casting; ensures reading reflects latest write.

## 7. Performance & Correctness

- **`test_read_does_not_copy`** – Verify that reading returns references (`&T`) not owned values (zero‑copy).
- **`test_record_iterator_yields_correct_references`** – Iterate over all records in buffer; each yielded reference must point to correct memory region.

## Edge Cases to Consider

- Buffer length = 0 (empty table).
- Record size larger than buffer capacity.
- Field offset + size overflow `usize`.
- Mixed‑endness if data is exchanged across architectures (assume native endian).
- Padding bytes in buffer (should be none due to tight packing).
- Concurrent modification of buffer while reading (should be prevented by ArcSwap or cause undefined behavior).
- Invalid pointer arithmetic (e.g., offset beyond `isize::MAX`).
- Type‑specific invalid bit patterns (e.g., enum discriminants).

## Assertions & Expected Behaviors

- Successful reads: returned values equal those written.
- Out‑of‑bounds reads: panic with clear message or return `Result::Err`.
- Invalid data (e.g., bool ≠ 0/1): validation function returns error; reading may still return the bytes (if validation separate).
- Alignment violations: either panic or perform unaligned read (platform‑dependent).
- Zero‑copy: `std::mem::size_of_val(&read_value) == field_size` and pointer equality to buffer region.

**Note:** Tests should be written as `#[test]` functions in the same module as the implementation, using `unsafe` blocks only where necessary. Use `should_panic` attribute for panic‑expected tests.
