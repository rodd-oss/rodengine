use std::any::TypeId;

use super::error::TypeError;
use super::type_layout::TypeLayout;
use super::type_registration::TypeRegistration;
use super::type_registry::TypeRegistry;

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
