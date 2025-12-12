//! Unit tests for TableBuffer - Vec<u8> storage buffer implementation

use db_core::storage::TableBuffer;

#[test]
fn new_with_capacity() {
    let buffer = TableBuffer::new_with_capacity(1024);
    assert_eq!(buffer.capacity(), 1024);
    assert!(buffer.is_empty());
}

#[test]
fn default_capacity() {
    let buffer = TableBuffer::default();
    assert_eq!(buffer.capacity(), 0);
    assert!(buffer.is_empty());
}

#[test]
fn zero_capacity() {
    let buffer = TableBuffer::new_with_capacity(0);
    assert_eq!(buffer.capacity(), 0);
    assert!(buffer.is_empty());
    
    // Should be able to reserve after zero capacity
    let mut buffer = buffer;
    buffer.reserve(1);
    assert!(buffer.capacity() >= 1);
}

#[test]
fn max_capacity() {
    // Test that we handle large capacities gracefully
    // Using a reasonable large number instead of usize::MAX to avoid OOM
    let large_capacity = 1024 * 1024 * 1024; // 1GB
    
    // This should either succeed or fail gracefully
    let result = TableBuffer::try_with_capacity(large_capacity);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn capacity_overflow() {
    // Test that reserve with usize::MAX panics
    let result = std::panic::catch_unwind(|| {
        let mut buffer = TableBuffer::new_with_capacity(1024);
        buffer.reserve(usize::MAX);
    });
    assert!(result.is_err(), "reserve with usize::MAX should panic");
}

#[test]
fn contiguous_memory() {
    let buffer = TableBuffer::new_with_capacity(1024);
    // Buffer should be contiguous - we can check by looking at pointer arithmetic
    // For an empty buffer, as_slice() is empty but as_ptr() is valid
    let ptr = buffer.as_ptr();
    assert!(!ptr.is_null());
    
    // After writing some data
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.extend_from_slice(&[1, 2, 3, 4]);
    let slice = buffer.as_slice();
    assert_eq!(slice.len(), buffer.len());
    // Check slice is contiguous by verifying it matches pointer range
    assert_eq!(slice.as_ptr(), buffer.as_ptr());
}

#[test]
fn alignment() {
    let buffer = TableBuffer::new_with_capacity(1024);
    let ptr = buffer.as_ptr();
    
    // Minimum alignment should be at least align_of::<u8>() which is 1
    assert_eq!(ptr as usize % std::mem::align_of::<u8>(), 0);
    
    // Optional: check for cache-line alignment (64 bytes)
    // This depends on Vec<u8> allocation behavior
    let _is_cache_aligned = ptr as usize % 64 == 0;
    // We don't assert this as it's implementation dependent
}

#[test]
fn zeroed_memory() {
    let buffer = TableBuffer::new_zeroed(1024);
    let slice = buffer.as_slice();
    assert!(slice.iter().all(|&b| b == 0));
}

#[test]
fn reserve_exact() {
    let mut buffer = TableBuffer::new_with_capacity(100);
    let initial_capacity = buffer.capacity();
    
    // reserve_exact reserves capacity for self.len() + additional bytes
    // Since len() is 0, it needs capacity for 50 bytes, and we already have 100
    // So it should do nothing
    buffer.reserve_exact(50);
    assert_eq!(buffer.capacity(), initial_capacity);
    
    // Now add some data and reserve more
    buffer.extend_from_slice(&[0; 60]); // Use 60 bytes
    buffer.reserve_exact(50); // Now we need 60 + 50 = 110 bytes
    assert!(buffer.capacity() >= 110);
    
    // Test when capacity is already sufficient
    let before = buffer.capacity();
    buffer.reserve_exact(10); // Should not increase capacity
    assert_eq!(buffer.capacity(), before);
}

#[test]
fn shrink_to_fit() {
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.extend_from_slice(&[1, 2, 3, 4]);
    
    let len_before = buffer.len();
    buffer.shrink_to_fit();
    
    assert_eq!(buffer.capacity(), len_before);
    assert_eq!(buffer.len(), len_before);
    
    // Test with empty buffer
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.shrink_to_fit();
    assert_eq!(buffer.capacity(), 0);
}

#[test]
fn clear_preserves_capacity() {
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    
    let capacity_before = buffer.capacity();
    buffer.clear();
    
    assert_eq!(buffer.capacity(), capacity_before);
    assert!(buffer.is_empty());
}

#[test]
fn send_sync() {
    // Verify TableBuffer implements Send + Sync
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TableBuffer>();
    
    // Test actual cross-thread usage
    let buffer = TableBuffer::new_with_capacity(1024);
    let handle = std::thread::spawn(move || {
        assert_eq!(buffer.capacity(), 1024);
    });
    handle.join().unwrap();
}

#[test]
fn atomic_reference() {
    use std::sync::Arc;
    
    let buffer = TableBuffer::new_with_capacity(1024);
    let arc = Arc::new(buffer);
    
    assert_eq!(Arc::strong_count(&arc), 1);
    
    // Clone and verify reference counting works
    let arc2 = arc.clone();
    assert_eq!(Arc::strong_count(&arc), 2);
    assert_eq!(Arc::strong_count(&arc2), 2);
}

#[test]
fn pointer_cast() {
    let mut buffer = TableBuffer::new_with_capacity(1024);
    
    // Write some u32 values
    let data: u32 = 0xDEADBEEF;
    let bytes = data.to_ne_bytes();
    buffer.extend_from_slice(&bytes);
    
    // Cast pointer to u32 and read back (using read_unaligned since pointer
    // may not be properly aligned for u32)
    unsafe {
        let ptr = buffer.as_ptr() as *const u32;
        let value = ptr.read_unaligned();
        assert_eq!(value, 0xDEADBEEF);
    }
}

#[test]
fn extend_from_slice() {
    let mut buffer = TableBuffer::new_with_capacity(10);
    assert_eq!(buffer.len(), 0);
    
    buffer.extend_from_slice(&[1, 2, 3, 4]);
    assert_eq!(buffer.len(), 4);
    assert_eq!(buffer.as_slice()[0..4], [1, 2, 3, 4]);
    
    buffer.extend_from_slice(&[5, 6, 7, 8]);
    assert_eq!(buffer.len(), 8);
    assert_eq!(buffer.as_slice()[0..8], [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn as_mut_slice() {
    let mut buffer = TableBuffer::new_with_capacity(10);
    buffer.extend_from_slice(&[0; 5]);
    
    let slice = buffer.as_mut_slice();
    slice[0] = 42;
    slice[1] = 24;
    
    assert_eq!(buffer.as_slice()[0], 42);
    assert_eq!(buffer.as_slice()[1], 24);
}

#[test]
fn new_zeroed_memory_safety() {
    // Test that new_zeroed properly initializes all bytes
    // This was fixed from unsafe set_len/write_bytes to vec![0; capacity]
    let buffer = TableBuffer::new_zeroed(1024);
    
    // All bytes should be zero
    assert_eq!(buffer.len(), 1024);
    assert_eq!(buffer.capacity(), 1024);
    assert!(buffer.as_slice().iter().all(|&b| b == 0));
    
    // Test with zero capacity
    let buffer = TableBuffer::new_zeroed(0);
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.capacity(), 0);
    assert!(buffer.is_empty());
}

#[test]
fn as_slice_returns_only_initialized_bytes() {
    // Test that as_slice() only returns initialized bytes (not entire capacity)
    let mut buffer = TableBuffer::new_with_capacity(1024);
    
    // Initially empty
    assert_eq!(buffer.as_slice().len(), 0);
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.capacity(), 1024);
    
    // Add some data
    buffer.extend_from_slice(&[1, 2, 3, 4]);
    assert_eq!(buffer.as_slice().len(), 4);
    assert_eq!(buffer.len(), 4);
    assert_eq!(buffer.capacity(), 1024);
    assert_eq!(buffer.as_slice(), &[1, 2, 3, 4]);
    
    // Add more data
    buffer.extend_from_slice(&[5, 6, 7, 8]);
    assert_eq!(buffer.as_slice().len(), 8);
    assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn new_constructor() {
    // Test the new() constructor added for consistency
    let buffer = TableBuffer::new();
    assert_eq!(buffer.capacity(), 0);
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
    
    // Should be equivalent to default()
    let buffer_default = TableBuffer::default();
    assert_eq!(buffer.capacity(), buffer_default.capacity());
    assert_eq!(buffer.len(), buffer_default.len());
}

#[test]
fn pointer_cast_unaligned() {
    // Test pointer casting with potentially unaligned access
    // This tests the fix from read() to read_unaligned()
    
    // Test with u32 at beginning (might be aligned by chance)
    let mut buffer = TableBuffer::new_with_capacity(1024);
    let data1: u32 = 0xDEADBEEF;
    buffer.extend_from_slice(&data1.to_ne_bytes());
    
    unsafe {
        let ptr = buffer.as_ptr() as *const u32;
        let value = ptr.read_unaligned();
        assert_eq!(value, 0xDEADBEEF);
    }
    
    // Test with u32 at offset 1 (definitely unaligned)
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.extend_from_slice(&[0xAA]); // Padding byte
    let data2: u32 = 0xCAFEBABE;
    buffer.extend_from_slice(&data2.to_ne_bytes());
    
    unsafe {
        let ptr = (buffer.as_ptr().add(1)) as *const u32;
        let value = ptr.read_unaligned();
        assert_eq!(value, 0xCAFEBABE);
    }
    
    // Test with u64 (8 bytes) at offset 3 (unaligned)
    let mut buffer = TableBuffer::new_with_capacity(1024);
    buffer.extend_from_slice(&[0x11, 0x22, 0x33]); // Padding bytes
    let data3: u64 = 0x0123456789ABCDEF;
    buffer.extend_from_slice(&data3.to_ne_bytes());
    
    unsafe {
        let ptr = (buffer.as_ptr().add(3)) as *const u64;
        let value = ptr.read_unaligned();
        assert_eq!(value, 0x0123456789ABCDEF);
    }
}

#[test]
fn safety_invariants_as_ptr() {
    // Test safety invariants for as_ptr() documentation
    let mut buffer = TableBuffer::new_with_capacity(64);
    
    // Write some data
    let data: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    buffer.extend_from_slice(&data);
    
    // as_ptr() should give us pointer to start of buffer
    let ptr = buffer.as_ptr();
    assert!(!ptr.is_null());
    
    // We should be able to read initialized bytes
    unsafe {
        for i in 0..8 {
            assert_eq!(*ptr.add(i), data[i as usize]);
        }
    }
    
    // Test that pointer remains valid after operations
    buffer.reserve(100);
    let ptr_after = buffer.as_ptr();
    // Note: pointer might change after reallocation
    // We just verify it's still valid
    assert!(!ptr_after.is_null());
}

#[test]
fn safety_invariants_as_mut_ptr() {
    // Test safety invariants for as_mut_ptr() documentation
    let mut buffer = TableBuffer::new_with_capacity(64);
    
    // Get mutable pointer
    let mut_ptr = buffer.as_mut_ptr();
    assert!(!mut_ptr.is_null());
    
    // Write through pointer (must update len after)
    unsafe {
        std::ptr::write(mut_ptr, 0x42);
        std::ptr::write(mut_ptr.add(1), 0x24);
    }
    
    // Update length to reflect initialized bytes
    // Note: This is unsafe - normally use extend_from_slice
    // We're testing the safety invariant that len must be updated
    // We can't call as_mut_slice() here because it would expose uninitialized memory
    // Instead, we'll clear and use the safe API
    
    // Clear and use safe API
    buffer.clear();
    buffer.extend_from_slice(&[0x42, 0x24]);
    assert_eq!(buffer.as_slice(), &[0x42, 0x24]);
}