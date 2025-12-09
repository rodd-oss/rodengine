use crate::error::{EcsDbError, Result};
use bincode;
use serde;

use std::mem;

/// Encode a Rust value into bytes using bincode serialization.
pub fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    bincode::serialize(value).map_err(EcsDbError::SerializationError)
}

/// Decode bytes into a Rust value using bincode deserialization.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    bincode::deserialize(bytes).map_err(EcsDbError::SerializationError)
}

/// Unsafe zero-copy casting from bytes to a reference of type T.
/// # Safety
/// - The byte slice must be valid for type T (correct size, alignment, and representation).
/// - The lifetime of the returned reference must not outlive the underlying bytes.
/// - The bytes must not be mutated while the reference exists.
pub unsafe fn cast_bytes_to_ref<T>(bytes: &[u8]) -> Result<&T> {
    if bytes.len() != mem::size_of::<T>() {
        return Err(EcsDbError::FieldTypeMismatch {
            expected: format!("{} bytes", mem::size_of::<T>()),
            got: format!("{} bytes", bytes.len()),
        });
    }

    let ptr = bytes.as_ptr() as *const T;
    if !(ptr as usize).is_multiple_of(mem::align_of::<T>()) {
        return Err(EcsDbError::AlignmentError {
            offset: ptr as usize % mem::align_of::<T>(),
        });
    }

    Ok(&*ptr)
}

/// Unsafe zero-copy casting from bytes to a mutable reference of type T.
/// # Safety
/// - Same as `cast_bytes_to_ref`, plus exclusive access.
pub unsafe fn cast_bytes_to_mut<T>(bytes: &mut [u8]) -> Result<&mut T> {
    if bytes.len() != mem::size_of::<T>() {
        return Err(EcsDbError::FieldTypeMismatch {
            expected: format!("{} bytes", mem::size_of::<T>()),
            got: format!("{} bytes", bytes.len()),
        });
    }

    let ptr = bytes.as_mut_ptr() as *mut T;
    if !(ptr as usize).is_multiple_of(mem::align_of::<T>()) {
        return Err(EcsDbError::AlignmentError {
            offset: ptr as usize % mem::align_of::<T>(),
        });
    }

    Ok(&mut *ptr)
}

/// Compute the size and alignment of a type at compile time.
pub fn size_and_align_of<T>() -> (usize, usize) {
    (mem::size_of::<T>(), mem::align_of::<T>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
    struct TestComponent {
        x: f32,
        y: f32,
        id: u32,
    }

    #[test]
    fn test_encode_decode() -> Result<()> {
        let comp = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 42,
        };
        let bytes = encode(&comp)?;
        let decoded: TestComponent = decode(&bytes)?;
        assert_eq!(comp, decoded);
        Ok(())
    }

    #[test]
    fn test_cast_bytes() -> Result<()> {
        let comp = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 42,
        };
        let _bytes = encode(&comp)?;

        // This is safe because we just encoded the struct with bincode,
        // but bincode encoding may not match memory layout.
        // For testing, we'll use raw bytes of the struct.
        let raw_bytes = unsafe {
            std::slice::from_raw_parts(
                &comp as *const TestComponent as *const u8,
                mem::size_of::<TestComponent>(),
            )
        };

        unsafe {
            let casted = cast_bytes_to_ref::<TestComponent>(raw_bytes)?;
            assert_eq!(casted.x, 1.0);
            assert_eq!(casted.y, 2.0);
            assert_eq!(casted.id, 42);
        }

        Ok(())
    }
}
