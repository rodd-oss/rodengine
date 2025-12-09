use serde::{de::DeserializeOwned, Serialize};

/// A component that can be stored in the database.
/// Components must be serializable and have a unique table ID.
pub trait Component: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Unique numeric identifier for this component type.
    const TABLE_ID: u16;

    /// Name of the component table (must match schema).
    const TABLE_NAME: &'static str;

    /// Optional: size of the component in bytes if using zero-copy layout.
    /// Returns None if using serialization.
    fn static_size() -> Option<usize> {
        None
    }

    /// Optional: alignment requirement for zero-copy layout.
    fn alignment() -> usize {
        1
    }
}

/// Marker trait for components that use zero-copy storage (repr(C)).
///
/// # Safety
///
/// This trait is unsafe because the component must have a stable, `repr(C)` layout
/// with no padding between fields. The `static_size()` and `alignment()` methods
/// must return values equal to `std::mem::size_of::<Self>()` and
/// `std::mem::align_of::<Self>()` respectively.
pub unsafe trait ZeroCopyComponent: Component {
    /// Returns the size of the component (must match std::mem::size_of::<Self>()).
    fn static_size() -> usize;

    /// Returns the alignment of the component (must match std::mem::align_of::<Self>()).
    fn alignment() -> usize;
}

// Implement ZeroCopyComponent for types that are repr(C) and have stable layout.
// Users must manually implement this trait for safety.
