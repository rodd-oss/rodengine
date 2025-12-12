# Test Plan for Task CT-2: User‑Defined Composite Types via Type Registry

## Context

- In‑memory relational database with `Vec<u8>` storage.
- Supports built‑in scalar types (i32, u64, f32, bool, etc.) – task CT‑1.
- Need to allow user‑defined composite types (e.g., `Vec3` as three `f32`s) that can be used as field types.
- Composite types are registered in a type registry; each composite has a name, a list of component types, and a computed size/alignment for tight packing.
- Zero‑copy access to composite values must be preserved.

## Assumptions

- `TypeRegistry` stores definitions of scalar and composite types.
- `register_composite(name: &str, components: &[Component]) -> Result<TypeId, RegistryError>`.
- `Component` describes a component type (by `TypeId`) and possibly a name for fields.
- Composite type alignment = maximum alignment of its components.
- Component offsets are computed with proper alignment (no extra padding beyond required).
- Composite size = sum of component sizes + padding to align to its own alignment.
- Duplicate registration is an error (or overwrites) – decide based on design.
- Nested composites (composite containing another composite) are allowed.
- Zero‑sized components (e.g., unit type) are allowed.
- The registry is thread‑safe with lock‑free reads via `ArcSwap`; updates are performed by atomically swapping a new version of the registry.

## Test Categories

### 1. Basic Registration & Retrieval

**test_register_simple_composite**  
Verifies that a simple composite (e.g., `Vec3 = [f32; 3]`) can be registered and retrieved.

- Setup: Empty registry.
- Action: Register `Vec3` with three `f32` components.
- Assert: Retrieval returns correct `CompositeDef`; size = 12 bytes, alignment = 4, offsets = `0, 4, 8`.

**test_register_composite_with_mixed_scalars**  
Composite with different scalar types (e.g., `Player { id: u64, health: f32, alive: bool }`).

- Assert: Size = sum of sizes with proper alignment (u64 align 8, f32 align 4, bool align 1).
- Offsets: `id` at 0, `health` at 8, `alive` at 12 (or 13 if padding?).

**test_register_nested_composite**  
Composite that contains another composite as a component.

- Setup: Register inner composite `Vec3`.
- Action: Register `Line { start: Vec3, end: Vec3 }`.
- Assert: Size = 2 \* size_of::<Vec3> (24), alignment = max(inner alignment, 4).
- Offsets: start at 0, end at 12.

### 2. Duplicate Registration

**test_duplicate_name_error**  
Attempt to register a composite with a name already used by a scalar or another composite.

- Expect: `RegistryError::DuplicateName`.

**test_duplicate_name_overwrite**  
If design allows overwriting, verify that old definition is replaced.

- Expect: New size/alignment matches new definition.

### 3. Invalid Composite Definitions

**test_empty_component_list**  
Composite with zero components (zero‑sized type).

- Action: Register `Empty = []`.
- Assert: Size = 0, alignment = 1 (or 0).
- Edge: Can be used as a field (zero‑size record).

**test_component_type_not_found**  
One of the component `TypeId`s does not exist in registry.

- Expect: `RegistryError::UnknownComponentType`.

**test_recursive_composite**  
Composite that contains itself directly (infinite size).

- Implementation may prevent this via acyclic check.
- Expect: `RegistryError::CyclicDependency`.

### 4. Size & Alignment Calculations

**test_alignment_respected**  
Composite with components of alignment 8 (u64) and 4 (f32).

- Assert: Alignment = 8, size = 16 (8 + 4 + 4 padding? depends on ordering).
- Offsets: u64 at 0, f32 at 8 (or 12 if we pack after u64?).

**test_packing_no_unnecessary_padding**  
Order components by decreasing alignment to minimize padding.

- Not required but good to verify that computed offsets follow Rust `repr(C)` packing.

**test_zero_sized_component_padding**  
Composite with zero‑sized component (e.g., `PhantomData`) does not affect size.

- Assert: Size = sum of non‑zero components, offsets correct.

### 5. Serialization / Deserialization

**test_composite_serialize_json**  
Composite definition can be serialized to JSON (for schema persistence).

- Assert: Round‑trip yields identical definition.

**test_composite_deserialize_with_unknown_component**  
Deserialize JSON where a component type is not yet registered.

- Expect: Error or automatic registration of missing scalar types.

### 6. Integration with Field Definition

**test_create_table_with_composite_field**  
Use a registered composite type as a field type in a table.

- Setup: Register `Vec3`.
- Action: Create table with a `Vec3` field.
- Assert: Record size = size_of::<Vec3>, field offset = 0.

**test_access_composite_field_zero_copy**  
Read/write a composite field via reference without copying.

- Setup: Table with `Vec3` field, insert a record.
- Action: Obtain `&Vec3` via field accessor.
- Assert: Value matches inserted data; memory address lies within storage buffer.

**test_nested_composite_field_access**  
Composite field that itself contains nested composites.

- Assert: Can read inner component via nested offset calculations.

### 7. Concurrency & Thread Safety

**test_concurrent_registration**  
Multiple threads registering different composites simultaneously, using atomic swaps via ArcSwap.

- Expect: No data races; all definitions retrievable afterward with atomic visibility.

**test_concurrent_read_while_write**  
One thread registers a composite while another reads an existing composite, using ArcSwap for atomic updates.

- Expect: Reader sees consistent state (no invalid pointers) and lock-free reads via ArcSwap.

### 8. Edge Cases & Stress

**test_large_number_of_components**  
Composite with hundreds of components (stress offset calculation).

- Assert: Size matches sum with proper alignment.

**test_component_with_large_alignment**  
Component with alignment larger than any basic type (e.g., SIMD type).

- Expect: Composite alignment matches that large alignment.

**test_composite_name_length_limits**  
Very long composite names (e.g., 1MB string).

- Expect: Registry accepts or rejects based on policy.

**test_composite_with_array_component**  
Component is a fixed‑size array of scalar (e.g., `matrix: [f32; 16]`).

- Treat as repeated component; size = 16 \* 4 = 64, alignment = 4.

### 9. Error Messages & Diagnostics

**test_error_includes_component_index**  
When a component type is unknown, error indicates which index failed.

- Assert: Error contains `component_index: usize`.

**test_error_on_size_overflow**  
If computed size overflows `usize`.

- Expect: `RegistryError::SizeOverflow`.

### 10. Memory Safety

**test_offset_within_bounds**  
Ensure each component offset + size ≤ composite size.

- Automatically satisfied by correct calculation; test with random component lists.

**test_alignment_not_zero**  
Composite alignment is at least 1.

- Zero‑sized composite may have alignment 1.

## Edge Cases to Consider

1. **Duplicate registration with same components but different name** – Allowed.
2. **Duplicate registration with same name but different components** – Error (or overwrite).
3. **Nested composite where inner composite alignment larger than outer** – Composite alignment must increase.
4. **Zero‑sized composite as component of another composite** – Should not affect size or alignment.
5. **Component list includes the same type multiple times** – Allowed (e.g., `Vec3 = [f32, f32, f32]`).
6. **Composite name clashes with built‑in scalar type name** – Should be disallowed (or shadow?).
7. **Composite with component that is later unregistered** – Should cause dangling reference; prevent unregistration while in use.
8. **Alignment of composite larger than any component due to trailing padding** – Size must be rounded up to alignment.
9. **Composite used as field in multiple tables** – Should work without extra registration.
10. **Serialization of nested composite with cycles** – Should be prevented.

## Implementation Notes

- Use `cargo test` with `--test-threads=1` for concurrency tests (or use `std::sync` barriers). Ensure ArcSwap atomic swaps are tested for lock-free reads.
- Mock the registry without actual storage buffer for unit tests.
- For integration tests, need table and storage infrastructure (tasks SL‑1 … SL‑5).
- Use `#[should_panic]` for panic tests, `Result` for error tests.
- Consider property‑based testing (quickcheck) for size/alignment calculations.

## Test Coverage Goals

- All registration error paths (duplicate, unknown component, cyclic).
- Size/alignment calculations for various component orders.
- Serialization round‑trip.
- Zero‑copy access to composite fields.
- Thread‑safe operations.

## Dependencies

- Type registry implementation (new).
- Built‑in scalar type definitions (task CT‑1).
- Composite definition struct with size, alignment, offsets.
- Table and field infrastructure (tasks TS‑1 … TS‑4).
