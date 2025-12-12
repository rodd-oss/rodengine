# Test Plan for `task_sl_2` – Field Type Definitions

## 1. Basic Primitive Types

**Test Name:** `test_primitive_type_sizes_and_alignments`  
**Description:** Verify that each built‑in scalar type reports the correct size and alignment as defined by `std::mem::size_of` and `std::mem::align_of`.  
**Verifies:**

- `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `f32`, `f64`, `bool` have expected sizes (1, 2, 4, 8, 16, etc.) and alignments (same as size for integers ≤ 8, 16 for i128/u128, 4 for f32, 8 for f64, 1 for bool).
- The type system’s `size()` and `alignment()` methods match Rust’s memory layout.

**Edge Cases:**

- `bool` size is 1 byte, alignment is 1.
- `i128`/`u128` size and alignment are 16 (may be platform‑independent for database storage).
- `isize`/`usize` are excluded because their size is target‑dependent; use fixed‑width alternatives.

**Assertions:**

```rust
assert_eq!(Type::I32.size(), 4);
assert_eq!(Type::I32.alignment(), 4);
// … for all primitive types
```

---

## 2. Alignment Validation

**Test Name:** `test_alignment_is_power_of_two_and_le_size`  
**Description:** Ensure every type’s alignment is a power of two and does not exceed its size (except for types where size < alignment, e.g., SIMD types – not required initially).  
**Verifies:**

- `alignment().is_power_of_two()`
- `alignment() <= size()` (or `size() % alignment() == 0`).

**Edge Cases:**

- Custom composite types (future) may have alignment larger than individual field sizes.
- Zero‑sized types (if supported) have size 0 and alignment 1.

**Assertions:**

```rust
assert!(ty.alignment().is_power_of_two());
assert!(ty.size() % ty.alignment() == 0);
```

---

## 3. Type Identity and Equality

**Test Name:** `test_type_identity_and_equality`  
**Description:** Verify that two instances of the same primitive type compare equal, and different types compare unequal.  
**Verifies:**

- `Type::I32 == Type::I32`
- `Type::I32 != Type::U32`
- Hash consistency (if `Hash` is derived).

**Edge Cases:**

- Distinguishing between signed/unsigned variants of same width (i32 vs u32).
- Distinguishing between integer and float of same size (i32 vs f32).

**Assertions:**

```rust
assert_eq!(Type::I32, Type::I32);
assert_ne!(Type::I32, Type::U32);
```

---

## 4. Serialization/Deserialization (if needed for schema)

**Test Name:** `test_type_serialization_roundtrip`  
**Description:** If types are serialized to/from JSON (for schema), ensure round‑trip fidelity.  
**Verifies:**

- `serde_json::to_string(&ty)` and `serde_json::from_str` produce the same type.
- Unknown type strings are rejected.

**Edge Cases:**

- Malformed type strings.
- Extra fields in JSON (should be ignored or error).

**Assertions:**

```rust
let json = serde_json::to_string(&Type::F64).unwrap();
let decoded: Type = serde_json::from_str(&json).unwrap();
assert_eq!(decoded, Type::F64);
```

---

## 5. Custom Composite Types (optional – if task includes `3xf32`)

**Test Name:** `test_composite_type_layout`  
**Description:** Verify that a user‑defined composite type (e.g., `Vec3` as `3×f32`) reports correct total size and alignment.  
**Verifies:**

- Size = sum of component sizes (tight packing).
- Alignment = max alignment of components.
- Component types are accessible.

**Edge Cases:**

- Nested composites.
- Zero‑size components.
- Alignment padding if tight packing is relaxed later.

**Assertions:**

```rust
let vec3 = Type::Composite(&[Type::F32, Type::F32, Type::F32]);
assert_eq!(vec3.size(), 12);
assert_eq!(vec3.alignment(), 4);
```

---

## 6. Error Cases

**Test Name:** `test_invalid_type_handling`  
**Description:** Ensure that requesting size/alignment for an invalid type (e.g., placeholder `Unknown`) panics or returns a `Result::Err`.  
**Verifies:**

- Unsupported type triggers a defined error (panic or `Err`).
- Error message is informative.

**Edge Cases:**

- Type registry not yet initialized.
- Dynamic type loading failures.

**Assertions:**

```rust
assert!(matches!(Type::Unknown.size(), Err(_)));
```

---

## 7. Integration with Offset Calculation (pre‑task_sl_3)

**Test Name:** `test_offset_calculation_helper`  
**Description:** Validate that a helper function `offset_after_field(current_offset, ty)` returns `current_offset` aligned up to `ty.alignment()` (i.e., `((current_offset + align - 1) / align) * align`).  
**Verifies:**

- Offsets respect alignment requirements.
- Tight packing (no extra padding beyond alignment) is enforced.

**Edge Cases:**

- Zero‑size types do not increase offset.
- Alignment = 1 yields no adjustment.

**Assertions:**

```rust
assert_eq!(align_offset(0, Type::I32), 0); // 0 aligned to 4 is 0
assert_eq!(align_offset(1, Type::I32), 4);
assert_eq!(align_offset(4, Type::I32), 4);
```

---

## Summary of Edge Cases to Consider:

- All primitive integer and float widths (including 128‑bit).
- Boolean representation (1 byte).
- Alignment power‑of‑two and ≤ size.
- Equality and hashing of types.
- Serialization round‑trip (if used).
- Custom composite types (if in scope).
- Invalid type handling.
- Offset alignment helper for tight packing.

## Expected Behaviors:

- Each type provides `size() -> usize` and `alignment() -> usize`.
- Sizes and alignments match Rust’s `std::mem` values.
- The system is ready for task_sl_3 (record size calculation) and task_sl_4 (unsafe pointer casting).
