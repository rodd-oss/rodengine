//! Type definitions and utilities.

use std::fmt;

/// Value representation for database fields.
///
/// This enum can hold any value that corresponds to a [`Type`] variant.
/// Used for reading and writing field values to/from storage buffers.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 8-bit signed integer
    I8(i8),
    /// 16-bit signed integer
    I16(i16),
    /// 32-bit signed integer
    I32(i32),
    /// 64-bit signed integer
    I64(i64),
    /// 128-bit signed integer
    I128(i128),
    /// 8-bit unsigned integer
    U8(u8),
    /// 16-bit unsigned integer
    U16(u16),
    /// 32-bit unsigned integer
    U32(u32),
    /// 64-bit unsigned integer
    U64(u64),
    /// 128-bit unsigned integer
    U128(u128),
    /// 32-bit floating point number
    F32(f32),
    /// 64-bit floating point number
    F64(f64),
    /// Boolean value
    Bool(bool),
    /// UTF-8 string
    String(String),
}

impl Value {
    /// Returns the type of this value.
    pub fn ty(&self) -> Type {
        match self {
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::I64(_) => Type::I64,
            Value::I128(_) => Type::I128,
            Value::U8(_) => Type::U8,
            Value::U16(_) => Type::U16,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::U128(_) => Type::U128,
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            Value::Bool(_) => Type::Bool,
            Value::String(_) => Type::String,
        }
    }
}

/// Primitive field types supported by the database.
///
/// Each variant represents a Rust primitive type that can be stored in a table.
/// The database uses these types for field definitions and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// 8-bit signed integer
    I8,
    /// 16-bit signed integer
    I16,
    /// 32-bit signed integer
    I32,
    /// 64-bit signed integer
    I64,
    /// 128-bit signed integer
    I128,
    /// 8-bit unsigned integer
    U8,
    /// 16-bit unsigned integer
    U16,
    /// 32-bit unsigned integer
    U32,
    /// 64-bit unsigned integer
    U64,
    /// 128-bit unsigned integer
    U128,
    /// 32-bit floating point number
    F32,
    /// 64-bit floating point number
    F64,
    /// Boolean value
    Bool,
    /// UTF-8 string (stored as length-prefixed bytes)
    String,
}

impl Type {
    /// Returns the size in bytes of this type.
    ///
    /// For `Type::String`, returns the size of the length prefix (8 bytes).
    /// The actual string data is stored separately.
    pub fn size(&self) -> usize {
        match self {
            Type::I8 | Type::U8 | Type::Bool => 1,
            Type::I16 | Type::U16 => 2,
            Type::I32 | Type::U32 | Type::F32 => 4,
            Type::I64 | Type::U64 | Type::F64 => 8,
            Type::I128 | Type::U128 => 16,
            Type::String => 8, // length prefix as u64
        }
    }

    /// Returns the alignment requirement of this type.
    ///
    /// Alignment is the same as size for primitive types.
    pub fn alignment(&self) -> usize {
        self.size()
    }

    /// Returns `true` if this type is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
        )
    }

    /// Returns `true` if this type is a floating point type.
    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    /// Returns `true` if this type is numeric (integer or float).
    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I8 => write!(f, "i8"),
            Type::I16 => write!(f, "i16"),
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::I128 => write!(f, "i128"),
            Type::U8 => write!(f, "u8"),
            Type::U16 => write!(f, "u16"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::U128 => write!(f, "u128"),
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_size_01() {
        assert_eq!(Type::I32.size(), 4);
    }

    #[test]
    fn test_type_size_02() {
        assert_eq!(Type::F32.size(), 4);
    }

    #[test]
    fn test_type_size_03() {
        assert_eq!(Type::Bool.size(), 1);
    }

    #[test]
    fn test_type_size_04() {
        assert_eq!(Type::U64.size(), 8);
    }

    #[test]
    fn test_type_alignment_01() {
        assert_eq!(Type::I32.alignment(), 4);
    }

    #[test]
    fn test_type_alignment_02() {
        assert_eq!(Type::F32.alignment(), 4);
    }

    #[test]
    fn test_type_alignment_03() {
        assert_eq!(Type::Bool.alignment(), 1);
    }

    #[test]
    fn test_type_alignment_04() {
        assert_eq!(Type::U64.alignment(), 8);
    }

    #[test]
    fn test_type_is_integer() {
        assert!(Type::I32.is_integer());
        assert!(Type::U64.is_integer());
        assert!(!Type::F32.is_integer());
        assert!(!Type::Bool.is_integer());
        assert!(!Type::String.is_integer());
    }

    #[test]
    fn test_type_is_float() {
        assert!(Type::F32.is_float());
        assert!(Type::F64.is_float());
        assert!(!Type::I32.is_float());
        assert!(!Type::Bool.is_float());
    }

    #[test]
    fn test_type_is_numeric() {
        assert!(Type::I32.is_numeric());
        assert!(Type::U64.is_numeric());
        assert!(Type::F32.is_numeric());
        assert!(!Type::Bool.is_numeric());
        assert!(!Type::String.is_numeric());
    }

    #[test]
    fn test_type_display() {
        assert_eq!(Type::I32.to_string(), "i32");
        assert_eq!(Type::F64.to_string(), "f64");
        assert_eq!(Type::Bool.to_string(), "bool");
        assert_eq!(Type::String.to_string(), "string");
    }
}
