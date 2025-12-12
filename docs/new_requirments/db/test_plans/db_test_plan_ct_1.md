# Test Plan for `task_ct_1`: Support built‑in scalar types (i32, u64, f32, bool, etc.)

**Objective:** Verify that each built‑in scalar type can be correctly serialized/deserialized to/from the packed `Vec<u8>` storage buffer, respects its size/alignment, and works with zero‑copy access.

---

## 1. Core Type Properties

| Test Name                     | Description                                                       | Verifies                                                                                                                             | Edge Cases                                                   |
| ----------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| `test_scalar_type_sizes`      | Check that each type’s `size_of` matches the expected byte count. | `i32` = 4B, `u64` = 8B, `f32` = 4B, `bool` = 1B, `i8`/`u8` = 1B, `i16`/`u16` = 2B, `f64` = 8B, `isize`/`usize` = platform‑dependent. | Verify `usize`/`isize` size matches target pointer width.    |
| `test_scalar_type_alignments` | Verify alignment is **1** (tight packing, no padding).            | All types have alignment 1 when packed.                                                                                              | Ensure no implicit padding between fields.                   |
| `test_type_id_uniqueness`     | Each scalar type has a distinct, stable type identifier.          | IDs are unique and consistent across runs.                                                                                           | No collisions between `i32` and `u32`, `f32` and `i32`, etc. |

---

## 2. Serialization / Deserialization (Bytes ↔ Value)

| Test Name                       | Description                                        | Verifies                                                            | Edge Cases                                              |
| ------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------- |
| `test_serialize_i32`            | Write `i32` values to a byte buffer, read back.    | Round‑trip equality for zero, positive, negative, min, max.         | Endianness (little‑endian as per Rust’s `to_le_bytes`). |
| `test_serialize_u64`            | Write `u64` values to a byte buffer, read back.    | Round‑trip equality for 0, `u64::MAX`, arbitrary values.            | 64‑bit overflow (none; storage matches size).           |
| `test_serialize_f32`            | Write `f32` values to a byte buffer, read back.    | Round‑trip equality for 0.0, ±inf, NaN, subnormals, `f32::MIN/MAX`. | NaN bits preserved (canonical NaN not required).        |
| `test_serialize_bool`           | Write `bool` values to a byte buffer, read back.   | `true` → 1u8, `false` → 0u8; any non‑zero byte → `true`.            | All 256 byte values map correctly to `bool`.            |
| `test_serialize_all_primitives` | Round‑trip all supported primitives in one buffer. | Mixed types placed at correct offsets (tight packing).              | Offsets are sum of preceding field sizes.               |

---

## 3. Zero‑Copy Access

| Test Name                               | Description                                                           | Verifies                                                                      | Edge Cases                                                                               |
| --------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `test_field_accessor_returns_reference` | Field accessor returns `&T` (not `T`).                                | Type of returned value is a reference, verified with `std::mem::size_of_val`. | References remain valid after buffer reallocation? (Not applicable if buffer is frozen.) |
| `test_iter_yields_references`           | Iterator over records yields `&[u8]` slices that can be cast to `&T`. | Slices are correctly aligned and sized.                                       | Empty iterator, single record, many records.                                             |

---

## 4. Memory‑Safety & Bounds

| Test Name                        | Description                                             | Verifies                              | Edge Cases                                     |
| -------------------------------- | ------------------------------------------------------- | ------------------------------------- | ---------------------------------------------- |
| `test_out_of_bounds_read_panics` | Reading past buffer end causes panic/bounds check.      | Explicit panic or `None` returned.    | Offset = buffer.len() – 1, size = 2 (overlap). |
| `test_unaligned_access_works`    | Reading a `u64` at an odd offset works (alignment = 1). | `ptr::read_unaligned` does not crash. | Misaligned reads produce correct value.        |

---

## 5. Type Equality / Ordering

| Test Name              | Description                                               | Verifies                                        | Edge Cases                              |
| ---------------------- | --------------------------------------------------------- | ----------------------------------------------- | --------------------------------------- |
| `test_scalar_equality` | Two values of same type compare equal/not equal.          | `i32(42) == i32(42)`, `i32(42) != i32(43)`.     | NaN != NaN (if using partial equality). |
| `test_scalar_ordering` | Ord‑able types (`i32`, `u64`, etc.) respect `PartialOrd`. | `i32(1) < i32(2)`, `f32` ordering (except NaN). | NaN comparisons return `None`.          |

---

## 6. JSON Serialization (API boundary)

| Test Name               | Description                                           | Verifies                                                       | Edge Cases                                            |
| ----------------------- | ----------------------------------------------------- | -------------------------------------------------------------- | ----------------------------------------------------- |
| `test_scalar_to_json`   | Each scalar can be serialized to JSON (for REST API). | `i32(42)` → `42`, `bool(true)` → `true`, `f32(3.14)` → number. | `f32::INFINITY`, `NaN` serialize to `null` or string. |
| `test_scalar_from_json` | JSON can be deserialized back to scalar.              | JSON number → `i32` (clamp on overflow?), JSON bool → `bool`.  | Invalid JSON (string for number) returns error.       |

---

## 7. Edge‑Case Coverage

| Test Name                              | Description                                      | Verifies                                                                                      | Edge Cases                                         |
| -------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| `test_all_supported_primitives_list`   | Ensure the list of built‑in types is complete.   | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `isize`, `usize`, `f32`, `f64`, `bool`. | `char` not supported (not a storage primitive).    |
| `test_default_values`                  | Each type has a sensible default (zero, false).  | `i32` → 0, `bool` → false, `f32` → 0.0.                                                       | Default is all‑zero bits.                          |
| `test_roundtrip_through_buffer_growth` | Serialize, grow buffer, deserialize still works. | Buffer capacity change does not corrupt stored values.                                        | Values at end of buffer before/after reallocation. |

---

**Next Steps:** Once the Rust crate `packages/db` is created, implement these tests as module‑level unit tests (`#[cfg(test)]`). Use `cargo test` to run them. Follow the existing pattern in `db_backlog_tdd.txt` (storage‑layout tasks) for integration with field offsets and record packing.
