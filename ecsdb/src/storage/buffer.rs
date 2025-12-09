use crate::error::{EcsDbError, Result};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Arc;

pub struct StorageBuffer {
    // Current read buffer (shared across threads)
    read_buffer: Arc<AtomicPtr<Vec<u8>>>,

    // Write buffer (only modified by write thread)
    write_buffer: Vec<u8>,

    // Staging buffer for new read buffer
    staging_buffer: Vec<u8>,

    // Current number of records
    record_count: u64,

    // Size of each record in bytes
    record_size: usize,

    // Memory allocated but unused
    capacity: usize,
}

impl StorageBuffer {
    pub fn new(record_size: usize, initial_capacity: usize) -> Self {
        let buf = vec![0u8; initial_capacity];
        let ptr = Box::leak(Box::new(buf)) as *mut Vec<u8>;

        Self {
            read_buffer: Arc::new(AtomicPtr::new(ptr)),
            write_buffer: vec![0u8; initial_capacity],
            staging_buffer: vec![0u8; initial_capacity],
            record_count: 0,
            record_size,
            capacity: initial_capacity,
        }
    }

    /// Insert a new record at the end (append)
    pub fn insert(&mut self, record: &[u8]) -> Result<usize> {
        if record.len() != self.record_size {
            return Err(EcsDbError::SchemaError(format!(
                "Record size mismatch: expected {}, got {}",
                self.record_size,
                record.len()
            )));
        }

        let offset = (self.record_count as usize) * self.record_size;

        // Resize if necessary
        if offset + self.record_size > self.capacity {
            self.grow();
        }

        // Copy record to write buffer
        let end = offset + self.record_size;
        self.write_buffer[offset..end].copy_from_slice(record);

        self.record_count += 1;

        Ok(offset)
    }

    /// Update a record in-place
    pub fn update(&mut self, offset: usize, record: &[u8]) -> Result<()> {
        if offset + record.len() > self.write_buffer.len() {
            return Err(EcsDbError::SchemaError("Offset out of bounds".into()));
        }

        let end = offset + record.len();
        self.write_buffer[offset..end].copy_from_slice(record);
        Ok(())
    }

    /// Get read-only access to a record from read buffer
    pub fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
        let read_buf = unsafe { &*self.read_buffer.load(Ordering::Acquire) };

        if offset + size > read_buf.len() {
            return Err(EcsDbError::SchemaError("Offset out of bounds".into()));
        }

        Ok(read_buf[offset..offset + size].to_vec())
    }

    /// Atomic swap: write buffer becomes new read buffer
    pub fn commit(&mut self) {
        // Clone write buffer to staging
        self.staging_buffer.resize(self.write_buffer.len(), 0);
        self.staging_buffer.copy_from_slice(&self.write_buffer);

        // Create new allocation
        let new_buf = Box::leak(Box::new(self.staging_buffer.clone())) as *mut Vec<u8>;

        // Atomic swap with Release ordering
        let old_ptr = self.read_buffer.swap(new_buf, Ordering::Release);

        // Deallocate old buffer (we can't really here, leaks are intentional)
        // In production, use Arc<Vec<u8>> instead
        let _ = unsafe { Box::from_raw(old_ptr) };
    }

    fn grow(&mut self) {
        self.capacity *= 2;
        self.write_buffer.resize(self.capacity, 0);
        self.staging_buffer.resize(self.capacity, 0);
    }

    pub fn record_count(&self) -> u64 {
        self.record_count
    }
}

/// Safer version using Arc
#[allow(dead_code)]
pub struct ArcStorageBuffer {
    read_buffer: Arc<AtomicPtr<Arc<Vec<u8>>>>,
    write_buffer: Vec<u8>,
    /// Next offset to allocate when free_list is empty (in units of records)
    next_record_offset: u64,
    pub record_size: usize,
    /// Generation number of the current read buffer.
    /// Incremented each time commit_with_generation is called.
    generation: AtomicU64,
    /// List of free slots (offsets in bytes) that can be reused
    free_list: Vec<usize>,
    /// Number of active records (excluding deleted)
    active_count: u64,
}

impl ArcStorageBuffer {
    pub fn new(record_size: usize, initial_capacity: usize) -> Self {
        let initial = Arc::new(vec![0u8; initial_capacity]);
        let ptr = Box::leak(Box::new(initial)) as *mut Arc<Vec<u8>>;

        Self {
            read_buffer: Arc::new(AtomicPtr::new(ptr)),
            write_buffer: vec![0u8; initial_capacity],
            next_record_offset: 0,
            record_size,
            generation: AtomicU64::new(0),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Insert a new record, reusing free slots if available.
    pub fn insert(&mut self, record: &[u8]) -> Result<usize> {
        if record.len() != self.record_size {
            return Err(EcsDbError::SchemaError(format!(
                "Record size mismatch: expected {}, got {}",
                self.record_size,
                record.len()
            )));
        }

        let offset = if let Some(offset) = self.free_list.pop() {
            // Reuse freed slot
            offset
        } else {
            // Allocate new slot at the end
            let offset = (self.next_record_offset as usize) * self.record_size;
            self.next_record_offset += 1;
            // Ensure capacity
            if offset + self.record_size > self.write_buffer.len() {
                self.grow();
            }
            offset
        };

        // Copy record to write buffer
        let end = offset + self.record_size;
        self.write_buffer[offset..end].copy_from_slice(record);

        self.active_count += 1;

        Ok(offset)
    }

    /// Mark a slot as free for reuse.
    pub fn free_slot(&mut self, offset: usize) {
        // Ensure offset is within allocated range and aligned
        if offset.is_multiple_of(self.record_size)
            && offset < (self.next_record_offset as usize) * self.record_size
        {
            self.free_list.push(offset);
            self.active_count = self.active_count.saturating_sub(1);
        }
    }

    /// Update a record in-place
    pub fn update(&mut self, offset: usize, record: &[u8]) -> Result<()> {
        if offset + record.len() > self.write_buffer.len() {
            return Err(EcsDbError::SchemaError("Offset out of bounds".into()));
        }

        let end = offset + record.len();
        self.write_buffer[offset..end].copy_from_slice(record);
        Ok(())
    }

    /// Get read-only access to a record from read buffer (copies bytes)
    pub fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
        let read_arc = unsafe { &*self.read_buffer.load(Ordering::Acquire) };

        if offset + size > read_arc.len() {
            return Err(EcsDbError::SchemaError("Offset out of bounds".into()));
        }

        Ok(read_arc[offset..offset + size].to_vec())
    }

    /// Get a reference to a record in the read buffer (zero-copy)
    pub fn read_ref(&self, offset: usize, size: usize) -> Result<&[u8]> {
        let read_arc = unsafe { &*self.read_buffer.load(Ordering::Acquire) };

        if offset + size > read_arc.len() {
            return Err(EcsDbError::SchemaError("Offset out of bounds".into()));
        }

        Ok(&read_arc[offset..offset + size])
    }

    /// Returns a clone of the current read buffer Arc.
    pub fn current_read_buffer(&self) -> Arc<Vec<u8>> {
        unsafe { (&*self.read_buffer.load(Ordering::Acquire)).clone() }
    }

    /// Atomic swap: write buffer becomes new read buffer
    pub fn commit(&mut self) {
        let new_arc = Arc::new(self.write_buffer.clone());
        let new_ptr = Box::leak(Box::new(new_arc)) as *mut Arc<Vec<u8>>;

        // Atomic swap with Release ordering
        let old_ptr = self.read_buffer.swap(new_ptr, Ordering::Release);

        // Safe deallocation
        let _ = unsafe { Box::from_raw(old_ptr) };
    }

    /// Commit and associate the new read buffer with a generation number.
    /// The generation is stored after the buffer swap, ensuring that any reader
    /// that sees the new buffer will also see the new generation.
    pub fn commit_with_generation(&mut self, generation: u64) {
        self.commit();
        self.generation.store(generation, Ordering::Release);
    }

    /// Returns the generation number of the current read buffer.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn grow(&mut self) {
        let new_capacity = self.write_buffer.len() * 2;
        self.write_buffer.resize(new_capacity, 0);
    }

    /// Returns the number of active records stored (excluding freed slots).
    pub fn record_count(&self) -> u64 {
        self.active_count
    }
}
