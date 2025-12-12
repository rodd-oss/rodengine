# Test Plan: task_ce_1 - Verify record packing eliminates padding (field alignment = 1)

## Context

Part of relational in-memory database for online games (Rust). Storage uses `Vec<u8>` buffer with tight packing for CPU cache locality. This task verifies that field alignment is forced to 1, eliminating all padding between fields.

## Test Suite

### 1. test_record_packing_basic_types

**Verifies**: Basic scalar types pack without padding when placed consecutively.
**Fields**: Sequence of [u8, i16, i32, f64, bool]
**Expected offsets**: 0, 1, 3, 7, 15 (not 0, 2, 4, 8, 16 with natural alignment)
**Assertions**:

- `record_size == 16` (1+2+4+8+1)
- Each field offset equals sum of previous field sizes

### 2. test_record_packing_mixed_alignment

**Verifies**: Fields with different natural alignments pack with alignment=1.
**Fields**: [u8, u32, u8, u64] (u32 normally 4-byte aligned, u64 8-byte aligned)
**Expected offsets**: 0, 1, 5, 6
**Assertions**:

- u32 at offset 1 is allowed (unaligned)
- u64 at offset 6 is allowed (unaligned)
- No gaps between fields

### 3. test_record_packing_custom_composite

**Verifies**: User-defined composite types pack without internal padding.
**Fields**: Custom type `Vec3` (3×f32) followed by `Color` (4×u8)
**Expected**: Vec3 size = 12 bytes, Color size = 4 bytes
**Assertions**:

- Composite type size equals sum of component sizes
- No padding between composite type components
- No padding between composite type and next field

### 4. test_record_packing_zero_sized_fields

**Verifies**: Zero-sized types don't affect packing or offsets.
**Fields**: [u8, PhantomData<()>, i32, ()]
**Expected offsets**: 0, 1, 1, 5
**Assertions**:

- Zero-sized fields occupy 0 bytes
- Offsets skip over zero-sized fields
- Record size = 5 bytes (1 + 0 + 4 + 0)

### 5. test_record_packing_edge_cases

**Verifies**: Extreme packing scenarios work correctly.
**Test cases**:

1. Single field record: [u64] → size=8, offset=0
2. Many small fields: 100×[u8] → size=100, each offset increments by 1
3. Alternating sizes: [u8, u64, u8, u64] → offsets 0,1,9,10
4. Maximum practical field count (stress test)
   **Assertions**:

- All offsets follow tight packing
- Buffer bounds respected
- No arithmetic overflow in size calculations

### 6. test_record_packing_verify_offsets

**Verifies**: Mathematical correctness of offset calculations.
**Method**: Generate random field sequences, compute expected offsets
**Assertions**:

- `offset_n = Σ(size_i) for i=0..n-1`
- `record_size = Σ(all_field_sizes)`
- Field access via computed offsets returns correct values

## Edge Cases to Test

### Alignment Issues

- **Unaligned access**: Verify system handles fields at odd addresses
- **Cross-architecture**: x86/x64 vs ARM alignment requirements
- **Compiler directives**: Effect of `#[repr(packed)]` vs manual byte access

### Type System Edge Cases

- **#[repr(align)] types**: User types with explicit alignment requirements
- **#[repr(C)] types**: C-compatible types with different packing rules
- **Enums**: Size depends on discriminant + largest variant
- **Arrays**: `[T; N]` should pack as N×size_of::<T>() without padding

### Buffer Management

- **Empty records**: 0-byte records for tables with no fields
- **Buffer resizing**: Packing preserved after buffer reallocation
- **Partial writes**: Writing individual fields doesn't corrupt neighbors

### Performance Considerations

- **Cache line straddling**: Fields may cross cache line boundaries
- **Access patterns**: Sequential vs random field access performance
- **Memory barriers**: Packed access with concurrent modifications

## Implementation Notes

### Rust-Specific Concerns

1. **Unaligned access**: May require `ptr::read_unaligned`/`ptr::write_unaligned`
2. **#[repr(packed)]**: Can cause performance penalties or undefined behavior
3. **Safe abstractions**: Need to ensure memory safety despite unaligned access
4. **Zero-copy**: References to packed fields must respect Rust's aliasing rules

### Validation Strategy

1. **Write/readback**: Write known patterns, read back verify bit-exact match
2. **Offset verification**: Independent calculation of expected offsets
3. **Bounds checking**: All field accesses stay within buffer
4. **Concurrency**: Packing works with ArcSwap buffer swapping

### Expected Failures (to catch)

- Padding inserted between differently sized fields
- Alignment rounding up to next power-of-two boundary
- Composite types adding padding for alignment
- Zero-sized fields consuming space
- Size calculations that include hidden padding bytes
