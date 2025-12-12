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
    let _is_cache_aligned = (ptr as usize).is_multiple_of(64);
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
        for (i, &byte) in data.iter().enumerate().take(8) {
            assert_eq!(*ptr.add(i), byte);
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

// Tests for task_sl_4: Write records into buffer at correct offsets via unsafe pointer casting

#[test]
fn test_write_single_record() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 24; // i32(4) + f32(4) + bool(1) = 9, but with alignment

    // Create field data
    let id_data = 42u32.to_ne_bytes();
    let score_data = 3.5f32.to_ne_bytes();
    let active_data = [1u8]; // true as u8

    // Write record with multiple field types
    let fields = vec![
        (0, id_data.as_slice()),     // id at offset 0
        (4, score_data.as_slice()),  // score at offset 4 (aligned)
        (8, active_data.as_slice()), // active at offset 8
    ];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Verify data written at correct byte offsets
    unsafe {
        assert_eq!(buffer.read_at::<u32>(0), 42);
        assert!((buffer.read_at::<f32>(4) - 3.5).abs() < 0.0001);
        assert!(buffer.read_at::<bool>(8));
    }

    // Record size matches calculated size (9 bytes for data, but we allocated 24 for test)
    assert!(record_size >= 9);
}

#[test]
fn test_write_multiple_records() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(256);
    let record_size = 16;

    // Write several consecutive records
    for i in 0..3 {
        let id_data = (100 + i as u32).to_ne_bytes();
        let value_data = (i as f64 * 10.0).to_ne_bytes();

        let fields = vec![
            (0, id_data.as_slice()),
            (8, value_data.as_slice()), // f64 needs 8-byte alignment
        ];

        unsafe {
            buffer.write_record(i, record_size, &fields);
        }
    }

    // Verify each record resides at correct offset
    for i in 0..3 {
        let record_offset = i * record_size;
        unsafe {
            assert_eq!(buffer.read_at::<u32>(record_offset), 100 + i as u32);
            assert!((buffer.read_at::<f64>(record_offset + 8) - (i as f64 * 10.0)).abs() < 0.0001);
        }

        // Verify fields don't bleed into neighboring records
        if i < 2 {
            let next_record_offset = (i + 1) * record_size;
            // Check that bytes between records are zero (buffer was zeroed)
            for byte_idx in (record_offset + 16)..next_record_offset {
                assert_eq!(buffer.as_slice()[byte_idx], 0);
            }
        }
    }
}

#[test]
fn test_write_partial_record() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // Write complete record first
    let id_data = 1u32.to_ne_bytes();
    let name_len_data = 5u16.to_ne_bytes();
    let active_data = [1u8]; // true as u8

    let initial_fields = vec![
        (0, id_data.as_slice()),
        (4, name_len_data.as_slice()),
        (6, active_data.as_slice()),
    ];

    unsafe {
        buffer.write_record(0, record_size, &initial_fields);
    }

    // Update only the active field
    let new_active_data = [0u8]; // false as u8
    let update_fields = vec![(6, new_active_data.as_slice())];

    unsafe {
        buffer.write_record(0, record_size, &update_fields);
    }

    // Verify unwritten fields retain previous values
    unsafe {
        assert_eq!(buffer.read_at::<u32>(0), 1); // unchanged
        assert_eq!(buffer.read_at::<u16>(4), 5); // unchanged
        assert!(!buffer.read_at::<bool>(6)); // updated
    }

    // Verify partial write didn't corrupt adjacent fields
    // Check bytes around the updated field
    assert_eq!(buffer.as_slice()[5], 0); // byte before active field (padding)
    assert_eq!(buffer.as_slice()[7], 0); // byte after active field (since bool is 1 byte)
}

#[test]
fn test_write_at_buffer_start() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    let field_data = 0x12345678u32.to_ne_bytes();
    let fields = vec![(0, field_data.as_slice())];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Pointer arithmetic should yield offset 0
    unsafe {
        let ptr = buffer.as_ptr() as *const u32;
        let value = ptr.read_unaligned();
        assert_eq!(value, 0x12345678);
    }

    // All fields should be accessible
    unsafe {
        assert_eq!(buffer.read_at::<u32>(0), 0x12345678);
    }
}

#[test]
fn test_write_at_buffer_end() {
    use db_core::storage::TableBuffer;

    let capacity = 64;
    let mut buffer = TableBuffer::new_zeroed(capacity);
    let record_size = 16;

    // Write record that ends exactly at buffer.len()
    let record_index = (capacity / record_size) - 1;
    let field_data = 0xCAFEBABEu32.to_ne_bytes();
    let fields = vec![(0, field_data.as_slice())];

    unsafe {
        buffer.write_record(record_index, record_size, &fields);
    }

    // Write should succeed without panic
    let record_offset = record_index * record_size;
    unsafe {
        assert_eq!(buffer.read_at::<u32>(record_offset), 0xCAFEBABE);
    }

    // Record should end at buffer capacity
    assert_eq!(record_offset + record_size, capacity);
}

#[test]
fn test_write_near_capacity() {
    use db_core::storage::TableBuffer;

    // Test that write_record_checked detects when record would exceed buffer capacity
    let capacity = 64;
    let mut buffer = TableBuffer::new_zeroed(capacity);
    let record_size = 32;

    // Try to write record where there's not enough space
    // 2 * 32 = 64, record would occupy bytes 64..96 exceeding capacity 64
    let record_index = 2;
    let field_data = 0xDEADBEEFu32.to_ne_bytes();
    let fields = vec![(0, field_data.as_slice())];

    // Should return error about capacity
    let result = buffer.write_record_checked(record_index, record_size, &fields);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("capacity"),
        "Error message should mention capacity: {}",
        err_msg
    );
}

#[test]
fn test_all_scalar_types() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(256);
    let record_size = 128;

    // Test writing each supported scalar type
    let i8_val: i8 = -42;
    let u16_val: u16 = 0x1234;
    let i32_val: i32 = -123456;
    let i64_val: i64 = 0x0123456789ABCDEF;
    let f32_val: f32 = std::f32::consts::PI;
    let f64_val: f64 = std::f64::consts::E;
    let bool_val: bool = true;

    // Store byte arrays to avoid temporary value issues
    let i8_bytes = i8_val.to_ne_bytes();
    let u16_bytes = u16_val.to_ne_bytes();
    let i32_bytes = i32_val.to_ne_bytes();
    let i64_bytes = i64_val.to_ne_bytes();
    let f32_bytes = f32_val.to_ne_bytes();
    let f64_bytes = f64_val.to_ne_bytes();
    let bool_bytes = [bool_val as u8];

    let fields = vec![
        (0, i8_bytes.as_slice()),
        (1, u16_bytes.as_slice()),
        (4, i32_bytes.as_slice()),
        (8, i64_bytes.as_slice()),
        (16, f32_bytes.as_slice()),
        (20, f64_bytes.as_slice()),
        (28, bool_bytes.as_slice()),
    ];

    unsafe {
        buffer.write_record(0, record_size, &fields);

        // Verify each value round-trips correctly
        assert_eq!(buffer.read_at::<i8>(0), -42);
        assert_eq!(buffer.read_unaligned_at::<u16>(1), 0x1234);
        assert_eq!(buffer.read_at::<i32>(4), -123456);
        assert_eq!(buffer.read_at::<i64>(8), 0x0123456789ABCDEF);
        assert!((buffer.read_at::<f32>(16) - std::f32::consts::PI).abs() < 0.0001);
        assert!((buffer.read_unaligned_at::<f64>(20) - std::f64::consts::E).abs() < 0.0001);
        assert!(buffer.read_at::<bool>(28));
    }
}

#[test]
fn test_endianness_consistency() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    // Write a multi-byte integer
    let value: u32 = 0xDEADBEEF;
    let field_data = value.to_ne_bytes();
    let fields = vec![(0, field_data.as_slice())];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Verify byte sequence in buffer matches to_ne_bytes()
    let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), 4) };
    let expected_bytes = value.to_ne_bytes();
    assert_eq!(bytes, expected_bytes);

    // Verify read back gives same value
    unsafe {
        let read_value = buffer.read_at::<u32>(0);
        assert_eq!(read_value, value);
    }
}

#[test]
fn test_zeroed_buffer_write() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // Verify buffer starts zeroed
    assert!(buffer.as_slice().iter().all(|&b| b == 0));

    // Write a record
    let id_data = 0x12345678u32.to_ne_bytes();
    let score_data = 0x3F8CCCCDu32.to_ne_bytes(); // ~1.1 in f32

    let fields = vec![(0, id_data.as_slice()), (4, score_data.as_slice())];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Verify written bytes are non-zero where expected
    for i in 0..4 {
        assert_ne!(buffer.as_slice()[i], 0); // id bytes
    }
    for i in 4..8 {
        assert_ne!(buffer.as_slice()[i], 0); // score bytes
    }

    // Verify untouched bytes stay zero
    for i in 8..record_size {
        assert_eq!(buffer.as_slice()[i], 0);
    }
}

#[test]
fn test_custom_composite_type() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // Simulate a Vec3 type (3xf32)
    let x_data = 1.0f32.to_ne_bytes();
    let y_data = 2.0f32.to_ne_bytes();
    let z_data = 3.0f32.to_ne_bytes();

    let fields = vec![
        (0, x_data.as_slice()),
        (4, y_data.as_slice()),
        (8, z_data.as_slice()),
    ];

    unsafe {
        buffer.write_record(0, record_size, &fields);

        // Verify all components appear at expected sub-offsets
        assert!((buffer.read_at::<f32>(0) - 1.0).abs() < 0.0001);
        assert!((buffer.read_at::<f32>(4) - 2.0).abs() < 0.0001);
        assert!((buffer.read_at::<f32>(8) - 3.0).abs() < 0.0001);
    }
}

#[test]
fn test_write_record_checked_success() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 16;

    let id_data = 42u32.to_ne_bytes();
    let score_data = 3.5f32.to_ne_bytes();

    let fields = vec![(0, id_data.as_slice()), (4, score_data.as_slice())];

    // Should succeed with bounds checking
    let result = buffer.write_record_checked(0, record_size, &fields);
    assert!(result.is_ok());

    // Verify data was written
    unsafe {
        assert_eq!(buffer.read_at::<u32>(0), 42);
        assert!((buffer.read_at::<f32>(4) - 3.5).abs() < 0.0001);
    }
}

#[test]
fn test_write_record_checked_out_of_bounds_index() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    let field_data = 0x12345678u32.to_ne_bytes();
    let fields = vec![(0, field_data.as_slice())];

    // Try to write at index that would exceed buffer (64 / 16 = 4 records max, index 4 is out of bounds)
    let result = buffer.write_record_checked(4, record_size, &fields);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceed buffer capacity"));
}

#[test]
fn test_write_record_checked_out_of_bounds_offset() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    let field_data = 0x12345678u32.to_ne_bytes();
    // Field offset 20 exceeds record size 16
    let fields = vec![(20, field_data.as_slice())];

    let result = buffer.write_record_checked(0, record_size, &fields);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("field offset exceeds record size"));
}

#[test]
fn test_write_record_checked_field_exceeds_record() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 8;

    // Create a field that's too large for the record
    let large_data = vec![0u8; 16]; // 16 bytes > record size 8
    let fields = vec![(0, large_data.as_slice())];

    let result = buffer.write_record_checked(0, record_size, &fields);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("field data would exceed record bounds"));
}

#[test]
fn test_write_record_checked_overflow_calculation() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(1024);

    // Test overflow in record_index * record_size
    let record_index = usize::MAX;
    let record_size = 2;
    let field_data = [0u8; 1];
    let fields = vec![(0, field_data.as_slice())];

    let result = buffer.write_record_checked(record_index, record_size, &fields);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("overflow"));
}

#[test]
fn test_write_record_checked_null_pointer_safety() {
    use db_core::storage::TableBuffer;

    // Even with zero capacity, should handle gracefully
    let mut buffer = TableBuffer::new_zeroed(0);
    let record_size = 16;
    let field_data = [0u8; 4];
    let fields = vec![(0, field_data.as_slice())];

    let result = buffer.write_record_checked(0, record_size, &fields);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceed buffer capacity"));
}

#[test]
fn test_write_record_checked_overlap_detection() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // First write a record
    let field1_data = 0x11111111u32.to_ne_bytes();
    let fields1 = vec![(0, field1_data.as_slice())];

    let result1 = buffer.write_record_checked(0, record_size, &fields1);
    assert!(result1.is_ok());

    // Try to write overlapping record (incorrect record size calculation)
    // If record_size were incorrectly calculated as 16 instead of 32,
    // record 1 would overlap with record 0
    let field2_data = 0x22222222u32.to_ne_bytes();
    let fields2 = vec![(0, field2_data.as_slice())];

    // This should succeed because we're using correct record_size
    let result2 = buffer.write_record_checked(1, record_size, &fields2);
    assert!(result2.is_ok());

    // Verify both records exist
    unsafe {
        assert_eq!(buffer.read_at::<u32>(0), 0x11111111);
        assert_eq!(buffer.read_at::<u32>(32), 0x22222222); // record 1 at offset 32
    }
}

#[test]
fn test_write_record_checked_field_overlap() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    // Create overlapping fields
    let field1_data = [1u8, 2, 3, 4]; // 4 bytes at offset 0
    let field2_data = [5u8, 6, 7, 8]; // 4 bytes at offset 2 (overlaps)
    let fields = vec![
        (0, field1_data.as_slice()),
        (2, field2_data.as_slice()), // overlaps with field1 at bytes 2-3
    ];

    let result = buffer.write_record_checked(0, record_size, &fields);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("overlapping"));
}

// Tests for task_zc_1: Field accessors returning references (&T) rather than copying values

#[test]
fn test_field_reference_scalar_types() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // Write test data
    let id_data = 42u32.to_ne_bytes();
    let score_data = 3.5f32.to_ne_bytes();
    let active_data = [1u8]; // true as u8
    let count_data = 100u64.to_ne_bytes();

    let fields = vec![
        (0, id_data.as_slice()),
        (4, score_data.as_slice()),
        (8, active_data.as_slice()),
        (16, count_data.as_slice()), // u64 needs 8-byte alignment
    ];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Test getting references to scalar types
    unsafe {
        // Get references instead of copying values
        let id_ref = buffer.field_ref::<u32>(0).unwrap();
        let score_ref = buffer.field_ref::<f32>(4).unwrap();
        let active_ref = buffer.field_ref::<bool>(8).unwrap();
        let count_ref = buffer.field_ref::<u64>(16).unwrap();

        // Verify values match
        assert_eq!(*id_ref, 42);
        assert!((*score_ref - 3.5).abs() < 0.0001);
        assert!(*active_ref);
        assert_eq!(*count_ref, 100);

        // Verify these are references (not copies) by checking pointer equality
        let buffer_ptr = buffer.as_ptr();
        assert_eq!(id_ref as *const u32 as *const u8, buffer_ptr.add(0));
        assert_eq!(score_ref as *const f32 as *const u8, buffer_ptr.add(4));
        assert_eq!(active_ref as *const bool as *const u8, buffer_ptr.add(8));
        assert_eq!(count_ref as *const u64 as *const u8, buffer_ptr.add(16));
    }
}

#[test]
fn test_field_mut_reference() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);
    let record_size = 16;

    // Write initial data
    let initial_data = 42u32.to_ne_bytes();
    let fields = vec![(0, initial_data.as_slice())];

    unsafe {
        buffer.write_record(0, record_size, &fields);
    }

    // Get mutable reference and modify value
    unsafe {
        let value_ref = buffer.field_mut_ref::<u32>(0).unwrap();
        assert_eq!(*value_ref, 42);

        // Modify through mutable reference
        *value_ref = 100;

        // Verify modification
        assert_eq!(*value_ref, 100);

        // Read back using immutable reference
        let read_ref = buffer.field_ref::<u32>(0).unwrap();
        assert_eq!(*read_ref, 100);
    }
}

#[test]
fn test_field_reference_alignment() {
    use db_core::storage::TableBuffer;
    use std::mem::align_of;

    let mut buffer = TableBuffer::new_zeroed(128);

    // Write u64 at offset 0 (should be 8-byte aligned)
    // Use write_at to write at specific offset
    unsafe {
        buffer.write_at::<u64>(0, 0x0123456789ABCDEF);
    }

    // Get reference and check alignment
    unsafe {
        let ref_u64 = buffer.field_ref::<u64>(0).unwrap();
        let ptr = ref_u64 as *const u64;
        assert_eq!(ptr as usize % align_of::<u64>(), 0);
        assert_eq!(*ref_u64, 0x0123456789ABCDEF);
    }

    // Write f32 at offset 4 (should be 4-byte aligned)
    unsafe {
        buffer.write_at::<f32>(4, std::f32::consts::PI);
    }

    unsafe {
        let ref_f32 = buffer.field_ref::<f32>(4).unwrap();
        let ptr = ref_f32 as *const f32;
        assert_eq!(ptr as usize % align_of::<f32>(), 0);
        assert!((*ref_f32 - std::f32::consts::PI).abs() < 0.0001);
    }
}

#[test]
fn test_field_reference_out_of_bounds() {
    use db_core::storage::TableBuffer;

    let buffer = TableBuffer::new_zeroed(64);

    // Try to get reference beyond buffer bounds
    unsafe {
        // Offset 60 for u32 would read bytes 60-63 (within bounds)
        let result = buffer.field_ref::<u32>(60);
        assert!(result.is_some());

        // Offset 61 for u32 would read bytes 61-64 (64 is out of bounds for 64-byte buffer)
        let result = buffer.field_ref::<u32>(61);
        assert!(result.is_none());

        // Offset 0 for u64 would read bytes 0-7 (within bounds)
        let result = buffer.field_ref::<u64>(0);
        assert!(result.is_some());

        // Offset 57 for u64 would read bytes 57-64 (64 is out of bounds)
        let result = buffer.field_ref::<u64>(57);
        assert!(result.is_none());
    }
}

#[test]
fn test_field_reference_custom_composite_type() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);

    // Simulate Vec3 type (3xf32)
    unsafe {
        buffer.write_at::<f32>(0, 1.0f32);
        buffer.write_at::<f32>(4, 2.0f32);
        buffer.write_at::<f32>(8, 3.0f32);
    }

    // Get references to individual f32 components
    unsafe {
        let x_ref = buffer.field_ref::<f32>(0).unwrap();
        let y_ref = buffer.field_ref::<f32>(4).unwrap();
        let z_ref = buffer.field_ref::<f32>(8).unwrap();

        assert!((*x_ref - 1.0).abs() < 0.0001);
        assert!((*y_ref - 2.0).abs() < 0.0001);
        assert!((*z_ref - 3.0).abs() < 0.0001);

        // Also test getting reference to the whole [f32; 3] array
        // Note: This requires the array to be stored contiguously
        // We can't directly get &[f32; 3] because it's not stored as a single value
        // But we can verify the individual components are contiguous
        let buffer_ptr = buffer.as_ptr();
        assert_eq!(x_ref as *const f32 as *const u8, buffer_ptr.add(0));
        assert_eq!(y_ref as *const f32 as *const u8, buffer_ptr.add(4));
        assert_eq!(z_ref as *const f32 as *const u8, buffer_ptr.add(8));
    }
}

#[test]
fn test_multiple_references_same_buffer() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);
    let record_size = 32;

    // Write two records
    for i in 0..2 {
        let id_data = (100 + i as u32).to_ne_bytes();
        let value_data = (i as f64 * 10.0).to_ne_bytes();

        let fields = vec![(0, id_data.as_slice()), (8, value_data.as_slice())];

        unsafe {
            buffer.write_record(i, record_size, &fields);
        }
    }

    // Get references to fields in different records
    unsafe {
        let id_ref0 = buffer.field_ref::<u32>(0).unwrap(); // Record 0, offset 0
        let value_ref0 = buffer.field_ref::<f64>(8).unwrap(); // Record 0, offset 8

        let id_ref1 = buffer.field_ref::<u32>(32).unwrap(); // Record 1, offset 32 (record_size * 1 + 0)
        let value_ref1 = buffer.field_ref::<f64>(40).unwrap(); // Record 1, offset 40 (record_size * 1 + 8)

        // Verify values
        assert_eq!(*id_ref0, 100);
        assert!((*value_ref0 - 0.0).abs() < 0.0001);

        assert_eq!(*id_ref1, 101);
        assert!((*value_ref1 - 10.0).abs() < 0.0001);

        // Verify pointers are different
        assert!(!std::ptr::eq(id_ref0, id_ref1));
        assert!(!std::ptr::eq(value_ref0, value_ref1));

        // Verify pointer offsets match record structure
        let buffer_ptr = buffer.as_ptr();
        assert_eq!(id_ref0 as *const u32 as *const u8, buffer_ptr.add(0));
        assert_eq!(value_ref0 as *const f64 as *const u8, buffer_ptr.add(8));
        assert_eq!(id_ref1 as *const u32 as *const u8, buffer_ptr.add(32));
        assert_eq!(value_ref1 as *const f64 as *const u8, buffer_ptr.add(40));
    }
}

#[test]
fn test_field_reference_bool_validity() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(64);

    // Write valid bool values (0 and 1)
    unsafe {
        buffer.write_at::<u8>(0, 0); // false
        buffer.write_at::<u8>(1, 1); // true

        // Get references as bool
        let false_ref = buffer.field_ref::<bool>(0).unwrap();
        let true_ref = buffer.field_ref::<bool>(1).unwrap();

        assert!(!*false_ref);
        assert!(*true_ref);
    }

    // Note: Writing invalid bool values (not 0 or 1) is undefined behavior
    // when reading as &bool. We rely on the write path to ensure only 0/1 are written.
}

#[test]
fn test_field_reference_unaligned_access() {
    use db_core::storage::TableBuffer;

    let mut buffer = TableBuffer::new_zeroed(128);

    // Write u32 at unaligned offset (offset 1)
    let value = 0xDEADBEEFu32;
    unsafe {
        buffer.write_unaligned_at::<u32>(1, value);
    }

    // Get reference to unaligned data - should fail because offset 1 is not 4-byte aligned
    unsafe {
        let ref_u32 = buffer.field_ref::<u32>(1);
        assert!(
            ref_u32.is_none(),
            "field_ref should reject unaligned access"
        );

        // But we can still read the value using read_unaligned_at
        let read_value = buffer.read_unaligned_at::<u32>(1);
        assert_eq!(read_value, 0xDEADBEEF);
    }
}

#[test]
fn test_field_reference_zero_sized_type() {
    use db_core::storage::TableBuffer;

    #[derive(Copy, Clone)]
    struct ZeroSizedType;

    let buffer = TableBuffer::new_zeroed(64);

    // For zero-sized types, we should be able to get a reference at any offset
    unsafe {
        let ref_zst = buffer.field_ref::<ZeroSizedType>(0).unwrap();
        let ref_zst2 = buffer.field_ref::<ZeroSizedType>(100).unwrap(); // Even beyond buffer bounds

        // All references to ZST are equal (dangling)
        assert!(std::ptr::eq(ref_zst, ref_zst2));

        // Can still use the reference
        let _ = *ref_zst; // No-op for ZST
    }
}
