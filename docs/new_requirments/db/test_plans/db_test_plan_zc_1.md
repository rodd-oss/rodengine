# Test Plan for task_zc_1: Field Accessors Return References (&T)

**Task ID**: task_zc_1  
**Description**: Ensure field accessors return references (&T) rather than copying values.  
**Context**: Relational in‑memory database for online games, Rust, `Vec<u8>` storage, tight packing, zero‑copy, atomic operations.

---

## Assumptions

- Storage buffer: `Vec<u8>` per table, tightly packed fields/records.
- Field definitions: `(name, type T, offset)`.
- Accessor signature: `fn field<T>(record_idx: usize, field_idx: usize) -> &T`.
- Mutable variant (if present): `fn field_mut<T>(…) -> &mut T`.

---

## 1. Basic Reference Semantics

| Test Name                              | What it verifies                                                     | Edge Cases                                                      | Assertions / Expected Behavior                                                                    |
| -------------------------------------- | -------------------------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `test_returns_reference_not_copy`      | `field` returns `&T`, not `T`.                                       | Scalar types, composite types.                                  | `std::mem::size_of_val(field_ref) == size_of::<T>()`; pointer equality with known buffer address. |
| `test_mut_accessor_returns_mut_ref`    | `field_mut` returns `&mut T` and allows modification.                | Only one mutable reference per field at a time.                 | Modify via `*field_mut = new_val`; read back via `field` shows updated value.                     |
| `test_reference_to_scalar_types`       | References to `i32`, `u64`, `f32`, `bool` point to correct bytes.    | Signed/unsigned, floating‑point, bool (must be 0/1).            | `*field_ref == expected_value`; pointer offset matches calculated field offset.                   |
| `test_custom_composite_type_reference` | User‑defined type (e.g., `[f32; 3]`) reference aligns to its layout. | Composite may have padding; reference must point to first byte. | `field_ref as *const _ as usize % align_of::<[f32;3]>() == 0`; all elements match.                |

---

## 2. Lifetime and Safety

| Test Name                                 | What it verifies                                                               | Edge Cases                                 | Assertions / Expected Behavior                                                                      |
| ----------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| `test_reference_lifetime_bound_to_buffer` | Reference cannot outlive the buffer guard.                                     | Drop guard while reference still in scope. | **Compile‑fail test** using `trybuild`: attempt to use reference after guard drop must be rejected. |
| `test_multiple_references_same_record`    | Two immutable references to different fields in same record can coexist.       | Fields may overlap? (should not).          | Both references usable simultaneously; `ptr::eq` shows distinct addresses.                          |
| `test_references_across_records`          | Hold references to field 0 of record 0 and field 0 of record 1 simultaneously. | Records are contiguous in buffer.          | References point to addresses `record_size` apart.                                                  |

---

## 3. Alignment and Packing

| Test Name                              | What it verifies                                                   | Edge Cases                                                                  | Assertions / Expected Behavior                                                          |
| -------------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `test_aligned_access_for_requirements` | For types with alignment >1, derived pointer satisfies `align_of`. | `u64` on 8‑byte boundary, `f32` on 4‑byte.                                  | `field_ref as *const T as usize % align_of::<T>() == 0`.                                |
| `test_packed_fields_no_padding`        | Consecutive fields’ offsets differ exactly by `size_of::<T>`.      | Mixed‑size fields (e.g., `i32` then `u8`).                                  | `offset_{i+1} == offset_i + size_of::<T_i>`.                                            |
| `test_bool_validity`                   | Stored byte is 0 or 1; reading as `&bool` does not cause UB.       | Any non‑0/1 byte is invalid; must be validated or guaranteed by write path. | If validation exists, invalid byte should panic; otherwise document as safety contract. |

---

## 4. Multiple Fields and Records

| Test Name                               | What it verifies                                                                       | Edge Cases                                              | Assertions / Expected Behavior                                |
| --------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------- |
| `test_iterator_yields_references`       | Iterate over records, collect references to a field; they point to distinct addresses. | Empty table, single record.                             | Collected references are all different (`ptr::eq`).           |
| `test_field_access_out_of_bounds_panic` | Request field beyond record count or field count panics.                               | `record_idx == len_records`, `field_idx == len_fields`. | `#[should_panic]` attribute.                                  |
| `test_zero_sized_types`                 | If ZSTs are supported, reference is dangling but usable.                               | ZST field between non‑ZST fields.                       | `size_of::<T>() == 0`; reference can be created and compared. |

---

## 5. Concurrency with ArcSwap

| Test Name                                 | What it verifies                                                                                                              | Edge Cases                             | Assertions / Expected Behavior                                                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------- |
| `test_reference_valid_across_buffer_swap` | Load buffer via `ArcSwap::load`, obtain reference, swap buffer in another thread; reference remains valid (old buffer alive). | Swap while reference held.             | Reference still points to old value; new buffer contains updated data.            |
| `test_no_data_race`                       | Concurrent reads via references while a write modifies a different record does not cause UB.                                  | Write to record 0, read from record 1. | No panic, no corrupted data; reads see either old or new value (but not garbage). |

---

## 6. Edge Cases and Invalid Access

| Test Name                                 | What it verifies                                                                                    | Edge Cases                                     | Assertions / Expected Behavior                                      |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------- |
| `test_null_bytes_valid_for_type`          | Fill buffer with zeros; reading as `&i32` yields `0` without UB.                                    | All bits zero is valid for numeric types.      | `*field_ref == 0`.                                                  |
| `test_misaligned_offset_unsafe`           | If offset is not naturally aligned, access is marked `unsafe` and documented.                       | Manually craft misaligned field definition.    | Function containing `read_unaligned`/`write_unaligned` is `unsafe`. |
| `test_large_record_near_buffer_end`       | Record that ends exactly at buffer length; reference to its last field is valid.                    | Buffer length = `record_size * n`.             | No out‑of‑bounds panic; reference dereference succeeds.             |
| `test_drop_buffer_while_reference_exists` | Use `Arc::try_unwrap` to attempt deallocation while reference is held; must fail (strong count >1). | Last strong reference dropped after reference. | `Arc::try_unwrap(arc).is_err()`.                                    |

---

## Summary of Assertions & Expected Behaviors

- **Pointer equality**: `assert!(std::ptr::eq(ref1, ref2))`
- **Value correctness**: `assert_eq!(*field_ref, expected_value)`
- **Alignment**: `assert!(field_ref as *const T as usize % align_of::<T>() == 0)`
- **Bounds checking**: `#[should_panic]` attribute for out‑of‑range indices
- **Lifetime safety**: compile‑fail tests using `trybuild`
- **Concurrency**: references remain valid after `ArcSwap` swap; no data races

---

## Edge Cases to Consider Explicitly

1. **Lifetime correctness**: reference must not outlive the `Arc` guard.
2. **Mutable vs immutable**: if both `&T` and `&mut T` are provided, they must follow Rust’s aliasing rules (no mutable alias while immutable references exist).
3. **Packed fields**: references to fields in packed structs may have unaligned addresses; must use `read_unaligned`/`write_unaligned` in unsafe blocks.
4. **Atomicity**: references derived from a loaded buffer snapshot must remain valid even after a swap (old buffer remains alive due to `Arc`).
5. **Custom types**: user‑defined composite types may have padding; ensure references point to the first byte of the composite.
6. **Zero‑sized types**: references are dangling; must still be safe to create and compare.
7. **Bool validity**: stored byte must be 0 or 1 to avoid UB when reading as `&bool`.

---

**Next Steps**: Implement these tests alongside the field accessor implementation in the `packages/db` crate.
