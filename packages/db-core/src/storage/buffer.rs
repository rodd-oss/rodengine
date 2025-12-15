//! TableBuffer - A Vec<u8> storage buffer for table data with zero-copy access.
//!
//! This buffer provides contiguous memory storage with capacity management,
//! designed for cache-efficient access and future unsafe casting to field types.

use db_types::{Field, FieldError, Type, Value};
use std::ops::{Deref, DerefMut};

/// A storage buffer backed by `Vec<u8>` for table records.
///
/// Provides capacity management and basic read/write operations.
/// Designed to be wrapped in `ArcSwap` for lock-free concurrent access.
#[derive(Debug, Clone)]
pub struct TableBuffer {
    data: Vec<u8>,
}

impl TableBuffer {
    /// Creates a new empty buffer with default capacity.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Creates a new empty buffer with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Returns the current capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns the current length (number of bytes) of the buffer.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Reserves capacity for at least `additional` more bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Clears the buffer, removing all data.
    ///
    /// This does not affect the capacity.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Writes data into the buffer at the specified offset.
    ///
    /// If the offset is beyond the current length, the buffer will be extended
    /// with zeros up to the offset before writing.
    ///
    /// # Panics
    ///
    /// Panics if the write would exceed the buffer's capacity.
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        let required_len = offset + data.len();
        if required_len > self.data.capacity() {
            panic!("Write would exceed buffer capacity");
        }

        if required_len > self.data.len() {
            self.data.resize(required_len, 0);
        }

        self.data[offset..offset + data.len()].copy_from_slice(data);
    }

    /// Reads data from the buffer at the specified offset.
    ///
    /// Returns a slice of the requested length, or an empty slice if the
    /// offset is beyond the buffer length.
    pub fn read(&self, offset: usize, len: usize) -> &[u8] {
        if offset >= self.data.len() {
            return &[];
        }

        let end = std::cmp::min(offset + len, self.data.len());
        &self.data[offset..end]
    }

    /// Writes a record into the buffer at the specified offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset within the buffer where the record starts
    /// * `fields` - Field definitions for this record
    /// * `values` - Values corresponding to each field
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or `Err(FieldError)` if:
    /// - The number of values doesn't match the number of fields
    /// - The record would extend beyond buffer bounds
    /// - A field offset is misaligned
    ///
    /// # Safety
    ///
    /// This function uses unsafe pointer operations to write values directly
    /// into the buffer. It assumes the buffer has been properly allocated
    /// and the offset is valid.
    pub fn write_record(
        &mut self,
        offset: usize,
        fields: &[Field],
        values: &[Value],
    ) -> Result<(), FieldError> {
        if fields.len() != values.len() {
            return Err(FieldError::InvalidName); // Using InvalidName as a generic error for now
        }

        // Validate the record fits in the buffer
        let record_size = db_types::calculate_record_size(fields);
        let required_len = offset
            .checked_add(record_size)
            .ok_or(FieldError::Overflow)?;
        if required_len > self.data.capacity() {
            return Err(FieldError::OutOfBounds);
        }

        // Ensure buffer is large enough
        if required_len > self.data.len() {
            self.data.resize(required_len, 0);
        }

        // Write each field
        for (field, value) in fields.iter().zip(values.iter()) {
            // Calculate absolute offset for this field
            let field_offset = offset
                .checked_add(field.offset())
                .ok_or(FieldError::Overflow)?;

            // Validate field alignment at absolute offset
            if !field_offset.is_multiple_of(field.ty().alignment()) {
                return Err(FieldError::Misaligned);
            }

            // Write the value based on its type
            match value {
                Value::I8(v) => self.write_i8(field_offset, *v),
                Value::I16(v) => self.write_i16(field_offset, *v),
                Value::I32(v) => self.write_i32(field_offset, *v),
                Value::I64(v) => self.write_i64(field_offset, *v),
                Value::I128(v) => self.write_i128(field_offset, *v),
                Value::U8(v) => self.write_u8(field_offset, *v),
                Value::U16(v) => self.write_u16(field_offset, *v),
                Value::U32(v) => self.write_u32(field_offset, *v),
                Value::U64(v) => self.write_u64(field_offset, *v),
                Value::U128(v) => self.write_u128(field_offset, *v),
                Value::F32(v) => self.write_f32(field_offset, *v),
                Value::F64(v) => self.write_f64(field_offset, *v),
                Value::Bool(v) => self.write_bool(field_offset, *v),
                Value::String(v) => self.write_string(field_offset, v),
            }
        }

        Ok(())
    }

    /// Reads a record from the buffer at the specified offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset within the buffer where the record starts
    /// * `fields` - Field definitions for this record
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<Value>)` with values for each field, or `Err(FieldError)` if:
    /// - The record would extend beyond buffer bounds
    /// - A field offset is misaligned
    ///
    /// # Safety
    ///
    /// This function uses unsafe pointer operations to read values directly
    /// from the buffer. It assumes the buffer contains valid data for the
    /// specified field types.
    pub fn read_record(&self, offset: usize, fields: &[Field]) -> Result<Vec<Value>, FieldError> {
        // Validate the record fits in the buffer
        let record_size = db_types::calculate_record_size(fields);
        let required_len = offset
            .checked_add(record_size)
            .ok_or(FieldError::Overflow)?;
        if required_len > self.data.len() {
            return Err(FieldError::OutOfBounds);
        }

        let mut values = Vec::with_capacity(fields.len());

        // Read each field
        for field in fields {
            // Calculate absolute offset for this field
            let field_offset = offset
                .checked_add(field.offset())
                .ok_or(FieldError::Overflow)?;

            // Validate field alignment at absolute offset
            if !field_offset.is_multiple_of(field.ty().alignment()) {
                return Err(FieldError::Misaligned);
            }

            // Read the value based on its type
            let value = match field.ty() {
                Type::I8 => Value::I8(self.read_i8(field_offset)),
                Type::I16 => Value::I16(self.read_i16(field_offset)),
                Type::I32 => Value::I32(self.read_i32(field_offset)),
                Type::I64 => Value::I64(self.read_i64(field_offset)),
                Type::I128 => Value::I128(self.read_i128(field_offset)),
                Type::U8 => Value::U8(self.read_u8(field_offset)),
                Type::U16 => Value::U16(self.read_u16(field_offset)),
                Type::U32 => Value::U32(self.read_u32(field_offset)),
                Type::U64 => Value::U64(self.read_u64(field_offset)),
                Type::U128 => Value::U128(self.read_u128(field_offset)),
                Type::F32 => Value::F32(self.read_f32(field_offset)),
                Type::F64 => Value::F64(self.read_f64(field_offset)),
                Type::Bool => Value::Bool(self.read_bool(field_offset)),
                Type::String => Value::String(self.read_string(field_offset)),
            };

            values.push(value);
        }

        Ok(values)
    }

    // Unsafe write methods for each type

    fn write_i8(&mut self, offset: usize, value: i8) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut i8;
            ptr.write(value);
        }
    }

    fn write_i16(&mut self, offset: usize, value: i16) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut i16;
            ptr.write(value);
        }
    }

    fn write_i32(&mut self, offset: usize, value: i32) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut i32;
            ptr.write(value);
        }
    }

    fn write_i64(&mut self, offset: usize, value: i64) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut i64;
            ptr.write(value);
        }
    }

    fn write_i128(&mut self, offset: usize, value: i128) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut i128;
            ptr.write(value);
        }
    }

    fn write_u8(&mut self, offset: usize, value: u8) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset);
            ptr.write(value);
        }
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut u16;
            ptr.write(value);
        }
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut u32;
            ptr.write(value);
        }
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut u64;
            ptr.write(value);
        }
    }

    fn write_u128(&mut self, offset: usize, value: u128) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut u128;
            ptr.write(value);
        }
    }

    fn write_f32(&mut self, offset: usize, value: f32) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut f32;
            ptr.write(value);
        }
    }

    fn write_f64(&mut self, offset: usize, value: f64) {
        unsafe {
            let ptr = self.data.as_mut_ptr().add(offset) as *mut f64;
            ptr.write(value);
        }
    }

    fn write_bool(&mut self, offset: usize, value: bool) {
        self.write_u8(offset, value as u8);
    }

    fn write_string(&mut self, offset: usize, value: &str) {
        // Write length as u64
        let len = value.len() as u64;
        self.write_u64(offset, len);

        // Write string bytes
        let bytes = value.as_bytes();
        let data_offset = offset + 8;
        if data_offset + bytes.len() <= self.data.len() {
            self.data[data_offset..data_offset + bytes.len()].copy_from_slice(bytes);
        }
    }

    // Unsafe read methods for each type

    fn read_i8(&self, offset: usize) -> i8 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const i8;
            ptr.read()
        }
    }

    fn read_i16(&self, offset: usize) -> i16 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const i16;
            ptr.read()
        }
    }

    fn read_i32(&self, offset: usize) -> i32 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const i32;
            ptr.read()
        }
    }

    fn read_i64(&self, offset: usize) -> i64 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const i64;
            ptr.read()
        }
    }

    fn read_i128(&self, offset: usize) -> i128 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const i128;
            ptr.read()
        }
    }

    fn read_u8(&self, offset: usize) -> u8 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset);
            ptr.read()
        }
    }

    fn read_u16(&self, offset: usize) -> u16 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const u16;
            ptr.read()
        }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const u32;
            ptr.read()
        }
    }

    fn read_u64(&self, offset: usize) -> u64 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const u64;
            ptr.read()
        }
    }

    fn read_u128(&self, offset: usize) -> u128 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const u128;
            ptr.read()
        }
    }

    fn read_f32(&self, offset: usize) -> f32 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const f32;
            ptr.read()
        }
    }

    fn read_f64(&self, offset: usize) -> f64 {
        unsafe {
            let ptr = self.data.as_ptr().add(offset) as *const f64;
            ptr.read()
        }
    }

    fn read_bool(&self, offset: usize) -> bool {
        self.read_u8(offset) != 0
    }

    fn read_string(&self, offset: usize) -> String {
        // Read length
        let len = self.read_u64(offset) as usize;

        // Read string bytes
        let data_offset = offset + 8;
        if data_offset + len <= self.data.len() {
            let bytes = &self.data[data_offset..data_offset + len];
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            String::new()
        }
    }
}

impl Default for TableBuffer {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_buffer_new_01() {
        let buffer = TableBuffer::new();
        assert_eq!(buffer.capacity(), 0);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_table_buffer_new_02() {
        let buffer = TableBuffer::with_capacity(1024);
        assert!(buffer.capacity() >= 1024);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_table_buffer_with_capacity_01() {
        let buffer = TableBuffer::with_capacity(0);
        assert_eq!(buffer.capacity(), 0);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_table_buffer_with_capacity_02() {
        let buffer = TableBuffer::with_capacity(512);
        assert!(buffer.capacity() >= 512);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_table_buffer_capacity_01() {
        let buffer = TableBuffer::new();
        assert_eq!(buffer.capacity(), 0);
    }

    #[test]
    fn test_table_buffer_capacity_02() {
        let mut buffer = TableBuffer::with_capacity(10);
        buffer.reserve(100);
        assert!(buffer.capacity() >= 10);
    }

    #[test]
    fn test_table_buffer_len_01() {
        let buffer = TableBuffer::with_capacity(100);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_table_buffer_len_02() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(0, b"test");
        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn test_table_buffer_reserve_01() {
        let mut buffer = TableBuffer::with_capacity(10);
        let old_capacity = buffer.capacity();
        buffer.reserve(50);
        assert!(buffer.capacity() >= old_capacity);
    }

    #[test]
    fn test_table_buffer_reserve_02() {
        let mut buffer = TableBuffer::with_capacity(10);
        let old_capacity = buffer.capacity();
        buffer.reserve(0);
        assert_eq!(buffer.capacity(), old_capacity);
    }

    #[test]
    fn test_table_buffer_clear_01() {
        let mut buffer = TableBuffer::with_capacity(100);
        let old_capacity = buffer.capacity();
        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), old_capacity);
    }

    #[test]
    fn test_table_buffer_clear_02() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(0, b"data");
        let old_capacity = buffer.capacity();
        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), old_capacity);
        assert!(buffer.read(0, 4).is_empty());
    }

    #[test]
    fn test_table_buffer_write_01() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(0, b"hello");
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.read(0, 5), b"hello");
    }

    #[test]
    fn test_table_buffer_write_02() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(10, b"world");
        assert_eq!(buffer.len(), 15);
        assert_eq!(buffer.read(10, 5), b"world");
        assert_eq!(buffer.read(0, 10), vec![0u8; 10].as_slice());
    }

    #[test]
    #[should_panic(expected = "Write would exceed buffer capacity")]
    fn test_table_buffer_write_03() {
        let mut buffer = TableBuffer::with_capacity(5);
        buffer.write(0, b"overflow");
    }

    #[test]
    fn test_table_buffer_write_04() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(0, &[]);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_table_buffer_read_01() {
        let mut buffer = TableBuffer::with_capacity(100);
        buffer.write(0, b"test");
        assert_eq!(buffer.read(0, 4), b"test");
    }

    #[test]
    fn test_table_buffer_read_02() {
        let buffer = TableBuffer::with_capacity(100);
        assert!(buffer.read(10, 5).is_empty());
    }

    #[test]
    fn test_table_buffer_read_03() {
        let buffer = TableBuffer::with_capacity(100);
        assert!(buffer.read(0, 0).is_empty());
    }

    #[test]
    fn test_write_record_01() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        let values = vec![Value::I32(42)];

        buffer.write_record(0, &fields, &values).unwrap();

        // Verify by reading back
        let read_values = buffer.read_record(0, &fields).unwrap();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_write_record_02() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![
            Field::new("id".to_string(), Type::I32, 0),
            Field::new("active".to_string(), Type::Bool, 4),
        ];
        let values = vec![Value::I32(123), Value::Bool(true)];

        buffer.write_record(0, &fields, &values).unwrap();

        let read_values = buffer.read_record(0, &fields).unwrap();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_write_record_03() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("score".to_string(), Type::F64, 0)];
        let values = vec![Value::F64(std::f64::consts::PI)];

        buffer.write_record(16, &fields, &values).unwrap();

        let read_values = buffer.read_record(16, &fields).unwrap();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_write_record_04() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        let values = vec![];

        let result = buffer.write_record(0, &fields, &values);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_record_05() {
        let mut buffer = TableBuffer::with_capacity(10);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        let values = vec![Value::I32(1)];

        let result = buffer.write_record(8, &fields, &values);
        assert!(result.is_err()); // Would need 12 bytes, only have 10 capacity
    }

    #[test]
    fn test_write_record_06() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        let values = vec![Value::I32(42)];

        // Offset 1 is misaligned for i32
        let result = buffer.write_record(1, &fields, &values);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_record_01() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];
        let values = vec![Value::I32(42)];

        buffer.write_record(0, &fields, &values).unwrap();
        let read_values = buffer.read_record(0, &fields).unwrap();
        assert_eq!(read_values, vec![Value::I32(42)]);
    }

    #[test]
    fn test_read_record_02() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![
            Field::new("id".to_string(), Type::I32, 0),
            Field::new("active".to_string(), Type::Bool, 4),
        ];
        let values = vec![Value::I32(123), Value::Bool(true)];

        buffer.write_record(0, &fields, &values).unwrap();
        let read_values = buffer.read_record(0, &fields).unwrap();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_read_record_03() {
        let mut buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("score".to_string(), Type::F64, 0)];
        let values = vec![Value::F64(std::f64::consts::PI)];

        buffer.write_record(16, &fields, &values).unwrap();
        let read_values = buffer.read_record(16, &fields).unwrap();
        assert_eq!(read_values, values);
    }

    #[test]
    fn test_read_record_04() {
        let buffer = TableBuffer::with_capacity(10);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];

        let result = buffer.read_record(8, &fields);
        assert!(result.is_err()); // Would need 12 bytes, only have 10 length
    }

    #[test]
    fn test_read_record_05() {
        let buffer = TableBuffer::with_capacity(100);
        let fields = vec![Field::new("id".to_string(), Type::I32, 0)];

        // Offset 1 is misaligned for i32
        let result = buffer.read_record(1, &fields);
        assert!(result.is_err());
    }
}
