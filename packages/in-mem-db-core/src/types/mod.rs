//! Type system, layout calculation, and custom type registration.

mod builtin_types;
mod error;
mod type_layout;
mod type_registration;
mod type_registry;

// Re-export public items
pub use builtin_types::{register_3xf32_type, register_builtin_types, register_type};
pub use error::TypeError;
pub use type_layout::TypeLayout;
pub use type_registration::{DeserializerFn, SerializerFn, TypeRegistration};
pub use type_registry::TypeRegistry;

#[cfg(test)]
mod tests {
    use super::*;
    use ntest::timeout;

    #[timeout(1000)]
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
                Some(std::any::TypeId::of::<u64>()),
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
                Some(std::any::TypeId::of::<u64>()),
            )
        };
        assert!(layout.validate().is_err());

        // Invalid: POD type missing TypeId
        let layout =
            unsafe { TypeLayout::new("test".to_string(), 8, 8, true, |_, _| 0, |_, _| 0, None) };
        assert!(layout.validate().is_err());
    }

    #[timeout(1000)]
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
                Some(std::any::TypeId::of::<[u64; 2]>()),
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
                Some(std::any::TypeId::of::<[u64; 2]>()),
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

    #[timeout(1000)]
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

    #[timeout(1000)]
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

    #[timeout(1000)]
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
                Some(std::any::TypeId::of::<[f32; 3]>()),
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

    #[timeout(1000)]
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
