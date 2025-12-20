use std::any::TypeId;

use super::error::TypeError;
use super::type_registration::{DeserializerFn, SerializerFn};

/// Layout information for a data type.
///
/// Contains size, alignment, POD flag, and serialization function.
/// Used for field offset calculation and memory layout validation.
#[derive(Clone)]
pub struct TypeLayout {
    /// Type identifier (e.g., "u64", "string", "3xf32")
    pub type_id: String,
    /// Size in bytes
    pub size: usize,
    /// Alignment requirement in bytes
    pub align: usize,
    /// Whether the type is Plain Old Data (Copy + 'static)
    pub pod: bool,
    /// Serializer function: writes bytes from source pointer to destination buffer
    pub serializer: std::sync::Arc<SerializerFn>,
    /// Deserializer function: reads bytes from source buffer to destination pointer
    pub deserializer: std::sync::Arc<DeserializerFn>,
    /// Runtime type ID for POD types
    pub type_id_internal: Option<TypeId>,
}

impl std::fmt::Debug for TypeLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeLayout")
            .field("type_id", &self.type_id)
            .field("size", &self.size)
            .field("align", &self.align)
            .field("pod", &self.pod)
            .field("type_id_internal", &self.type_id_internal)
            .finish_non_exhaustive()
    }
}

impl TypeLayout {
    /// Creates a new type layout.
    ///
    /// # Arguments
    /// * `type_id` - Type identifier string
    /// * `size` - Size in bytes
    /// * `align` - Alignment requirement in bytes
    /// * `pod` - Whether the type is Plain Old Data
    /// * `serializer` - Function to serialize from pointer to buffer
    /// * `deserializer` - Function to deserialize from buffer to pointer
    /// * `type_id_internal` - Runtime type ID for POD types
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `size % align == 0`
    /// - `serializer` and `deserializer` are thread-safe
    /// - For POD types, `type_id_internal` must match the actual type
    pub unsafe fn new(
        type_id: String,
        size: usize,
        align: usize,
        pod: bool,
        serializer: impl Fn(*const u8, &mut Vec<u8>) -> usize + Send + Sync + 'static,
        deserializer: impl Fn(&[u8], *mut u8) -> usize + Send + Sync + 'static,
        type_id_internal: Option<TypeId>,
    ) -> Self {
        Self {
            type_id,
            size,
            align,
            pod,
            serializer: std::sync::Arc::new(serializer),
            deserializer: std::sync::Arc::new(deserializer),
            type_id_internal,
        }
    }

    /// Validates that the layout is consistent.
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err(TypeError)` otherwise.
    pub fn validate(&self) -> Result<(), TypeError> {
        // POD types must have non-zero size
        if self.pod && self.size == 0 {
            return Err(TypeError::InvalidSize {
                type_id: self.type_id.clone(),
                size: self.size,
            });
        }

        if self.align == 0 {
            return Err(TypeError::InvalidAlignment {
                type_id: self.type_id.clone(),
                align: self.align,
            });
        }

        // Non-zero sized types must have size divisible by alignment
        if self.size > 0 && !self.size.is_multiple_of(self.align) {
            return Err(TypeError::SizeAlignmentMismatch {
                type_id: self.type_id.clone(),
                size: self.size,
                align: self.align,
            });
        }

        if self.pod && self.type_id_internal.is_none() {
            return Err(TypeError::MissingTypeId {
                type_id: self.type_id.clone(),
            });
        }

        Ok(())
    }

    /// Serializes data from a source pointer into a destination buffer.
    ///
    /// # Arguments
    /// * `src` - Source pointer to data
    /// * `dst` - Destination buffer
    ///
    /// # Returns
    /// Number of bytes written.
    ///
    /// # Safety
    /// Caller must ensure `src` points to valid data of this type.
    pub unsafe fn serialize(&self, src: *const u8, dst: &mut Vec<u8>) -> usize {
        (self.serializer)(src, dst)
    }

    /// Deserializes data from a source buffer into a destination pointer.
    ///
    /// # Arguments
    /// * `src` - Source buffer slice
    /// * `dst` - Destination pointer
    ///
    /// # Returns
    /// Number of bytes read.
    ///
    /// # Safety
    /// Caller must ensure `dst` points to valid memory of this type.
    pub unsafe fn deserialize(&self, src: &[u8], dst: *mut u8) -> usize {
        (self.deserializer)(src, dst)
    }
}
