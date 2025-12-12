//! TableBuffer - A Vec<u8> storage buffer for table data with zero-copy access.
//!
//! This buffer provides contiguous memory storage with capacity management,
//! designed for cache-efficient access and future unsafe casting to field types.

use std::ops::{Deref, DerefMut};

/// A storage buffer wrapping Vec<u8> for table data.
///
/// Provides zero-copy access to the underlying bytes and capacity management.
/// The buffer is contiguous in memory and designed for cache-efficient access
/// when storing tightly packed records and fields.
///
/// # Safety
/// This buffer enables zero-copy access through raw pointers (`as_ptr()`).
/// When casting pointers to field types, callers must ensure:
/// - Proper alignment for the target type
/// - Only initialized memory is accessed (up to `len()` bytes)
/// - No data races (buffer is `Send + Sync` but mutable access requires exclusivity)
#[derive(Debug)]
pub struct TableBuffer {
    data: Vec<u8>,
}

impl TableBuffer {
    /// Creates a new empty TableBuffer.
    ///
    /// The buffer has no allocated capacity. It will not allocate until
    /// elements are pushed onto it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new TableBuffer with the specified capacity.
    ///
    /// The buffer will have exactly `capacity` bytes allocated but zero length.
    /// The memory is uninitialized.
    ///
    /// # Panics
    /// Panics if the capacity exceeds `isize::MAX` bytes.
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Attempts to create a new TableBuffer with the specified capacity.
    ///
    /// Returns `Ok(TableBuffer)` if allocation succeeds, or `Err` if allocation
    /// fails (e.g., out of memory).
    pub fn try_with_capacity(capacity: usize) -> Result<Self, std::collections::TryReserveError> {
        let mut data = Vec::new();
        data.try_reserve_exact(capacity)?;
        Ok(Self { data })
    }

    /// Creates a new TableBuffer with zeroed memory of the specified capacity.
    ///
    /// The buffer will have exactly `capacity` bytes allocated and initialized to zero.
    pub fn new_zeroed(capacity: usize) -> Self {
        let data = vec![0; capacity];
        Self { data }
    }

    /// Returns the total capacity of the buffer in bytes.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns the current length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the buffer contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a raw pointer to the buffer's data.
    ///
    /// The pointer is valid for reads and writes as long as the buffer is alive.
    /// The memory is contiguous and aligned to at least `align_of::<u8>()`.
    ///
    /// # Safety
    /// When casting this pointer to other types (`*const T`), the caller must ensure:
    /// 1. The target type `T` has no invalid bit patterns (or memory is initialized)
    /// 2. The pointer is properly aligned for `T` (use `read_unaligned` if unsure)
    /// 3. Only initialized bytes are read (up to `len()` bytes)
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    /// Returns a mutable raw pointer to the buffer's data.
    ///
    /// # Safety
    /// The caller must ensure:
    /// 1. The buffer is not accessed concurrently (no other references exist)
    /// 2. When writing through this pointer, only initialized regions are written
    ///    (or newly written memory is properly initialized before being read)
    /// 3. `len()` is updated if additional bytes are initialized
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    /// Returns a slice view of the initialized portion of the buffer.
    ///
    /// This returns only the bytes that have been written to the buffer.
    /// For accessing the entire allocated capacity (including uninitialized bytes),
    /// use `as_ptr()` and appropriate pointer arithmetic.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable slice view of the initialized portion of the buffer.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Reserves capacity for at least `additional` more bytes.
    ///
    /// The buffer may reserve more space to avoid frequent reallocations.
    /// After calling `reserve`, capacity will be greater than or equal to
    /// `self.len() + additional`.
    ///
    /// # Panics
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Reserves capacity for at least `additional` more bytes.
    ///
    /// After calling `reserve_exact`, capacity will be at least
    /// `self.len() + additional`. Does nothing if the capacity is already sufficient.
    /// The actual capacity may be larger due to allocation granularity.
    ///
    /// # Panics
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve_exact(additional);
    }

    /// Shrinks the capacity of the buffer as much as possible.
    ///
    /// It will drop down as close as possible to the length but may still
    /// be larger due to allocation granularity.
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// Clears the buffer, removing all bytes.
    ///
    /// Note that this method has no effect on the allocated capacity.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Appends a slice of bytes to the end of the buffer.
    ///
    /// This increases the length of the buffer by the slice's length.
    pub fn extend_from_slice(&mut self, slice: &[u8]) {
        self.data.extend_from_slice(slice);
    }
}

impl Default for TableBuffer {
    /// Creates an empty TableBuffer with zero capacity.
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl Deref for TableBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for TableBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

// Safety: TableBuffer is Send + Sync because Vec<u8> is Send + Sync
unsafe impl Send for TableBuffer {}
unsafe impl Sync for TableBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deref() {
        let mut buffer = TableBuffer::new_with_capacity(10);
        buffer.extend_from_slice(&[1, 2, 3]);
        
        // Test Deref
        assert_eq!(buffer.len(), 3);
        assert_eq!(&buffer[0..3], &[1, 2, 3]);
        
        // Test DerefMut
        buffer[0] = 42;
        assert_eq!(buffer[0], 42);
    }
}