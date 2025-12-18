//! Type system, layout calculation, and custom type registration.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
    pub serializer: Arc<SerializerFn>,
    /// Function to deserialize from buffer to pointer
    pub deserializer: Arc<DeserializerFn>,
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
            serializer: Arc::new(serializer),
            deserializer: Arc::new(deserializer),
            type_id_internal,
        }
    }
}

/// Registry for type layouts.
///
/// Stores registered types with lookup by type identifier.
/// Provides thread-safe registration and retrieval.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    types: RwLock<HashMap<String, TypeLayout>>,
}

impl TypeRegistry {
    /// Creates a new empty type registry.
    pub fn new() -> Self {
        Self {
            types: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a type layout.
    ///
    /// # Arguments
    /// * `layout` - Type layout to register
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(TypeError)` if type already registered or invalid.
    pub fn register(&self, layout: TypeLayout) -> Result<(), TypeError> {
        layout.validate()?;

        let mut types = self
            .types
            .write()
            .map_err(|_| TypeError::ValidationFailed {
                type_id: layout.type_id.clone(),
                message: "failed to acquire write lock".to_string(),
            })?;

        if types.contains_key(&layout.type_id) {
            return Err(TypeError::AlreadyRegistered {
                type_id: layout.type_id.clone(),
            });
        }

        types.insert(layout.type_id.clone(), layout);
        Ok(())
    }

    /// Retrieves a type layout by identifier.
    ///
    /// # Arguments
    /// * `type_id` - Type identifier
    ///
    /// # Returns
    /// `Some(&TypeLayout)` if found, `None` otherwise.
    pub fn get(&self, type_id: &str) -> Option<TypeLayout> {
        let types = self.types.read().ok()?;
        types.get(type_id).cloned()
    }

    /// Checks if a type is registered.
    pub fn contains(&self, type_id: &str) -> bool {
        let types = match self.types.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        types.contains_key(type_id)
    }

    /// Returns all registered type identifiers.
    pub fn type_ids(&self) -> Vec<String> {
        let types = match self.types.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };
        types.keys().cloned().collect()
    }

    /// Removes a type registration.
    ///
    /// # Arguments
    /// * `type_id` - Type identifier to remove
    ///
    /// # Returns
    /// `true` if the type was removed, `false` if it wasn't found.
    pub fn remove(&self, type_id: &str) -> bool {
        let mut types = match self.types.write() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        types.remove(type_id).is_some()
    }

    /// Validates that a type is registered and matches expected properties.
    ///
    /// # Arguments
    /// * `type_id` - Type identifier
    /// * `expected_size` - Expected size in bytes (optional)
    /// * `expected_align` - Expected alignment in bytes (optional)
    /// * `expected_pod` - Expected POD flag (optional)
    ///
    /// # Returns
    /// `Ok(TypeLayout)` if validation passes, `Err(TypeError)` otherwise.
    pub fn validate_type(
        &self,
        type_id: &str,
        expected_size: Option<usize>,
        expected_align: Option<usize>,
        expected_pod: Option<bool>,
    ) -> Result<TypeLayout, TypeError> {
        let layout = self.get(type_id).ok_or_else(|| TypeError::NotFound {
            type_id: type_id.to_string(),
        })?;

        if let Some(size) = expected_size {
            if layout.size != size {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!("size mismatch: expected {}, got {}", size, layout.size),
                });
            }
        }

        if let Some(align) = expected_align {
            if layout.align != align {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!(
                        "alignment mismatch: expected {}, got {}",
                        align, layout.align
                    ),
                });
            }
        }

        if let Some(pod) = expected_pod {
            if layout.pod != pod {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!("POD flag mismatch: expected {}, got {}", pod, layout.pod),
                });
            }
        }

        Ok(layout)
    }

    /// Ensures a type is registered, registering it if necessary from schema information.
    ///
    /// This is used when loading a schema to ensure all required types exist.
    /// For POD types loaded from schema, TypeId may be None.
    ///
    /// # Arguments
    /// * `type_id` - Type identifier
    /// * `size` - Size in bytes
    /// * `align` - Alignment requirement
    /// * `pod` - Whether the type is POD
    ///
    /// # Returns
    /// `Ok(())` if type is registered or was successfully registered.
    pub fn ensure_type_registered(
        &self,
        type_id: &str,
        size: usize,
        align: usize,
        pod: bool,
    ) -> Result<(), TypeError> {
        // Check if type is already registered
        if let Some(existing) = self.get(type_id) {
            // Validate it matches the schema
            if existing.size != size {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!("size mismatch: expected {}, got {}", size, existing.size),
                });
            }
            if existing.align != align {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!(
                        "alignment mismatch: expected {}, got {}",
                        align, existing.align
                    ),
                });
            }
            if existing.pod != pod {
                return Err(TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: format!("POD flag mismatch: expected {}, got {}", pod, existing.pod),
                });
            }
            return Ok(());
        }

        // Type not registered, register it from schema
        // Note: For POD types, we can't provide TypeId when loading from schema
        unsafe {
            let layout = TypeLayout::new(
                type_id.to_string(),
                size,
                align,
                pod,
                move |src, dst| {
                    // Default serializer: copy bytes
                    dst.extend_from_slice(std::slice::from_raw_parts(src, size));
                    size
                },
                move |src, dst| {
                    // Default deserializer: copy bytes
                    if src.len() >= size {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, size);
                        size
                    } else {
                        0
                    }
                },
                None, // No TypeId for schema-loaded types
            );

            // Skip validation for schema-loaded types since they won't have TypeId
            // We'll manually check the important constraints
            if align == 0 {
                return Err(TypeError::InvalidAlignment {
                    type_id: type_id.to_string(),
                    align,
                });
            }

            if size > 0 && !size.is_multiple_of(align) {
                return Err(TypeError::SizeAlignmentMismatch {
                    type_id: type_id.to_string(),
                    size,
                    align,
                });
            }

            // Register without validation
            let mut types = self
                .types
                .write()
                .map_err(|_| TypeError::ValidationFailed {
                    type_id: type_id.to_string(),
                    message: "failed to acquire write lock".to_string(),
                })?;

            types.insert(type_id.to_string(), layout);
            Ok(())
        }
    }
}

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
    pub serializer: Arc<SerializerFn>,
    /// Deserializer function: reads bytes from source buffer to destination pointer
    pub deserializer: Arc<DeserializerFn>,
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
            serializer: Arc::new(serializer),
            deserializer: Arc::new(deserializer),
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

/// Error type for type registration and validation.
#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error("Type '{type_id}' has invalid size: {size}")]
    InvalidSize { type_id: String, size: usize },

    #[error("Type '{type_id}' has invalid alignment: {align}")]
    InvalidAlignment { type_id: String, align: usize },

    #[error("Type '{type_id}' size {size} not divisible by alignment {align}")]
    SizeAlignmentMismatch {
        type_id: String,
        size: usize,
        align: usize,
    },

    #[error("POD type '{type_id}' missing internal TypeId")]
    MissingTypeId { type_id: String },

    #[error("Type '{type_id}' already registered")]
    AlreadyRegistered { type_id: String },

    #[error("Type '{type_id}' not found")]
    NotFound { type_id: String },

    #[error("Type validation failed: {message}")]
    ValidationFailed { type_id: String, message: String },
}

// Helper functions for built-in type serialization/deserialization

/// Serializer for fixed-size types (memcpy).
unsafe fn copy_serializer<T: Copy>(src: *const u8, dst: &mut Vec<u8>) -> usize {
    let size = std::mem::size_of::<T>();
    let slice = std::slice::from_raw_parts(src, size);
    dst.extend_from_slice(slice);
    size
}

/// Deserializer for fixed-size types (memcpy).
unsafe fn copy_deserializer<T: Copy>(src: &[u8], dst: *mut u8) -> usize {
    let size = std::mem::size_of::<T>();
    if src.len() < size {
        return 0;
    }
    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, size);
    size
}

/// Serializer for bool (1 byte, 0=false, 1=true).
unsafe fn bool_serializer(src: *const u8, dst: &mut Vec<u8>) -> usize {
    let value_ptr = src as *const bool;
    let value = *value_ptr;
    dst.push(if value { 1 } else { 0 });
    1
}

/// Deserializer for bool (1 byte, 0=false, 1=true).
unsafe fn bool_deserializer(src: &[u8], dst: *mut u8) -> usize {
    if src.is_empty() {
        return 0;
    }
    let dst_ptr = dst as *mut bool;
    *dst_ptr = src[0] != 0;
    1
}

/// Serializer for string (length-prefixed UTF-8 with maximum size).
unsafe fn string_serializer(src: *const u8, dst: &mut Vec<u8>) -> usize {
    let string_ptr = src as *const String;
    let string = &*string_ptr;

    // Validate string length fits within maximum size (256 bytes for string data)
    let max_string_data_size = 256;
    if string.len() > max_string_data_size {
        // Truncate to maximum size
        let truncated_len = max_string_data_size;
        let truncated_bytes = &string.as_bytes()[..truncated_len];

        // Write length as u32 (4 bytes)
        dst.extend_from_slice(&(truncated_len as u32).to_ne_bytes());

        // Write truncated UTF-8 bytes
        dst.extend_from_slice(truncated_bytes);

        // Pad with zeros to reach total field size (4 + 256 = 260 bytes)
        let padding = max_string_data_size - truncated_len;
        if padding > 0 {
            dst.extend(std::iter::repeat_n(0u8, padding));
        }

        4 + max_string_data_size
    } else {
        // Write length as u32 (4 bytes)
        let len = string.len() as u32;
        dst.extend_from_slice(&len.to_ne_bytes());

        // Write UTF-8 bytes
        dst.extend_from_slice(string.as_bytes());

        // Pad with zeros to reach total field size (4 + 256 = 260 bytes)
        let padding = max_string_data_size - string.len();
        if padding > 0 {
            dst.extend(std::iter::repeat_n(0u8, padding));
        }

        4 + max_string_data_size
    }
}

/// Deserializer for string (length-prefixed UTF-8 with maximum size).
unsafe fn string_deserializer(src: &[u8], dst: *mut u8) -> usize {
    // String field size is 260 bytes (4 bytes length + 256 bytes data)
    let total_field_size = 260;
    if src.len() < total_field_size {
        return 0;
    }

    // Read length (first 4 bytes)
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&src[..4]);
    let len = u32::from_ne_bytes(len_bytes) as usize;

    // Validate length doesn't exceed maximum
    let max_string_data_size = 256;
    let actual_len = len.min(max_string_data_size);

    // Read string bytes (bytes 4..4+actual_len)
    let string_bytes = &src[4..4 + actual_len];
    let string = String::from_utf8_lossy(string_bytes).to_string();

    let dst_ptr = dst as *mut String;
    *dst_ptr = string;

    total_field_size
}

/// Registers a custom type with the registry.
///
/// # Arguments
/// * `registry` - Type registry to register with
/// * `registration` - Type registration parameters
///
/// # Returns
/// `Ok(())` if registration successful, `Err(TypeError)` otherwise.
///
/// # Safety
/// Caller must ensure:
/// - `size % align == 0`
/// - `serializer` and `deserializer` are thread-safe
/// - For POD types, `type_id_internal` must match the actual type
/// - Serializer/deserializer functions must handle the correct data size
pub unsafe fn register_type(
    registry: &TypeRegistry,
    registration: TypeRegistration,
) -> Result<(), TypeError> {
    let layout = TypeLayout::new(
        registration.type_id,
        registration.size,
        registration.align,
        registration.pod,
        move |src, dst| (registration.serializer)(src, dst),
        move |src, dst| (registration.deserializer)(src, dst),
        registration.type_id_internal,
    );
    registry.register(layout)
}

/// Registers all built-in types in the registry.
///
/// # Arguments
/// * `registry` - Type registry to populate
///
/// # Returns
/// `Ok(())` if all types registered successfully.
pub fn register_builtin_types(registry: &TypeRegistry) -> Result<(), TypeError> {
    // Numeric types
    register_numeric_type::<i8>("i8", registry)?;
    register_numeric_type::<i16>("i16", registry)?;
    register_numeric_type::<i32>("i32", registry)?;
    register_numeric_type::<i64>("i64", registry)?;

    register_numeric_type::<u8>("u8", registry)?;
    register_numeric_type::<u16>("u16", registry)?;
    register_numeric_type::<u32>("u32", registry)?;
    register_numeric_type::<u64>("u64", registry)?;

    register_numeric_type::<f32>("f32", registry)?;
    register_numeric_type::<f64>("f64", registry)?;

    // Bool type
    let bool_layout = unsafe {
        TypeLayout::new(
            "bool".to_string(),
            std::mem::size_of::<bool>(),
            std::mem::align_of::<bool>(),
            true,
            move |src, dst| bool_serializer(src, dst),
            move |src, dst| bool_deserializer(src, dst),
            Some(TypeId::of::<bool>()),
        )
    };
    registry.register(bool_layout)?;

    // String type (not POD) - fixed size: 260 bytes (4 bytes length + 256 bytes data)
    let string_layout = unsafe {
        TypeLayout::new(
            "string".to_string(),
            260,   // Fixed size: 4 bytes length + 256 bytes string data
            1,     // Byte alignment
            false, // Not POD (contains heap allocation)
            move |src, dst| string_serializer(src, dst),
            move |src, dst| string_deserializer(src, dst),
            None, // No TypeId for non-POD types
        )
    };
    registry.register(string_layout)?;

    Ok(())
}

/// Registers a 3xf32 composite type (three f32 values).
///
/// # Arguments
/// * `registry` - Type registry to register with
///
/// # Returns
/// `Ok(())` if registration successful.
///
/// # Example
/// ```
/// use in_mem_db_core::types::{TypeRegistry, register_3xf32_type};
/// let registry = TypeRegistry::new();
/// assert!(register_3xf32_type(&registry).is_ok());
/// ```
pub fn register_3xf32_type(registry: &TypeRegistry) -> Result<(), TypeError> {
    unsafe {
        let registration = TypeRegistration::new(
            "3xf32".to_string(),
            12, // 3 * 4 bytes
            4,  // f32 alignment
            true,
            |src, dst| {
                // Copy 12 bytes (3 f32 values)
                let slice = std::slice::from_raw_parts(src, 12);
                dst.extend_from_slice(slice);
                12
            },
            |src, dst| {
                if src.len() >= 12 {
                    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 12);
                    12
                } else {
                    0
                }
            },
            Some(TypeId::of::<[f32; 3]>()),
        );
        register_type(registry, registration)
    }
}

/// Helper to register a numeric type.
fn register_numeric_type<T: Copy + 'static>(
    type_id: &str,
    registry: &TypeRegistry,
) -> Result<(), TypeError> {
    let layout = unsafe {
        TypeLayout::new(
            type_id.to_string(),
            std::mem::size_of::<T>(),
            std::mem::align_of::<T>(),
            true,
            move |src, dst| copy_serializer::<T>(src, dst),
            move |src, dst| copy_deserializer::<T>(src, dst),
            Some(TypeId::of::<T>()),
        )
    };
    registry.register(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_layout_validation() {
        // Valid layout
        let layout = unsafe {
            TypeLayout::new(
                "test".to_string(),
                8,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                    8
                },
                |src, dst| {
                    if src.len() >= 8 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                        8
                    } else {
                        0
                    }
                },
                Some(TypeId::of::<u64>()),
            )
        };
        assert!(layout.validate().is_ok());

        // Invalid: size not divisible by alignment
        let layout = unsafe {
            TypeLayout::new(
                "test".to_string(),
                7,
                8,
                true,
                |_, _| 0,
                |_, _| 0,
                Some(TypeId::of::<u64>()),
            )
        };
        assert!(layout.validate().is_err());

        // Invalid: POD type missing TypeId
        let layout =
            unsafe { TypeLayout::new("test".to_string(), 8, 8, true, |_, _| 0, |_, _| 0, None) };
        assert!(layout.validate().is_err());
    }

    #[test]
    fn test_type_registry_basic() {
        let registry = TypeRegistry::new();

        // Register a type
        let layout = unsafe {
            TypeLayout::new(
                "custom".to_string(),
                16,
                8,
                true,
                |src, dst| {
                    dst.extend_from_slice(std::slice::from_raw_parts(src, 16));
                    16
                },
                |src, dst| {
                    if src.len() >= 16 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 16);
                        16
                    } else {
                        0
                    }
                },
                Some(TypeId::of::<[u64; 2]>()),
            )
        };

        assert!(registry.register(layout).is_ok());
        assert!(registry.contains("custom"));
        assert!(!registry.contains("nonexistent"));

        // Can't register same type twice
        let layout2 = unsafe {
            TypeLayout::new(
                "custom".to_string(),
                16,
                8,
                true,
                |_, _| 0,
                |_, _| 0,
                Some(TypeId::of::<[u64; 2]>()),
            )
        };
        assert!(registry.register(layout2).is_err());

        // Retrieve type
        let retrieved = registry.get("custom").unwrap();
        assert_eq!(retrieved.type_id, "custom");
        assert_eq!(retrieved.size, 16);
        assert_eq!(retrieved.align, 8);
        assert!(retrieved.pod);

        // Remove type
        assert!(registry.remove("custom"));
        assert!(!registry.contains("custom"));
        assert!(!registry.remove("custom")); // Already removed
    }

    #[test]
    fn test_builtin_types_registration() {
        let registry = TypeRegistry::new();
        let result = register_builtin_types(&registry);
        if let Err(e) = &result {
            println!("Registration error: {}", e);
        }
        assert!(result.is_ok());

        // Check numeric types
        assert!(registry.contains("i8"));
        assert!(registry.contains("i16"));
        assert!(registry.contains("i32"));
        assert!(registry.contains("i64"));
        assert!(registry.contains("u8"));
        assert!(registry.contains("u16"));
        assert!(registry.contains("u32"));
        assert!(registry.contains("u64"));
        assert!(registry.contains("f32"));
        assert!(registry.contains("f64"));

        // Check bool
        let bool_layout = registry.get("bool").unwrap();
        assert_eq!(bool_layout.size, 1);
        assert_eq!(bool_layout.align, 1);
        assert!(bool_layout.pod);

        // Check string
        let string_layout = registry.get("string").unwrap();
        assert_eq!(string_layout.size, 260); // Fixed size: 4 bytes length + 256 bytes data
        assert_eq!(string_layout.align, 1);
        assert!(!string_layout.pod); // String is not POD
    }

    #[test]
    fn test_serialization_deserialization() {
        let registry = TypeRegistry::new();
        assert!(register_builtin_types(&registry).is_ok());

        // Test u64 serialization
        let u64_layout = registry.get("u64").unwrap();
        let value: u64 = 0x1234567890ABCDEF;
        let mut buffer = Vec::new();

        unsafe {
            let bytes_written = u64_layout.serialize(&value as *const _ as *const u8, &mut buffer);
            assert_eq!(bytes_written, 8);
            assert_eq!(buffer.len(), 8);

            let mut deserialized: u64 = 0;
            let bytes_read =
                u64_layout.deserialize(&buffer, &mut deserialized as *mut _ as *mut u8);
            assert_eq!(bytes_read, 8);
            assert_eq!(deserialized, value);
        }

        // Test bool serialization
        let bool_layout = registry.get("bool").unwrap();
        let value = true;
        buffer.clear();

        unsafe {
            let bytes_written = bool_layout.serialize(&value as *const _ as *const u8, &mut buffer);
            assert_eq!(bytes_written, 1);
            assert_eq!(buffer, vec![1]);

            let mut deserialized = false;
            let bytes_read =
                bool_layout.deserialize(&buffer, &mut deserialized as *mut _ as *mut u8);
            assert_eq!(bytes_read, 1);
            assert!(deserialized);
        }

        // Test string serialization
        let string_layout = registry.get("string").unwrap();
        let value = String::from("Hello, World!");
        buffer.clear();

        unsafe {
            let bytes_written =
                string_layout.serialize(&value as *const _ as *const u8, &mut buffer);
            assert_eq!(bytes_written, 260); // Fixed size: 4 bytes length + 256 bytes data
            assert_eq!(buffer.len(), 260);

            let mut deserialized = String::new();
            let bytes_read =
                string_layout.deserialize(&buffer, &mut deserialized as *mut _ as *mut u8);
            assert_eq!(bytes_read, 260);
            assert_eq!(deserialized, value);
        }
    }

    #[test]
    fn test_custom_type_registration() {
        let registry = TypeRegistry::new();

        // Register a custom composite type
        unsafe {
            let registration = TypeRegistration::new(
                "Vector3f".to_string(),
                12, // 3 * f32
                4,  // f32 alignment
                true,
                |src, dst| {
                    let slice = std::slice::from_raw_parts(src, 12);
                    dst.extend_from_slice(slice);
                    12
                },
                |src, dst| {
                    if src.len() >= 12 {
                        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 12);
                        12
                    } else {
                        0
                    }
                },
                Some(TypeId::of::<[f32; 3]>()),
            );
            let result = register_type(&registry, registration);
            assert!(result.is_ok());
        }

        // Verify custom type is registered
        assert!(registry.contains("Vector3f"));
        let layout = registry.get("Vector3f").unwrap();
        assert_eq!(layout.type_id, "Vector3f");
        assert_eq!(layout.size, 12);
        assert_eq!(layout.align, 4);
        assert!(layout.pod);

        // Test 3xf32 helper
        let registry2 = TypeRegistry::new();
        assert!(register_3xf32_type(&registry2).is_ok());
        assert!(registry2.contains("3xf32"));
        let layout = registry2.get("3xf32").unwrap();
        assert_eq!(layout.size, 12);
        assert_eq!(layout.align, 4);
    }

    #[test]
    fn test_validate_type() {
        let registry = TypeRegistry::new();
        assert!(register_builtin_types(&registry).is_ok());

        // Valid validation
        let layout = registry.validate_type("u64", Some(8), Some(8), Some(true));
        assert!(layout.is_ok());

        // Size mismatch
        let layout = registry.validate_type("u64", Some(4), Some(8), Some(true));
        assert!(layout.is_err());

        // Alignment mismatch
        let layout = registry.validate_type("u64", Some(8), Some(4), Some(true));
        assert!(layout.is_err());

        // POD flag mismatch
        let layout = registry.validate_type("u64", Some(8), Some(8), Some(false));
        assert!(layout.is_err());

        // Type not found
        let layout = registry.validate_type("nonexistent", None, None, None);
        assert!(layout.is_err());
    }
}
