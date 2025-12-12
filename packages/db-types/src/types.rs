//! Type definitions and utilities.

use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// Represents a field type in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Bool,
}

impl Type {
    /// Returns the size in bytes of this type.
    pub fn size(&self) -> usize {
        match self {
            Type::I8 | Type::U8 | Type::Bool => 1,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::F64 => 8,
            Type::I128 | Type::U128 => 16,
        }
    }

    /// Returns the alignment requirement in bytes of this type.
    pub fn alignment(&self) -> usize {
        match self {
            Type::I8 | Type::U8 | Type::Bool => 1,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::F64 => 8,
            Type::I128 | Type::U128 => 16,
        }
    }
}

/// Calculates the offset after aligning `current_offset` to the alignment of `ty`.
///
/// Returns `Some(offset)` where offset is the smallest offset `>= current_offset`
/// that is a multiple of `ty.alignment()`. If `current_offset` is already aligned,
/// it is returned unchanged.
///
/// Returns `None` if the calculation would overflow `usize`.
///
/// # Examples
///
/// ```
/// use db_types::{Type, align_offset};
///
/// assert_eq!(align_offset(0, Type::I32), Some(0));
/// assert_eq!(align_offset(1, Type::I32), Some(4));
/// assert_eq!(align_offset(4, Type::I32), Some(4));
/// assert_eq!(align_offset(5, Type::I32), Some(8));
/// assert_eq!(align_offset(usize::MAX, Type::I32), None);
/// ```
pub fn align_offset(current_offset: usize, ty: Type) -> Option<usize> {
    let align = ty.alignment();
    if align == 1 {
        Some(current_offset)
    } else {
        current_offset
            .checked_add(align - 1)
            .map(|aligned_minus_one| (aligned_minus_one / align) * align)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hasher;

    #[test]
    fn test_primitive_type_sizes_and_alignments() {
        // Test i8
        assert_eq!(Type::I8.size(), 1);
        assert_eq!(Type::I8.alignment(), 1);

        // Test i16
        assert_eq!(Type::I16.size(), 2);
        assert_eq!(Type::I16.alignment(), 2);

        // Test i32
        assert_eq!(Type::I32.size(), 4);
        assert_eq!(Type::I32.alignment(), 4);

        // Test i64
        assert_eq!(Type::I64.size(), 8);
        assert_eq!(Type::I64.alignment(), 8);

        // Test i128
        assert_eq!(Type::I128.size(), 16);
        assert_eq!(Type::I128.alignment(), 16);

        // Test u8
        assert_eq!(Type::U8.size(), 1);
        assert_eq!(Type::U8.alignment(), 1);

        // Test u16
        assert_eq!(Type::U16.size(), 2);
        assert_eq!(Type::U16.alignment(), 2);

        // Test u32
        assert_eq!(Type::U32.size(), 4);
        assert_eq!(Type::U32.alignment(), 4);

        // Test u64
        assert_eq!(Type::U64.size(), 8);
        assert_eq!(Type::U64.alignment(), 8);

        // Test u128
        assert_eq!(Type::U128.size(), 16);
        assert_eq!(Type::U128.alignment(), 16);

        // Test f32
        assert_eq!(Type::F32.size(), 4);
        assert_eq!(Type::F32.alignment(), 4);

        // Test f64
        assert_eq!(Type::F64.size(), 8);
        assert_eq!(Type::F64.alignment(), 8);

        // Test bool
        assert_eq!(Type::Bool.size(), 1);
        assert_eq!(Type::Bool.alignment(), 1);
    }

    #[test]
    fn test_alignment_is_power_of_two_and_le_size() {
        let types = [
            Type::I8,
            Type::I16,
            Type::I32,
            Type::I64,
            Type::I128,
            Type::U8,
            Type::U16,
            Type::U32,
            Type::U64,
            Type::U128,
            Type::F32,
            Type::F64,
            Type::Bool,
        ];

        for ty in types {
            let align = ty.alignment();
            let size = ty.size();

            // Alignment must be a power of two
            assert!(
                align.is_power_of_two(),
                "Alignment for {:?} is not a power of two: {}",
                ty,
                align
            );

            // Size must be a multiple of alignment (for primitive types)
            assert_eq!(
                size % align,
                0,
                "Size {} not multiple of alignment {} for {:?}",
                size,
                align,
                ty
            );
        }
    }

    #[test]
    fn test_type_identity_and_equality() {
        // Same types should be equal
        assert_eq!(Type::I32, Type::I32);
        assert_eq!(Type::F64, Type::F64);
        assert_eq!(Type::Bool, Type::Bool);

        // Different types should not be equal
        assert_ne!(Type::I32, Type::U32);
        assert_ne!(Type::I32, Type::F32);
        assert_ne!(Type::I64, Type::U64);
        assert_ne!(Type::I8, Type::Bool);

        // Hash consistency
        use std::collections::hash_map::DefaultHasher;

        let mut hasher1 = DefaultHasher::new();
        Type::I32.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        Type::I32.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);

        let mut hasher3 = DefaultHasher::new();
        Type::U32.hash(&mut hasher3);
        let hash3 = hasher3.finish();

        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_type_serialization_roundtrip() {
        let types = [
            Type::I8,
            Type::I16,
            Type::I32,
            Type::I64,
            Type::I128,
            Type::U8,
            Type::U16,
            Type::U32,
            Type::U64,
            Type::U128,
            Type::F32,
            Type::F64,
            Type::Bool,
        ];

        for ty in types {
            let json = serde_json::to_string(&ty).unwrap();
            let decoded: Type = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, ty, "Failed roundtrip for {:?}", ty);
        }
    }

    #[test]
    fn test_offset_calculation_helper() {
        // Test with alignment = 1 (no adjustment)
        assert_eq!(align_offset(0, Type::U8), Some(0));
        assert_eq!(align_offset(5, Type::U8), Some(5));
        assert_eq!(align_offset(100, Type::U8), Some(100));

        // Test with alignment = 4
        assert_eq!(align_offset(0, Type::I32), Some(0));
        assert_eq!(align_offset(1, Type::I32), Some(4));
        assert_eq!(align_offset(4, Type::I32), Some(4));
        assert_eq!(align_offset(5, Type::I32), Some(8));
        assert_eq!(align_offset(8, Type::I32), Some(8));

        // Test with alignment = 8
        assert_eq!(align_offset(0, Type::I64), Some(0));
        assert_eq!(align_offset(1, Type::I64), Some(8));
        assert_eq!(align_offset(7, Type::I64), Some(8));
        assert_eq!(align_offset(8, Type::I64), Some(8));
        assert_eq!(align_offset(9, Type::I64), Some(16));

        // Test with alignment = 16
        assert_eq!(align_offset(0, Type::I128), Some(0));
        assert_eq!(align_offset(1, Type::I128), Some(16));
        assert_eq!(align_offset(15, Type::I128), Some(16));
        assert_eq!(align_offset(16, Type::I128), Some(16));
        assert_eq!(align_offset(17, Type::I128), Some(32));
    }

    #[test]
    fn test_align_offset_overflow() {
        // Test overflow case with maximum usize and alignment > 1
        let max_usize = usize::MAX;
        // This should return None because max_usize + (align - 1) would overflow
        assert_eq!(align_offset(max_usize, Type::I32), None);
        assert_eq!(align_offset(max_usize, Type::I64), None);
        assert_eq!(align_offset(max_usize, Type::I128), None);
    }

    #[test]
    fn test_align_offset_no_overflow_with_alignment_1() {
        // With alignment = 1, no addition happens, so no overflow
        let max_usize = usize::MAX;
        assert_eq!(align_offset(max_usize, Type::U8), Some(max_usize));
        assert_eq!(align_offset(max_usize, Type::Bool), Some(max_usize));
    }

    #[test]
    fn test_size_and_alignment_match_std_mem() {
        // Verify our sizes and alignments match Rust's std::mem values
        use std::mem;

        assert_eq!(Type::I8.size(), mem::size_of::<i8>());
        assert_eq!(Type::I8.alignment(), mem::align_of::<i8>());

        assert_eq!(Type::I16.size(), mem::size_of::<i16>());
        assert_eq!(Type::I16.alignment(), mem::align_of::<i16>());

        assert_eq!(Type::I32.size(), mem::size_of::<i32>());
        assert_eq!(Type::I32.alignment(), mem::align_of::<i32>());

        assert_eq!(Type::I64.size(), mem::size_of::<i64>());
        assert_eq!(Type::I64.alignment(), mem::align_of::<i64>());

        assert_eq!(Type::I128.size(), mem::size_of::<i128>());
        assert_eq!(Type::I128.alignment(), mem::align_of::<i128>());

        assert_eq!(Type::U8.size(), mem::size_of::<u8>());
        assert_eq!(Type::U8.alignment(), mem::align_of::<u8>());

        assert_eq!(Type::U16.size(), mem::size_of::<u16>());
        assert_eq!(Type::U16.alignment(), mem::align_of::<u16>());

        assert_eq!(Type::U32.size(), mem::size_of::<u32>());
        assert_eq!(Type::U32.alignment(), mem::align_of::<u32>());

        assert_eq!(Type::U64.size(), mem::size_of::<u64>());
        assert_eq!(Type::U64.alignment(), mem::align_of::<u64>());

        assert_eq!(Type::U128.size(), mem::size_of::<u128>());
        assert_eq!(Type::U128.alignment(), mem::align_of::<u128>());

        assert_eq!(Type::F32.size(), mem::size_of::<f32>());
        assert_eq!(Type::F32.alignment(), mem::align_of::<f32>());

        assert_eq!(Type::F64.size(), mem::size_of::<f64>());
        assert_eq!(Type::F64.alignment(), mem::align_of::<f64>());

        assert_eq!(Type::Bool.size(), mem::size_of::<bool>());
        assert_eq!(Type::Bool.alignment(), mem::align_of::<bool>());
    }
}
