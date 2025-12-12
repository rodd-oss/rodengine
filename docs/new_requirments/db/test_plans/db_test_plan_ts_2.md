# Test Plan for task_ts_2 – Define Field struct

## Overview

Unit tests for the `Field` struct with `name`, `type identifier`, and `byte offset`. The struct is a simple data carrier; validation of values is deferred to later tasks.

## Test Cases

### 1. **test_field_creation**

**Verifies:** Basic instantiation with valid data.
**Assertions:**

- `field.name == "id"`
- `field.type_id == TypeId::I32`
- `field.byte_offset == 0`

### 2. **test_field_accessors**

**Verifies:** Public fields (or getter methods) return expected values.
**Assertions:**

- `field.name()` returns `"score"`
- `field.type_id()` returns `TypeId::F32`
- `field.byte_offset()` returns `8`

### 3. **test_derived_traits**

**Verifies:** `Debug`, `Clone`, `PartialEq` are implemented (if required).
**Assertions:**

- `format!("{:?}", field)` contains field data
- `field.clone() == field`
- Two fields with same data compare equal; different data compare unequal

### 4. **test_edge_empty_name**

**Verifies:** Field can be created with empty string name (allowed? maybe not).
**Edge case:** Empty name may be invalid for schema; test that validation (if present) catches it.
**Assertions:**

- `Field::new("", TypeId::Bool, 0)` does not panic
- Later validation method `field.validate()` returns `Err` (if validation exists)

### 5. **test_edge_max_offset**

**Verifies:** Offset can be `usize::MAX` (theoretical limit).
**Edge case:** Offset may overflow when added to base pointer; safety checks belong elsewhere.
**Assertions:**

- `Field::new("dummy", TypeId::U8, usize::MAX)` succeeds

### 6. **test_edge_offset_alignment**

**Verifies:** Offset alignment is not enforced by struct itself (tight packing assumes alignment = 1).
**Edge case:** Misaligned offset for a type with alignment > 1 will cause UB; later validation should flag it.
**Assertions:**

- `Field::new("x", TypeId::I32, 3)` (misaligned) creates without panic

### 7. **test_invalid_type_identifier**

**Verifies:** Type identifier can be any value (e.g., `u8` out of enum range) if represented as `u8`; no runtime validation in struct.
**Edge case:** Unknown type identifier should be caught by schema validation.
**Assertions:**

- `Field::new("y", 255_u8, 0)` (if type_id is `u8`) compiles

### 8. **test_serialization_deserialization**

**Verifies:** Field can be serialized/deserialized (if `serde` support is planned).
**Assertions:**

- `serde_json::to_string(&field)` succeeds
- Round‑trip yields equal field

### 9. **test_field_update**

**Verifies:** Mutability of fields (if struct fields are `pub`).
**Assertions:**

- `field.name = "new_name"` updates correctly
- `field.byte_offset = 12` updates correctly

### 10. **test_constants_for_builtin_types**

**Verifies:** Constants (e.g., `Field::i32`) or helper constructors exist.
**Assertions:**

- `Field::i32("id", 0)` creates field with `TypeId::I32`
- `Field::bool("active", 4)` creates field with `TypeId::Bool`

## Edge Cases Considered

- Empty name
- Invalid type identifier (out‑of‑range)
- Offset overflow (usize::MAX)
- Misaligned offset for aligned types
- Unicode characters in name
- Very long name (memory usage)
- Negative offset (not possible with `usize`)

## Notes

Safety‑critical validation (bounds, alignment, type validity) is deferred to later tasks (`task_ms_1`, `task_sl_2`, `task_sl_3`). These tests ensure the struct is a simple data carrier.
