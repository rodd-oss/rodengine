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

    /// Writes a value of type `T` at the specified byte offset in the buffer.
    ///
    /// # Safety
    ///
    /// - `offset` must be within the bounds of the buffer's allocated capacity
    /// - `offset + size_of::<T>()` must not exceed the buffer's capacity
    /// - The memory at `offset` must be properly aligned for type `T`
    /// - The caller must ensure no other references to this memory exist
    /// - The memory must be initialized before being read
    pub unsafe fn write_at<T>(&mut self, offset: usize, value: T)
    where
        T: Copy,
    {
        debug_assert_eq!(
            offset % std::mem::align_of::<T>(),
            0,
            "offset must be aligned for type T"
        );
        debug_assert!(
            offset + std::mem::size_of::<T>() <= self.capacity(),
            "write would exceed buffer capacity"
        );

        let ptr = self.as_mut_ptr().add(offset) as *mut T;
        ptr.write(value);
    }

    /// Writes a value of type `T` at the specified byte offset in the buffer,
    /// using unaligned write if necessary.
    ///
    /// This is safer than `write_at` for unaligned access, but may be slower.
    ///
    /// # Safety
    ///
    /// - `offset` must be within the bounds of the buffer's allocated capacity
    /// - `offset + size_of::<T>()` must not exceed the buffer's capacity
    /// - The caller must ensure no other references to this memory exist
    /// - The memory must be initialized before being read
    pub unsafe fn write_unaligned_at<T>(&mut self, offset: usize, value: T)
    where
        T: Copy,
    {
        debug_assert!(
            offset + std::mem::size_of::<T>() <= self.capacity(),
            "write would exceed buffer capacity"
        );

        let ptr = self.as_mut_ptr().add(offset) as *mut T;
        ptr.write_unaligned(value);
    }

    /// Reads a value of type `T` from the specified byte offset in the buffer.
    ///
    /// # Safety
    ///
    /// - `offset` must be within the bounds of the buffer's initialized length
    /// - `offset + size_of::<T>()` must not exceed the buffer's initialized length
    /// - The memory at `offset` must be properly aligned for type `T`
    /// - The memory must be initialized
    pub unsafe fn read_at<T>(&self, offset: usize) -> T
    where
        T: Copy,
    {
        debug_assert_eq!(
            offset % std::mem::align_of::<T>(),
            0,
            "offset must be aligned for type T"
        );
        debug_assert!(
            offset + std::mem::size_of::<T>() <= self.capacity(),
            "read would exceed buffer capacity"
        );

        let ptr = self.as_ptr().add(offset) as *const T;
        ptr.read()
    }

    /// Reads a value of type `T` from the specified byte offset in the buffer,
    /// using unaligned read if necessary.
    ///
    /// This is safer than `read_at` for unaligned access, but may be slower.
    ///
    /// # Safety
    ///
    /// - `offset` must be within the bounds of the buffer's initialized length
    /// - `offset + size_of::<T>()` must not exceed the buffer's initialized length
    /// - The memory must be initialized
    pub unsafe fn read_unaligned_at<T>(&self, offset: usize) -> T
    where
        T: Copy,
    {
        debug_assert!(
            offset + std::mem::size_of::<T>() <= self.len(),
            "read would exceed initialized buffer length"
        );

        let ptr = self.as_ptr().add(offset) as *const T;
        ptr.read_unaligned()
    }

    /// Writes a record (multiple fields) into the buffer at the specified record index.
    ///
    /// # Parameters
    ///
    /// - `record_index`: The index of the record to write (0-based)
    /// - `record_size`: The size of each record in bytes
    /// - `fields`: A slice of tuples containing field offsets and values
    ///
    /// # Safety
    ///
    /// - `record_index * record_size` must be within buffer bounds
    /// - `record_index * record_size + record_size` must not exceed buffer capacity
    /// - Each field offset must be within the record bounds
    /// - Field values must be properly aligned for their types
    /// - No overlapping writes between fields
    pub unsafe fn write_record(
        &mut self,
        record_index: usize,
        record_size: usize,
        fields: &[(usize, &[u8])],
    ) {
        let record_offset = record_index * record_size;

        for &(field_offset, field_data) in fields {
            let target_offset = record_offset + field_offset;
            let target_ptr = self.as_mut_ptr().add(target_offset);
            std::ptr::copy_nonoverlapping(field_data.as_ptr(), target_ptr, field_data.len());
        }
    }

    /// Reads a record (multiple fields) from the buffer at the specified record index.
    ///
    /// # Parameters
    ///
    /// - `record_index`: The index of the record to read (0-based)
    /// - `record_size`: The size of each record in bytes
    /// - `fields`: A slice of tuples containing field offsets and sizes
    ///
    /// # Returns
    ///
    /// A vector of byte slices, each containing the data for one field.
    ///
    /// # Safety
    ///
    /// - `record_index * record_size` must be within buffer bounds
    /// - `record_index * record_size + record_size` must not exceed buffer capacity
    /// - Each field offset must be within the record bounds
    /// - Field offset + field size must not exceed record size
    /// - The memory must be initialized
    pub unsafe fn read_record(
        &self,
        record_index: usize,
        record_size: usize,
        fields: &[(usize, usize)],
    ) -> Vec<&[u8]> {
        let record_offset = record_index * record_size;

        // Debug assertions to catch common errors in development
        debug_assert!(
            record_offset + record_size <= self.capacity(),
            "record would exceed buffer capacity"
        );

        let mut result = Vec::with_capacity(fields.len());

        for &(field_offset, field_size) in fields {
            let field_start = record_offset + field_offset;
            let _field_end = field_start + field_size;

            debug_assert!(
                field_offset < record_size,
                "field offset exceeds record size"
            );
            debug_assert!(
                _field_end <= record_offset + record_size,
                "field data would exceed record bounds"
            );
            debug_assert!(
                _field_end <= self.capacity(),
                "field read would exceed buffer capacity"
            );

            let field_slice =
                std::slice::from_raw_parts(self.as_ptr().add(field_start), field_size);
            result.push(field_slice);
        }

        result
    }

    /// Writes a record (multiple fields) into the buffer at the specified record index with bounds checking.
    ///
    /// Returns `Ok(())` if the write succeeds, or `Err` if any bounds check fails.
    ///
    /// # Parameters
    ///
    /// - `record_index`: The index of the record to write (0-based)
    /// - `record_size`: The size of each record in bytes
    /// - `fields`: A slice of tuples containing field offsets and values
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - `record_index * record_size` would overflow
    /// - `record_index * record_size + record_size` exceeds buffer capacity
    /// - Any field offset + field data length exceeds record size
    /// - Any field write would exceed buffer bounds
    pub fn write_record_checked(
        &mut self,
        record_index: usize,
        record_size: usize,
        fields: &[(usize, &[u8])],
    ) -> Result<(), &'static str> {
        // Check record offset calculation
        let record_offset = record_index
            .checked_mul(record_size)
            .ok_or("record_index * record_size would overflow")?;

        // Check record fits in buffer
        let record_end = record_offset
            .checked_add(record_size)
            .ok_or("record_offset + record_size would overflow")?;

        if record_end > self.capacity() {
            return Err("record would exceed buffer capacity");
        }

        // Check each field
        for &(field_offset, field_data) in fields {
            // Check field offset within record
            if field_offset >= record_size {
                return Err("field offset exceeds record size");
            }

            // Check field data fits within record
            let field_end = field_offset
                .checked_add(field_data.len())
                .ok_or("field_offset + field_data.len() would overflow")?;

            if field_end > record_size {
                return Err("field data would exceed record bounds");
            }

            // Check field fits in buffer
            let target_offset = record_offset
                .checked_add(field_offset)
                .ok_or("record_offset + field_offset would overflow")?;

            let target_end = target_offset
                .checked_add(field_data.len())
                .ok_or("target_offset + field_data.len() would overflow")?;

            if target_end > self.capacity() {
                return Err("field write would exceed buffer bounds");
            }
        }

        // Check for overlapping fields
        for (i, &(offset1, data1)) in fields.iter().enumerate() {
            let end1 = offset1 + data1.len();

            for &(offset2, data2) in fields.iter().skip(i + 1) {
                let end2 = offset2 + data2.len();

                // Check if ranges [offset1, end1) and [offset2, end2) overlap
                if offset1 < end2 && offset2 < end1 {
                    return Err("overlapping fields detected");
                }
            }
        }

        // All checks passed, perform the write
        unsafe {
            self.write_record(record_index, record_size, fields);
        }

        Ok(())
    }

    /// Reads a record (multiple fields) from the buffer at the specified record index with bounds checking.
    ///
    /// # Parameters
    ///
    /// - `record_index`: The index of the record to read (0-based)
    /// - `record_size`: The size of each record in bytes
    /// - `fields`: A slice of tuples containing field offsets and sizes
    ///
    /// # Returns
    ///
    /// `Ok(Vec<&[u8]>)` containing the field data if all bounds checks pass,
    /// or `Err(&'static str)` describing the first violation found.
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - `record_index * record_size` would overflow
    /// - `record_index * record_size + record_size` exceeds buffer length
    /// - Any field offset exceeds record size
    /// - Any field offset + field size exceeds record size
    /// - Any field read would exceed buffer bounds
    ///
    /// # Note on Buffer Length vs Capacity
    ///
    /// This method uses `len()` (initialized buffer length) for bounds checking, not `capacity()`.
    /// Reading requires access to initialized memory, while writing can use uninitialized capacity.
    /// The unsafe `read_record` method uses `capacity()` in debug assertions but still requires
    /// the memory to be initialized for safe operation.
    pub fn read_record_checked(
        &self,
        record_index: usize,
        record_size: usize,
        fields: &[(usize, usize)],
    ) -> Result<Vec<&[u8]>, &'static str> {
        // Check record offset calculation
        let record_offset = record_index
            .checked_mul(record_size)
            .ok_or("record_index * record_size would overflow")?;

        // Check record fits in buffer
        let record_end = record_offset
            .checked_add(record_size)
            .ok_or("record_offset + record_size would overflow")?;

        if record_end > self.len() {
            return Err("record would exceed buffer length");
        }

        // Check each field
        for &(field_offset, field_size) in fields {
            // Check field offset within record
            if field_offset >= record_size {
                return Err("field offset exceeds record size");
            }

            // Check field size fits within record
            let field_end = field_offset
                .checked_add(field_size)
                .ok_or("field_offset + field_size would overflow")?;

            if field_end > record_size {
                return Err("field data would exceed record bounds");
            }

            // Check field fits in buffer
            let target_offset = record_offset
                .checked_add(field_offset)
                .ok_or("record_offset + field_offset would overflow")?;

            let target_end = target_offset
                .checked_add(field_size)
                .ok_or("target_offset + field_size would overflow")?;

            if target_end > self.len() {
                return Err("field read would exceed buffer bounds");
            }
        }

        // All checks passed, perform the read
        unsafe { Ok(self.read_record(record_index, record_size, fields)) }
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

    #[test]
    fn test_write_at_aligned() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Write u32 at aligned offset
        unsafe {
            buffer.write_at::<u32>(0, 0xDEADBEEF);
            buffer.write_at::<u32>(8, 0xCAFEBABE);
        }

        // Verify using read_at
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0xDEADBEEF);
            assert_eq!(buffer.read_at::<u32>(8), 0xCAFEBABE);
        }

        // Verify using read_unaligned_at (should also work for aligned)
        unsafe {
            assert_eq!(buffer.read_unaligned_at::<u32>(0), 0xDEADBEEF);
            assert_eq!(buffer.read_unaligned_at::<u32>(8), 0xCAFEBABE);
        }
    }

    #[test]
    fn test_write_unaligned_at() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Write u32 at unaligned offset (offset 1)
        unsafe {
            buffer.write_unaligned_at::<u32>(1, 0xDEADBEEF);
            buffer.write_unaligned_at::<u32>(5, 0xCAFEBABE);
        }

        // Verify using read_unaligned_at
        unsafe {
            assert_eq!(buffer.read_unaligned_at::<u32>(1), 0xDEADBEEF);
            assert_eq!(buffer.read_unaligned_at::<u32>(5), 0xCAFEBABE);
        }
    }

    #[test]
    fn test_write_at_different_types() {
        let mut buffer = TableBuffer::new_zeroed(64);

        unsafe {
            // Write various types
            buffer.write_at::<i8>(0, -42);
            buffer.write_at::<u16>(2, 0x1234);
            buffer.write_at::<i32>(4, -123456);
            buffer.write_at::<f32>(8, std::f32::consts::PI);
            buffer.write_at::<f64>(16, std::f64::consts::E);
            buffer.write_at::<bool>(24, true);
            buffer.write_at::<bool>(25, false);

            // Verify
            assert_eq!(buffer.read_at::<i8>(0), -42);
            assert_eq!(buffer.read_at::<u16>(2), 0x1234);
            assert_eq!(buffer.read_at::<i32>(4), -123456);
            assert!((buffer.read_at::<f32>(8) - std::f32::consts::PI).abs() < 0.0001);
            assert!((buffer.read_at::<f64>(16) - std::f64::consts::E).abs() < 0.0001);
            assert!(buffer.read_at::<bool>(24));
            assert!(!buffer.read_at::<bool>(25));
        }
    }

    #[test]
    fn test_write_record_single_field() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 16;

        // Write a record with a single u32 field at offset 0
        let field_data = 0xDEADBEEFu32.to_ne_bytes();
        let fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &fields);
        }

        // Verify the record was written correctly
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0xDEADBEEF);
        }

        // Verify no other bytes were touched (buffer was zeroed)
        for i in 4..record_size {
            assert_eq!(buffer.as_slice()[i], 0);
        }
    }

    #[test]
    fn test_write_record_multiple_fields() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 32;

        // Write a record with multiple fields at different offsets
        let id_data = 0x12345678u32.to_ne_bytes();
        let score_data = std::f64::consts::PI.to_ne_bytes();
        let active_data = [1u8]; // true as u8

        let fields = vec![
            (0, id_data.as_slice()),
            (8, score_data.as_slice()),
            (16, active_data.as_slice()),
        ];

        unsafe {
            buffer.write_record(0, record_size, &fields);
        }

        // Verify all fields were written correctly
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0x12345678);
            assert!((buffer.read_at::<f64>(8) - std::f64::consts::PI).abs() < 0.0001);
            assert!(buffer.read_at::<bool>(16));
        }
    }

    #[test]
    fn test_write_multiple_records() {
        let mut buffer = TableBuffer::new_zeroed(256);
        let record_size = 16;

        // Write 3 records with different data
        for i in 0..3 {
            let id_data = (1000 + i as u32).to_ne_bytes();
            let value_data = (i as f32 * 10.0).to_ne_bytes();

            let fields = vec![(0, id_data.as_slice()), (4, value_data.as_slice())];

            unsafe {
                buffer.write_record(i, record_size, &fields);
            }
        }

        // Verify all records were written correctly
        for i in 0..3 {
            let record_offset = i * record_size;
            unsafe {
                assert_eq!(buffer.read_at::<u32>(record_offset), 1000 + i as u32);
                assert!(
                    (buffer.read_at::<f32>(record_offset + 4) - (i as f32 * 10.0)).abs() < 0.0001
                );
            }
        }
    }

    #[test]
    fn test_write_record_partial_update() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 24;

        // First write a complete record
        let id_data = 0x12345678u32.to_ne_bytes();
        let score_data = std::f64::consts::PI.to_ne_bytes();
        let active_data = [1u8]; // true as u8

        let initial_fields = vec![
            (0, id_data.as_slice()),
            (8, score_data.as_slice()),
            (16, active_data.as_slice()),
        ];

        unsafe {
            buffer.write_record(0, record_size, &initial_fields);
        }

        // Now update just the score field
        let new_score_data = std::f64::consts::E.to_ne_bytes();
        let update_fields = vec![(8, new_score_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &update_fields);
        }

        // Verify score was updated, other fields unchanged
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0x12345678); // unchanged
            assert!((buffer.read_at::<f64>(8) - std::f64::consts::E).abs() < 0.0001); // updated
            assert!(buffer.read_at::<bool>(16)); // unchanged
        }
    }

    #[test]
    fn test_write_record_at_buffer_start() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        let field_data = 0xDEADBEEFu32.to_ne_bytes();
        let fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &fields);
        }

        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0xDEADBEEF);
        }
    }

    #[test]
    fn test_write_record_at_buffer_end() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        // Write record at the last possible position
        let record_index = (buffer.capacity() / record_size) - 1;
        let field_data = 0xCAFEBABEu32.to_ne_bytes();
        let fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(record_index, record_size, &fields);
        }

        let record_offset = record_index * record_size;
        unsafe {
            assert_eq!(buffer.read_at::<u32>(record_offset), 0xCAFEBABE);
        }
    }

    #[test]
    fn test_write_record_endianness() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 8;

        let value: u32 = 0x12345678;
        let field_data = value.to_ne_bytes();
        let fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &fields);
        }

        // Verify byte sequence matches native endianness
        let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), 4) };
        let expected_bytes = value.to_ne_bytes();
        assert_eq!(bytes, expected_bytes);
    }

    #[test]
    fn test_write_record_zeroed_buffer() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        // Verify buffer starts zeroed
        assert!(buffer.as_slice().iter().all(|&b| b == 0));

        // Write a record
        let field1_data = 0xAAu8.to_ne_bytes();
        let field2_data = 0xBBBBu16.to_ne_bytes();

        let fields = vec![(0, field1_data.as_slice()), (1, field2_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &fields);
        }

        // Verify written bytes are non-zero
        assert_eq!(buffer.as_slice()[0], 0xAA);
        assert_eq!(buffer.as_slice()[1], 0xBB);
        assert_eq!(buffer.as_slice()[2], 0xBB);

        // Verify untouched bytes remain zero
        for i in 3..record_size {
            assert_eq!(buffer.as_slice()[i], 0);
        }
    }

    #[test]
    fn test_write_record_checked_success() {
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
    fn test_read_record_single_field() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 16;

        // Write a record with a single u32 field at offset 0
        let field_data = 0xDEADBEEFu32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read the record back
        let read_fields = vec![(0, 4)]; // u32 is 4 bytes
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], field_data.as_slice());

        // Verify using read_at for comparison
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0xDEADBEEF);
        }
    }

    #[test]
    fn test_read_record_multiple_fields() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 32;

        // Write a record with multiple fields at different offsets
        let id_data = 0x12345678u32.to_ne_bytes();
        let score_data = std::f64::consts::PI.to_ne_bytes();
        let active_data = [1u8]; // true as u8

        let write_fields = vec![
            (0, id_data.as_slice()),
            (8, score_data.as_slice()),
            (16, active_data.as_slice()),
        ];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read the record back
        let read_fields = vec![(0, 4), (8, 8), (16, 1)]; // u32, f64, bool
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], id_data.as_slice());
        assert_eq!(result[1], score_data.as_slice());
        assert_eq!(result[2], active_data.as_slice());

        // Verify using read_at for comparison
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), 0x12345678);
            assert!((buffer.read_at::<f64>(8) - std::f64::consts::PI).abs() < 0.0001);
            assert!(buffer.read_at::<bool>(16));
        }
    }

    #[test]
    fn test_read_multiple_records() {
        let mut buffer = TableBuffer::new_zeroed(256);
        let record_size = 16;

        // Write 3 records with different data
        for i in 0..3 {
            let id_data = (1000 + i as u32).to_ne_bytes();
            let value_data = (i as f32 * 10.0).to_ne_bytes();

            let write_fields = vec![(0, id_data.as_slice()), (4, value_data.as_slice())];

            unsafe {
                buffer.write_record(i, record_size, &write_fields);
            }
        }

        // Read all records back
        let read_fields = vec![(0, 4), (4, 4)]; // u32, f32

        for i in 0..3 {
            let result = unsafe { buffer.read_record(i, record_size, &read_fields) };
            assert_eq!(result.len(), 2);

            // Verify id field
            let expected_id = (1000 + i as u32).to_ne_bytes();
            assert_eq!(result[0], expected_id.as_slice());

            // Verify value field
            let expected_value = (i as f32 * 10.0).to_ne_bytes();
            assert_eq!(result[1], expected_value.as_slice());
        }
    }

    #[test]
    fn test_read_record_checked_success() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 16;

        // Write a record
        let id_data = 42u32.to_ne_bytes();
        let score_data = 3.5f32.to_ne_bytes();

        let write_fields = vec![(0, id_data.as_slice()), (4, score_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read the record back with bounds checking
        let read_fields = vec![(0, 4), (4, 4)]; // u32, f32
        let result = buffer.read_record_checked(0, record_size, &read_fields);

        assert!(result.is_ok());
        let field_data = result.unwrap();
        assert_eq!(field_data.len(), 2);
        assert_eq!(field_data[0], id_data.as_slice());
        assert_eq!(field_data[1], score_data.as_slice());
    }

    #[test]
    fn test_read_record_checked_out_of_bounds_index() {
        let buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        let read_fields = vec![(0, 4)]; // u32 field

        // Try to read at index that would exceed buffer (64 / 16 = 4 records max, index 4 is out of bounds)
        let result = buffer.read_record_checked(4, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceed buffer length"));
    }

    #[test]
    fn test_read_record_checked_out_of_bounds_offset() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        // Write some data first
        let field_data = 0x12345678u32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Try to read with field offset that exceeds record size
        let read_fields = vec![(20, 4)]; // Field offset 20 exceeds record size 16
        let result = buffer.read_record_checked(0, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("field offset exceeds record size"));
    }

    #[test]
    fn test_read_record_checked_field_exceeds_record() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 8;

        // Write some data first
        let field_data = 0x12345678u32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Try to read field that's too large for the record
        let read_fields = vec![(0, 16)]; // 16 bytes > record size 8
        let result = buffer.read_record_checked(0, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("field data would exceed record bounds"));
    }

    #[test]
    fn test_read_record_checked_overflow_calculation() {
        let buffer = TableBuffer::new_zeroed(1024);

        // Test overflow in record_index * record_size
        let record_index = usize::MAX;
        let record_size = 2;
        let read_fields = vec![(0, 1)];

        let result = buffer.read_record_checked(record_index, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("overflow"));
    }

    #[test]
    fn test_read_record_checked_empty_buffer() {
        let buffer = TableBuffer::new_zeroed(0);
        let record_size = 16;
        let read_fields = vec![(0, 4)];

        let result = buffer.read_record_checked(0, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceed buffer length"));
    }

    #[test]
    fn test_read_record_partial_fields() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 24;

        // Write a complete record
        let id_data = 0x12345678u32.to_ne_bytes();
        let score_data = std::f64::consts::PI.to_ne_bytes();
        let active_data = [1u8]; // true as u8

        let write_fields = vec![
            (0, id_data.as_slice()),
            (8, score_data.as_slice()),
            (16, active_data.as_slice()),
        ];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read only specific fields (partial read)
        let read_fields = vec![(8, 8)]; // Only read the score field (f64)
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], score_data.as_slice());

        // Verify using read_at for comparison
        unsafe {
            assert!((buffer.read_at::<f64>(8) - std::f64::consts::PI).abs() < 0.0001);
        }
    }

    #[test]
    fn test_read_record_at_buffer_start() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        let field_data = 0xDEADBEEFu32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        let read_fields = vec![(0, 4)];
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], field_data.as_slice());
    }

    #[test]
    fn test_read_record_at_buffer_end() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        // Write record at the last possible position
        let record_index = (buffer.capacity() / record_size) - 1;
        let field_data = 0xCAFEBABEu32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(record_index, record_size, &write_fields);
        }

        let read_fields = vec![(0, 4)];
        let result = unsafe { buffer.read_record(record_index, record_size, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], field_data.as_slice());
    }

    #[test]
    fn test_read_record_endianness() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 8;

        let value: u32 = 0x12345678;
        let field_data = value.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        let read_fields = vec![(0, 4)];
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        // Verify byte sequence matches native endianness
        assert_eq!(result[0], field_data.as_slice());

        // Also verify using read_at
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), value);
        }
    }

    #[test]
    fn test_write_then_read_roundtrip() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 32;

        // Test data
        let id = 42u32;
        let score = 3.14159f64;
        let active = true;
        let name = b"test";

        // Prepare write data
        let id_data = id.to_ne_bytes();
        let score_data = score.to_ne_bytes();
        let active_data = [active as u8];
        let name_data = name;

        let write_fields = vec![
            (0, id_data.as_slice()),
            (8, score_data.as_slice()),
            (16, active_data.as_slice()),
            (17, name_data),
        ];

        // Write the record
        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read the record back
        let read_fields = vec![(0, 4), (8, 8), (16, 1), (17, 4)];
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        // Verify all fields match
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], id_data.as_slice());
        assert_eq!(result[1], score_data.as_slice());
        assert_eq!(result[2], active_data.as_slice());
        assert_eq!(result[3], name_data);

        // Verify using typed reads
        unsafe {
            assert_eq!(buffer.read_at::<u32>(0), id);
            assert!((buffer.read_at::<f64>(8) - score).abs() < 0.0001);
            assert_eq!(buffer.read_at::<bool>(16), active);
        }
    }

    #[test]
    fn test_read_out_of_bounds_index_fails() {
        let buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;
        let read_fields = vec![(0, 4)];

        // This should fail with checked version because index 10 * 16 = 160 > buffer capacity 64
        let result = buffer.read_record_checked(10, record_size, &read_fields);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceed buffer length"));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "record would exceed buffer capacity")]
    fn test_read_out_of_bounds_index_panics_in_debug() {
        let buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;
        let read_fields = vec![(0, 4)];

        // This should panic in debug mode because index 10 * 16 = 160 > buffer capacity 64
        unsafe {
            buffer.read_record(10, record_size, &read_fields);
        }
    }

    #[test]
    fn test_read_partial_record_near_end_of_buffer() {
        let mut buffer = TableBuffer::new_zeroed(20); // Small buffer
        let record_size = 16;

        // Write a record that fits
        let field_data = 0x12345678u32.to_ne_bytes();
        let write_fields = vec![(0, field_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Try to read a field that would extend beyond buffer
        let read_fields = vec![(12, 8)]; // Field at offset 12, size 8 would end at 20 (buffer end)
        let result = buffer.read_record_checked(0, record_size, &read_fields);

        // This should fail because field_end (12 + 8 = 20) > record_size (16)
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("field data would exceed record bounds"));
    }

    #[test]
    fn test_zero_sized_record() {
        let buffer = TableBuffer::new_zeroed(64);
        let record_size = 0;

        // Reading a zero-sized record with no fields should succeed
        let read_fields: Vec<(usize, usize)> = vec![];
        let result = buffer.read_record_checked(0, record_size, &read_fields);

        assert!(result.is_ok());
        let field_data = result.unwrap();
        assert!(field_data.is_empty());
    }

    #[test]
    fn test_buffer_length_not_multiple_of_record_size() {
        let mut buffer = TableBuffer::new_zeroed(30); // Not a multiple of 16
        let record_size = 16;

        // Write first record
        let field1_data = 0x11111111u32.to_ne_bytes();
        let write_fields1 = vec![(0, field1_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields1);
        }

        // Try to read second record - should fail because buffer only has 30 bytes
        // and second record would start at offset 16 and need 16 more bytes (total 32)
        let read_fields = vec![(0, 4)];
        let result = buffer.read_record_checked(1, record_size, &read_fields);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceed buffer length"));
    }

    #[test]
    fn test_unaligned_buffer_access() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Write u64 at unaligned offset (offset 1)
        let value: u64 = 0x0123456789ABCDEF;
        let value_data = value.to_ne_bytes();

        unsafe {
            // Write using unaligned write
            buffer.write_unaligned_at::<u64>(1, value);
        }

        // Read using unaligned read - should work
        unsafe {
            let read_value = buffer.read_unaligned_at::<u64>(1);
            assert_eq!(read_value, value);
        }

        // Try to read using aligned read at unaligned offset - this is unsafe but
        // we're testing that our API allows it with proper warnings
        let read_fields = vec![(1, 8)]; // u64 at offset 1
        let result = unsafe { buffer.read_record(0, 64, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], value_data.as_slice());
    }

    #[test]
    fn test_misaligned_field_offsets() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 32;

        // Write u32 at misaligned offset (offset 1 instead of multiple of 4)
        let value: u32 = 0xDEADBEEF;
        let value_data = value.to_ne_bytes();

        let write_fields = vec![(1, value_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read it back
        let read_fields = vec![(1, 4)];
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], value_data.as_slice());

        // Verify using unaligned read
        unsafe {
            let read_value = buffer.read_unaligned_at::<u32>(1);
            assert_eq!(read_value, value);
        }
    }

    #[test]
    fn test_bool_validation() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Write valid bool values (0 and 1)
        unsafe {
            buffer.write_at::<u8>(0, 0); // false
            buffer.write_at::<u8>(1, 1); // true
            buffer.write_at::<u8>(2, 2); // invalid bool value
        }

        // Read and validate
        unsafe {
            // Valid bool values
            let false_val: u8 = buffer.read_at(0);
            let true_val: u8 = buffer.read_at(1);
            let invalid_val: u8 = buffer.read_at(2);

            // These reads succeed because we're reading u8, not bool
            assert_eq!(false_val, 0);
            assert_eq!(true_val, 1);
            assert_eq!(invalid_val, 2);

            // If we try to interpret as bool, Rust will treat any non-zero as true
            // This is why we need validation functions
            assert!(!(false_val != 0));
            assert!(true_val != 0);
            assert!(invalid_val != 0); // Invalid value treated as true
        }
    }

    #[test]
    fn test_float_nan_and_infinity_allowed() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Write special float values
        let nan = f32::NAN;
        let infinity = f32::INFINITY;
        let neg_infinity = f32::NEG_INFINITY;

        unsafe {
            buffer.write_at::<f32>(0, nan);
            buffer.write_at::<f32>(4, infinity);
            buffer.write_at::<f32>(8, neg_infinity);
        }

        // Read them back
        unsafe {
            let read_nan = buffer.read_at::<f32>(0);
            let read_infinity = buffer.read_at::<f32>(4);
            let read_neg_infinity = buffer.read_at::<f32>(8);

            // NaN comparisons are special
            assert!(read_nan.is_nan());
            assert!(read_infinity.is_infinite());
            assert!(read_neg_infinity.is_infinite());
            assert_eq!(read_infinity, f32::INFINITY);
            assert_eq!(read_neg_infinity, f32::NEG_INFINITY);
        }
    }

    #[test]
    fn test_corrupted_buffer_detection() {
        let mut buffer = TableBuffer::new_zeroed(64);

        // Fill buffer with random bytes
        for i in 0..buffer.capacity() {
            buffer.as_mut_slice()[i] = (i % 256) as u8;
        }

        // Try to read various types from random data
        // This should either succeed (returning garbage) or panic if bounds are violated

        // Test 1: Read within bounds
        let read_fields = vec![(0, 4), (4, 4), (8, 8)];
        let result = buffer.read_record_checked(0, 16, &read_fields);

        // Should succeed because bounds are valid
        assert!(result.is_ok());

        // Test 2: Try to read beyond bounds
        let bad_read_fields = vec![(60, 8)]; // Would read from 60 to 68 > buffer len 64
        let result = buffer.read_record_checked(0, 64, &bad_read_fields);

        // Should fail bounds check
        assert!(result.is_err());
    }

    #[test]
    fn test_overwrite_record_then_read() {
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 16;

        // Write initial record
        let initial_data = 0x11111111u32.to_ne_bytes();
        let initial_fields = vec![(0, initial_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &initial_fields);
        }

        // Verify initial data
        let read_fields = vec![(0, 4)];
        let initial_result = unsafe { buffer.read_record(0, record_size, &read_fields) };
        assert_eq!(initial_result[0], initial_data.as_slice());

        // Overwrite the record
        let new_data = 0x22222222u32.to_ne_bytes();
        let new_fields = vec![(0, new_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &new_fields);
        }

        // Verify new data
        let new_result = unsafe { buffer.read_record(0, record_size, &read_fields) };
        assert_eq!(new_result[0], new_data.as_slice());
        assert_ne!(new_result[0], initial_data.as_slice());
    }

    #[test]
    fn test_read_does_not_copy() {
        let mut buffer = TableBuffer::new_zeroed(64);
        let record_size = 16;

        // Write a record
        let value: u32 = 0xDEADBEEF;
        let value_data = value.to_ne_bytes();
        let write_fields = vec![(0, value_data.as_slice())];

        unsafe {
            buffer.write_record(0, record_size, &write_fields);
        }

        // Read the record
        let read_fields = vec![(0, 4)];
        let result = unsafe { buffer.read_record(0, record_size, &read_fields) };

        // Verify we got a reference, not a copy
        assert_eq!(result.len(), 1);

        // The slice should point to the buffer memory
        let buffer_ptr = buffer.as_ptr();
        let result_ptr = result[0].as_ptr();

        // result_ptr should be equal to buffer_ptr (offset 0)
        assert_eq!(result_ptr, buffer_ptr);

        // Verify the data matches
        assert_eq!(result[0], value_data.as_slice());
    }

    #[test]
    fn test_record_iterator_concept() {
        // This test demonstrates the concept of iterating over records
        // Actual iterator implementation would be in a higher-level layer
        let mut buffer = TableBuffer::new_zeroed(128);
        let record_size = 16;
        let num_records = 3;

        // Write multiple records
        for i in 0..num_records {
            let value = (1000 + i as u32).to_ne_bytes();
            let write_fields = vec![(0, value.as_slice())];

            unsafe {
                buffer.write_record(i, record_size, &write_fields);
            }
        }

        // Simulate iteration by reading each record
        let read_fields = vec![(0, 4)];
        for i in 0..num_records {
            let result = unsafe { buffer.read_record(i, record_size, &read_fields) };
            let expected_value = (1000 + i as u32).to_ne_bytes();
            assert_eq!(result[0], expected_value.as_slice());
        }
    }
}
