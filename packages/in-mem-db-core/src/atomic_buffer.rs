//! Atomic buffer management using ArcSwap for lock-free operations.
//!
//! Provides zero-copy reads and atomic writes via copy-on-write semantics.
//! Supports both in-memory buffers and memory-mapped files.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;

#[cfg(feature = "persist")]
use memmap2::Mmap;

/// Enum representing different types of buffer storage.
#[derive(Debug)]
pub enum BufferStorage {
    /// In-memory vector storage
    Memory(Vec<u8>),
    /// Memory-mapped file storage
    #[cfg(feature = "persist")]
    Mmap(Arc<Mmap>),
}

impl BufferStorage {
    /// Returns a slice of the buffer data.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            BufferStorage::Memory(vec) => vec.as_slice(),
            #[cfg(feature = "persist")]
            BufferStorage::Mmap(mmap) => mmap.as_ref(),
        }
    }

    /// Returns the length of the buffer in bytes.
    pub fn len(&self) -> usize {
        match self {
            BufferStorage::Memory(vec) => vec.len(),
            #[cfg(feature = "persist")]
            BufferStorage::Mmap(mmap) => mmap.len(),
        }
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            BufferStorage::Memory(vec) => vec.is_empty(),
            #[cfg(feature = "persist")]
            BufferStorage::Mmap(mmap) => mmap.is_empty(),
        }
    }

    /// Returns a raw pointer to the buffer data.
    pub fn as_ptr(&self) -> *const u8 {
        match self {
            BufferStorage::Memory(vec) => vec.as_ptr(),
            #[cfg(feature = "persist")]
            BufferStorage::Mmap(mmap) => mmap.as_ptr(),
        }
    }

    /// Returns the capacity of the buffer (for Memory) or length (for Mmap).
    pub fn capacity(&self) -> usize {
        match self {
            BufferStorage::Memory(vec) => vec.capacity(),
            #[cfg(feature = "persist")]
            BufferStorage::Mmap(mmap) => mmap.len(),
        }
    }
}

/// Atomic buffer wrapper providing lock-free read/write operations.
///
/// Uses `ArcSwap<BufferStorage>` for atomic buffer swapping with copy-on-write
/// semantics. Readers hold `Arc<BufferStorage>` references preventing buffer
/// deallocation during read operations.
///
/// # Safety
/// - All pointer casts must verify alignment and bounds
/// - Buffer `Arc` must outlive any derived references
#[derive(Debug)]
pub struct AtomicBuffer {
    /// Atomic reference-counted buffer for lock-free swapping
    inner: ArcSwap<BufferStorage>,
    /// Current capacity of the buffer in bytes
    capacity: AtomicUsize,
    /// Record size in bytes for offset calculations
    record_size: usize,
}

impl AtomicBuffer {
    /// Creates a new atomic buffer with specified initial capacity and record size.
    ///
    /// # Arguments
    /// * `initial_capacity` - Initial buffer capacity in bytes
    /// * `record_size` - Size of each record in bytes
    ///
    /// # Panics
    /// Panics if `initial_capacity` is 0 or if `record_size` is 0.
    pub fn new(initial_capacity: usize, record_size: usize) -> Self {
        assert!(initial_capacity > 0, "initial_capacity must be > 0");
        assert!(record_size > 0, "record_size must be > 0");

        let buffer = BufferStorage::Memory(Vec::with_capacity(initial_capacity));
        Self {
            inner: ArcSwap::new(Arc::new(buffer)),
            capacity: AtomicUsize::new(initial_capacity),
            record_size,
        }
    }

    /// Loads the current buffer for zero-copy read access.
    ///
    /// Returns an `Arc<BufferStorage>` that holds a reference to the immutable buffer.
    /// The buffer cannot be modified while this reference is held.
    ///
    /// # Performance
    /// - O(1) operation
    /// - No allocations in hot path
    pub fn load(&self) -> Arc<BufferStorage> {
        self.inner.load_full()
    }

    /// Loads and clones the current buffer for modification.
    ///
    /// Returns a mutable `Vec<u8>` clone of the current buffer that can be modified.
    /// Changes are not visible until `store()` is called.
    ///
    /// # Performance
    /// - O(n) operation where n is buffer size
    /// - One allocation for the cloned buffer
    pub fn load_full(&self) -> Vec<u8> {
        let current = self.inner.load_full();
        current.as_slice().to_vec()
    }

    /// Atomically swaps the buffer with a new one.
    ///
    /// # Arguments
    /// * `new_buffer` - New buffer to publish
    ///
    /// # Safety
    /// Caller must ensure `new_buffer` is properly initialized and aligned.
    pub fn store(&self, new_buffer: Vec<u8>) {
        let new_capacity = new_buffer.capacity();
        self.inner
            .store(Arc::new(BufferStorage::Memory(new_buffer)));
        self.capacity.store(new_capacity, Ordering::Release);
    }

    /// Atomically swaps the buffer with a memory-mapped file.
    ///
    /// # Arguments
    /// * `mmap` - Memory-mapped file to publish
    ///
    /// # Safety
    /// Caller must ensure `mmap` is properly initialized and aligned.
    #[cfg(feature = "persist")]
    pub fn store_mmap(&self, mmap: Mmap) {
        let capacity = mmap.len();
        self.inner
            .store(Arc::new(BufferStorage::Mmap(Arc::new(mmap))));
        self.capacity.store(capacity, Ordering::Release);
    }

    /// Returns the byte offset for a record at the given index.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// Byte offset within the buffer where the record starts.
    ///
    /// # Panics
    /// Panics if `record_index * record_size` would overflow `usize`.
    pub fn record_offset(&self, record_index: usize) -> usize {
        record_index
            .checked_mul(self.record_size)
            .expect("record_offset calculation overflow")
    }

    /// Grows the buffer capacity, preserving existing data.
    ///
    /// Creates a new buffer with doubled capacity, copies existing data,
    /// and atomically swaps it in.
    ///
    /// # Arguments
    /// * `required_capacity` - Minimum capacity needed
    ///
    /// # Returns
    /// `true` if growth occurred, `false` if current capacity is sufficient.
    pub fn grow(&self, required_capacity: usize) -> bool {
        let current_capacity = self.capacity.load(Ordering::Acquire);

        if required_capacity <= current_capacity {
            return false;
        }

        // Double capacity strategy
        let new_capacity = current_capacity.max(1) * 2;
        let new_capacity = new_capacity.max(required_capacity);

        let current_buffer = self.inner.load_full();
        let mut new_buffer = Vec::with_capacity(new_capacity);
        new_buffer.extend_from_slice(current_buffer.as_slice());

        self.store(new_buffer);
        true
    }

    /// Returns the current buffer capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity.load(Ordering::Acquire)
    }

    /// Returns the record size in bytes.
    pub fn record_size(&self) -> usize {
        self.record_size
    }

    /// Returns the current buffer length in bytes.
    pub fn len(&self) -> usize {
        self.load().len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.load().is_empty()
    }

    /// Returns a raw pointer to the buffer data with the lifetime bound to the Arc.
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The pointer may be null if the buffer is empty
    /// - The caller must ensure proper alignment for the intended use
    pub fn as_ptr(&self) -> (*const u8, Arc<BufferStorage>) {
        let arc = self.load();
        (arc.as_ptr(), arc)
    }

    /// Returns a raw pointer to a specific byte offset in the buffer.
    ///
    /// # Arguments
    /// * `offset` - Byte offset within the buffer
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the byte at the given offset
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The offset must be within buffer bounds (0 <= offset <= buffer.len())
    /// - The caller must ensure proper alignment for the intended use
    pub fn ptr_at_offset(
        &self,
        offset: usize,
    ) -> Result<(*const u8, Arc<BufferStorage>), &'static str> {
        let arc = self.load();
        if offset >= arc.len() {
            return Err("offset out of bounds");
        }
        let ptr = unsafe { arc.as_ptr().add(offset) };
        Ok((ptr, arc))
    }

    /// Returns a raw pointer to a record at the given index.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the start of the record
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The record must be fully within buffer bounds
    /// - The caller must ensure proper alignment for the record type
    pub fn record_ptr(
        &self,
        record_index: usize,
    ) -> Result<(*const u8, Arc<BufferStorage>), &'static str> {
        let offset = self.record_offset(record_index);
        self.ptr_at_offset(offset)
    }

    /// Returns a slice of the buffer as raw bytes with lifetime bound to the Arc.
    ///
    /// # Arguments
    /// * `range` - Byte range within the buffer
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the start of the slice
    /// - Length of the slice in bytes
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The range must be within buffer bounds
    /// - The caller must ensure proper alignment for the intended use
    pub fn slice(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<(*const u8, usize, Arc<BufferStorage>), &'static str> {
        let arc = self.load();
        if range.start > range.end || range.end > arc.len() {
            return Err("range out of bounds");
        }
        let ptr = unsafe { arc.as_ptr().add(range.start) };
        let len = range.end - range.start;
        Ok((ptr, len, arc))
    }

    /// Returns a slice for a record at the given index.
    ///
    /// # Arguments
    /// * `record_index` - Zero-based record index
    ///
    /// # Returns
    /// A tuple containing:
    /// - Raw pointer to the start of the record
    /// - Length of the record (record_size)
    /// - Arc holding the buffer to ensure lifetime validity
    ///
    /// # Safety
    /// - The returned pointer is valid as long as the returned Arc is alive
    /// - The record must be fully within buffer bounds
    /// - The caller must ensure proper alignment for the record type
    pub fn record_slice(
        &self,
        record_index: usize,
    ) -> Result<(*const u8, usize, Arc<BufferStorage>), &'static str> {
        let offset = self.record_offset(record_index);
        let end_offset = offset + self.record_size;
        self.slice(offset..end_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntest::timeout;

    #[test]
    #[timeout(1000)]
    fn test_new_buffer() {
        let buffer = AtomicBuffer::new(1024, 64);
        assert_eq!(buffer.capacity(), 1024);
        assert_eq!(buffer.record_size(), 64);
        assert_eq!(buffer.len(), 0); // Buffer is empty initially
        assert!(buffer.is_empty());
    }

    #[test]
    #[timeout(1000)]
    fn test_record_offset() {
        let buffer = AtomicBuffer::new(1024, 64);
        assert_eq!(buffer.record_offset(0), 0);
        assert_eq!(buffer.record_offset(1), 64);
        assert_eq!(buffer.record_offset(10), 640);
    }

    #[test]
    #[timeout(1000)]
    fn test_load_store() {
        let buffer = AtomicBuffer::new(1024, 64);

        let loaded = buffer.load();
        assert_eq!(loaded.len(), 0); // Empty initially

        let new_data = vec![1u8, 2, 3, 4];
        buffer.store(new_data.clone());

        let loaded_after = buffer.load();
        assert_eq!(loaded_after.as_slice(), &new_data);
        assert_eq!(buffer.capacity(), new_data.capacity());
    }

    #[timeout(1000)]
    #[test]
    fn test_grow() {
        let buffer = AtomicBuffer::new(1024, 64);
        assert_eq!(buffer.capacity(), 1024);

        // No growth needed
        assert!(!buffer.grow(512));
        assert_eq!(buffer.capacity(), 1024);

        // Growth needed
        assert!(buffer.grow(2048));
        assert!(buffer.capacity() >= 2048);

        // Verify data preserved (buffer is empty)
        let loaded = buffer.load();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    #[timeout(1000)]
    #[should_panic(expected = "initial_capacity must be > 0")]
    fn test_new_zero_capacity() {
        AtomicBuffer::new(0, 64);
    }

    #[test]
    #[timeout(1000)]
    #[should_panic(expected = "record_size must be > 0")]
    fn test_new_zero_record_size() {
        AtomicBuffer::new(1024, 0);
    }

    #[timeout(1000)]
    #[test]
    fn test_as_ptr() {
        let buffer = AtomicBuffer::new(1024, 64);
        let (ptr, arc) = buffer.as_ptr();

        // Pointer should not be null (buffer is allocated)
        assert!(!ptr.is_null());

        // Arc should hold the buffer
        assert_eq!(arc.len(), 0); // Buffer is empty initially
    }

    #[timeout(1000)]
    #[test]
    fn test_ptr_at_offset() {
        let buffer = AtomicBuffer::new(1024, 64);

        // Store some data
        let data = vec![1u8, 2, 3, 4, 5];
        buffer.store(data.clone());

        // Get pointer at offset 0
        let (ptr0, _arc0) = buffer.ptr_at_offset(0).unwrap();
        assert!(!ptr0.is_null());
        unsafe {
            assert_eq!(*ptr0, 1);
        }

        // Get pointer at offset 2
        let (ptr2, _arc2) = buffer.ptr_at_offset(2).unwrap();
        unsafe {
            assert_eq!(*ptr2, 3);
        }

        // Test out of bounds
        assert!(buffer.ptr_at_offset(100).is_err());
    }

    #[timeout(1000)]
    #[test]
    fn test_record_ptr() {
        let buffer = AtomicBuffer::new(1024, 64);

        // Store some data
        let mut data = vec![0u8; 128]; // 2 records worth
        data[0] = 1; // First byte of record 0
        data[64] = 2; // First byte of record 1
        buffer.store(data);

        // Get pointer to record 0
        let (ptr0, _arc0) = buffer.record_ptr(0).unwrap();
        unsafe {
            assert_eq!(*ptr0, 1);
        }

        // Get pointer to record 1
        let (ptr1, _arc1) = buffer.record_ptr(1).unwrap();
        unsafe {
            assert_eq!(*ptr1, 2);
        }

        // Test out of bounds
        assert!(buffer.record_ptr(2).is_err()); // Only 2 records (0 and 1)
    }

    #[timeout(1000)]
    #[test]
    fn test_slice() {
        let buffer = AtomicBuffer::new(1024, 64);

        // Store some data
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        buffer.store(data.clone());

        // Get slice 2..6
        let (ptr, len, _arc) = buffer.slice(2..6).unwrap();
        assert_eq!(len, 4);

        // Verify slice contents
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(slice, &[3, 4, 5, 6]);

        // Test invalid ranges
        assert!(buffer.slice(5..5).is_ok()); // empty range is valid
        assert!(buffer.slice(0..100).is_err()); // end out of bounds
    }

    #[timeout(1000)]
    #[test]
    fn test_record_slice() {
        let buffer = AtomicBuffer::new(1024, 64);

        // Store 2 records
        let mut data = vec![0u8; 128];
        data[0] = 1; // First byte of record 0
        data[63] = 2; // Last byte of record 0
        data[64] = 3; // First byte of record 1
        data[127] = 4; // Last byte of record 1
        buffer.store(data);

        // Get slice for record 0
        let (ptr0, len0, _arc0) = buffer.record_slice(0).unwrap();
        assert_eq!(len0, 64);
        let slice0 = unsafe { std::slice::from_raw_parts(ptr0, len0) };
        assert_eq!(slice0[0], 1);
        assert_eq!(slice0[63], 2);

        // Get slice for record 1
        let (ptr1, len1, _arc1) = buffer.record_slice(1).unwrap();
        assert_eq!(len1, 64);
        let slice1 = unsafe { std::slice::from_raw_parts(ptr1, len1) };
        assert_eq!(slice1[0], 3);
        assert_eq!(slice1[63], 4);

        // Test out of bounds
        assert!(buffer.record_slice(2).is_err());
    }
}
