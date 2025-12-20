use std::collections::HashMap;
use std::sync::RwLock;

use super::error::TypeError;
use super::type_layout::TypeLayout;

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
