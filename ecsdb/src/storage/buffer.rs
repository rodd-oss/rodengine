use crate::error::{EcsDbError, Result};
use std::collections::{HashMap, HashSet};
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

    /// Compacts the write buffer by moving active records to fill gaps.
    /// Returns a mapping from old byte offsets to new byte offsets.
    /// After compaction, free_list is cleared and next_record_offset is updated.
    pub fn compact(&mut self) -> HashMap<usize, usize> {
        let record_size = self.record_size;
        let total_slots = self.next_record_offset as usize;
        // Build set of free slots (in slot indices)
        let free_set: HashSet<usize> = self
            .free_list
            .iter()
            .map(|&offset| offset / record_size)
            .collect();
        let mut old_to_new = HashMap::new();
        let mut new_slot = 0;
        for old_slot in 0..total_slots {
            if free_set.contains(&old_slot) {
                continue;
            }
            let old_offset = old_slot * record_size;
            let new_offset = new_slot * record_size;
            if old_offset != new_offset {
                // Ensure destination range is within buffer capacity
                let dst_end = new_offset + record_size;
                if dst_end > self.write_buffer.len() {
                    self.write_buffer.resize(dst_end, 0);
                }
                // Copy record
                let src_start = old_offset;
                let src_end = src_start + record_size;
                self.write_buffer
                    .copy_within(src_start..src_end, new_offset);
            }
            old_to_new.insert(old_offset, new_offset);
            new_slot += 1;
        }
        // Update state
        self.next_record_offset = new_slot as u64;
        self.free_list.clear();
        // Shrink buffer if it's much larger than needed (optional)
        // For now, keep capacity.
        old_to_new
    }

    /// Returns a snapshot of the current write buffer state.
    /// Used for transaction rollback.
    pub fn snapshot_state(&self) -> (Vec<u8>, u64, Vec<usize>, u64) {
        (
            self.write_buffer.clone(),
            self.next_record_offset,
            self.free_list.clone(),
            self.active_count,
        )
    }

    /// Restores write buffer state from a snapshot.
    /// Used for transaction rollback.
    pub fn restore_state(
        &mut self,
        write_buffer: Vec<u8>,
        next_record_offset: u64,
        free_list: Vec<usize>,
        active_count: u64,
    ) {
        self.write_buffer = write_buffer;
        self.next_record_offset = next_record_offset;
        self.free_list = free_list;
        self.active_count = active_count;
    }

    /// Returns the fragmentation ratio (free slots / total slots) as a value between 0.0 and 1.0.
    /// Higher values indicate more fragmentation.
    pub fn fragmentation_ratio(&self) -> f32 {
        let total_slots = self.next_record_offset as usize;
        if total_slots == 0 {
            0.0
        } else {
            self.free_list.len() as f32 / total_slots as f32
        }
    }

    /// Returns true if fragmentation exceeds the given threshold (0.0 to 1.0).
    pub fn is_fragmented(&self, threshold: f32) -> bool {
        self.fragmentation_ratio() >= threshold
    }

    /// Loads a snapshot of the entire buffer, replacing both read and write buffers.
    /// The buffer data must be a multiple of record_size.
    pub fn load_snapshot(&mut self, buffer_data: Vec<u8>, free_slots: Vec<usize>) -> Result<()> {
        if !buffer_data.len().is_multiple_of(self.record_size) {
            return Err(EcsDbError::SchemaError(format!(
                "Buffer size {} is not a multiple of record size {}",
                buffer_data.len(),
                self.record_size
            )));
        }
        // Set write buffer to the snapshot data
        self.write_buffer = buffer_data;
        // Create new read buffer arc
        let new_arc = Arc::new(self.write_buffer.clone());
        let new_ptr = Box::leak(Box::new(new_arc)) as *mut Arc<Vec<u8>>;
        // Swap read buffer pointer (old pointer will be leaked? We should drop old)
        let old_ptr = self.read_buffer.swap(new_ptr, Ordering::Release);
        // Safe deallocation of old Arc
        let _ = unsafe { Box::from_raw(old_ptr) };
        // Calculate total slots
        self.next_record_offset = (self.write_buffer.len() / self.record_size) as u64;
        self.free_list = free_slots;
        // Active count = total slots - free slots
        self.active_count = self.next_record_offset - self.free_list.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    #[test]
    fn test_arc_storage_buffer_new() {
        let buffer = ArcStorageBuffer::new(16, 1024);
        assert_eq!(buffer.record_size, 16);
        assert_eq!(buffer.write_buffer.len(), 1024);
        assert_eq!(buffer.record_count(), 0);
        assert_eq!(buffer.free_list.len(), 0);
        assert_eq!(buffer.generation(), 0);
    }

    #[test]
    fn test_insert_record_size_mismatch() {
        let mut buffer = ArcStorageBuffer::new(16, 1024);
        let data = vec![0u8; 15];
        let result = buffer.insert(&data);
        assert!(result.is_err());
        if let Err(EcsDbError::SchemaError(msg)) = result {
            assert!(msg.contains("Record size mismatch"));
        }
    }

    #[test]
    fn test_insert_and_read() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(8, 1024);
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let offset = buffer.insert(&data)?;
        assert_eq!(offset, 0);
        // Write buffer has data, but read buffer not yet committed
        let read_data = buffer.read(offset, 8)?;
        assert_eq!(read_data, vec![0u8; 8]); // read buffer still zeroed
        // Commit
        buffer.commit();
        // Now read buffer should have data
        let read_data = buffer.read(offset, 8)?;
        assert_eq!(read_data, data);
        Ok(())
    }

    #[test]
    fn test_insert_free_slot_reuse() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(4, 1024);
        let data1 = vec![1u8; 4];
        let offset1 = buffer.insert(&data1)?;
        assert_eq!(offset1, 0);
        buffer.free_slot(offset1);
        assert_eq!(buffer.free_list.len(), 1);
        // Next insert should reuse freed slot
        let data2 = vec![2u8; 4];
        let offset2 = buffer.insert(&data2)?;
        assert_eq!(offset2, offset1); // reused same offset
        assert_eq!(buffer.free_list.len(), 0);
        Ok(())
    }

    #[test]
    fn test_update() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(4, 1024);
        let data = vec![1u8; 4];
        let offset = buffer.insert(&data)?;
        buffer.commit();
        // Update write buffer
        let new_data = vec![9u8; 4];
        buffer.update(offset, &new_data)?;
        // Not yet committed, read buffer unchanged
        assert_eq!(buffer.read(offset, 4)?, data);
        buffer.commit();
        assert_eq!(buffer.read(offset, 4)?, new_data);
        Ok(())
    }

    #[test]
    fn test_generation() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(4, 1024);
        assert_eq!(buffer.generation(), 0);
        buffer.commit_with_generation(5);
        assert_eq!(buffer.generation(), 5);
        Ok(())
    }

    #[test]
    fn test_compact() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(8, 1024);
        // Insert three records
        let offsets: Vec<_> = (0..3)
            .map(|i| buffer.insert(&vec![i; 8]).unwrap())
            .collect();
        // Free middle record
        buffer.free_slot(offsets[1]);
        assert_eq!(buffer.free_list.len(), 1);
        // Compact
        let mapping = buffer.compact();
        // Expect mapping from old offset[2] (16) to new offset[1] (8)
        assert_eq!(mapping.get(&offsets[2]), Some(&8));
        // After compaction, free list cleared
        assert_eq!(buffer.free_list.len(), 0);
        // Next insert should go to slot 2 (offset 16) because next_record_offset = 2
        let offset = buffer.insert(&vec![99u8; 8])?;
        assert_eq!(offset, 16);
        Ok(())
    }

    #[test]
    fn test_snapshot_restore_state() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(4, 1024);
        let offsets: Vec<_> = (0..3)
            .map(|i| buffer.insert(&vec![i; 4]).unwrap())
            .collect();
        buffer.free_slot(offsets[1]);
        let snapshot = buffer.snapshot_state();
        // Modify buffer after snapshot
        buffer.insert(&vec![99u8; 4])?;
        buffer.free_slot(offsets[0]);
        // Restore snapshot
        buffer.restore_state(snapshot.0, snapshot.1, snapshot.2, snapshot.3);
        // Should be back to original state
        assert_eq!(buffer.next_record_offset, 3);
        assert_eq!(buffer.free_list.len(), 1);
        assert_eq!(buffer.free_list[0], offsets[1]);
        assert_eq!(buffer.active_count, 2);
        Ok(())
    }

    #[test]
    fn test_fragmentation_ratio() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(4, 1024);
        assert_eq!(buffer.fragmentation_ratio(), 0.0);
        let offset = buffer.insert(&vec![0u8; 4])?;
        buffer.free_slot(offset);
        // One free slot out of one total slot
        assert_eq!(buffer.fragmentation_ratio(), 1.0);
        buffer.insert(&vec![1u8; 4])?;
        // No free slots, total slots = 2 (next_record_offset = 2)
        assert_eq!(buffer.fragmentation_ratio(), 0.0);
        Ok(())
    }

    #[test]
    fn test_load_snapshot() -> Result<()> {
        let mut buffer = ArcStorageBuffer::new(8, 1024);
        let snapshot_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]; // two records
        let free_slots = vec![8]; // second record is free
        buffer.load_snapshot(snapshot_data.clone(), free_slots)?;
        assert_eq!(buffer.write_buffer.len(), 16);
        assert_eq!(buffer.next_record_offset, 2);
        assert_eq!(buffer.free_list, vec![8]);
        assert_eq!(buffer.active_count, 1);
        // Read buffer should have snapshot data
        buffer.commit();
        assert_eq!(buffer.read(0, 8)?, vec![1u8, 2, 3, 4, 5, 6, 7, 8]);
        Ok(())
    }

    #[test]
    fn test_load_snapshot_invalid_size() {
        let mut buffer = ArcStorageBuffer::new(8, 1024);
        let snapshot_data = vec![1u8; 15]; // not multiple of 8
        let free_slots = vec![];
        let result = buffer.load_snapshot(snapshot_data, free_slots);
        assert!(result.is_err());
    }
}
