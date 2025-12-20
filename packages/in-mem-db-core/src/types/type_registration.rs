use std::any::TypeId;

/// Type alias for serializer function signature.
pub type SerializerFn = dyn Fn(*const u8, &mut Vec<u8>) -> usize + Send + Sync;

/// Type alias for deserializer function signature.
pub type DeserializerFn = dyn Fn(&[u8], *mut u8) -> usize + Send + Sync;

/// Parameters for registering a new type.
#[derive(Clone)]
pub struct TypeRegistration {
    /// Type identifier string
    pub type_id: String,
    /// Size in bytes
    pub size: usize,
    /// Alignment requirement in bytes
    pub align: usize,
    /// Whether the type is Plain Old Data (Copy + 'static)
    pub pod: bool,
    /// Function to serialize from pointer to buffer
    pub serializer: std::sync::Arc<SerializerFn>,
    /// Function to deserialize from buffer to pointer
    pub deserializer: std::sync::Arc<DeserializerFn>,
    /// Runtime type ID for POD types (None for non-POD)
    pub type_id_internal: Option<TypeId>,
}

impl TypeRegistration {
    /// Creates a new type registration.
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
}
