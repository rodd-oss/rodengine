use crate::component::{Component, ZeroCopyComponent};
use crate::error::{EcsDbError, Result};
use crate::storage::buffer::ArcStorageBuffer;
use crate::storage::field_codec;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Table for storing components of a specific type.
/// Uses fixed-size records and zero-copy access.
pub struct ComponentTable<T: Component> {
    buffer: ArcStorageBuffer,
    entity_index: HashMap<u64, usize>, // entity_id -> byte offset in buffer
    _marker: PhantomData<T>,
}

impl<T: Component> ComponentTable<T> {
    /// Creates a new component table with the given record size and initial capacity.
    pub fn new(record_size: usize, initial_capacity: usize) -> Self {
        Self {
            buffer: ArcStorageBuffer::new(record_size, initial_capacity),
            entity_index: HashMap::new(),
            _marker: PhantomData,
        }
    }

    /// Creates a new component table using the static size of the component.
    /// Requires that the component has a fixed size (e.g., repr(C)).
    pub fn with_static_size(initial_capacity: usize) -> Self
    where
        T: ZeroCopyComponent,
    {
        let record_size = <T as ZeroCopyComponent>::static_size();
        Self::new(record_size, initial_capacity)
    }

    /// Inserts a component for the given entity.
    /// Returns the byte offset where the component was stored.
    pub fn insert(&mut self, entity_id: u64, component: &T) -> Result<usize> {
        // Serialize component to bytes
        let bytes = field_codec::encode(component)?;

        // Ensure serialized size matches buffer record size
        if bytes.len() != self.buffer.record_size {
            return Err(EcsDbError::SchemaError(format!(
                "Serialized component size {} does not match table record size {}",
                bytes.len(),
                self.buffer.record_size
            )));
        }

        // Insert into buffer
        let offset = self.buffer.insert(&bytes)?;

        // Update entity index
        self.entity_index.insert(entity_id, offset);

        Ok(offset)
    }

    /// Updates an existing component for the given entity.
    pub fn update(&mut self, entity_id: u64, component: &T) -> Result<()> {
        let offset =
            self.entity_index
                .get(&entity_id)
                .ok_or_else(|| EcsDbError::ComponentNotFound {
                    entity_id,
                    component_type: std::any::type_name::<T>().to_string(),
                })?;

        let bytes = field_codec::encode(component)?;
        if bytes.len() != self.buffer.record_size {
            return Err(EcsDbError::SchemaError(format!(
                "Serialized component size {} does not match table record size {}",
                bytes.len(),
                self.buffer.record_size
            )));
        }

        self.buffer.update(*offset, &bytes)
    }

    /// Deletes the component for the given entity.
    /// Removes from index and marks the buffer slot as free for reuse.
    pub fn delete(&mut self, entity_id: u64) -> Result<()> {
        let offset =
            self.entity_index
                .remove(&entity_id)
                .ok_or_else(|| EcsDbError::ComponentNotFound {
                    entity_id,
                    component_type: std::any::type_name::<T>().to_string(),
                })?;

        self.buffer.free_slot(offset);
        Ok(())
    }

    /// Retrieves the component for the given entity.
    /// Deserializes from stored bytes.
    pub fn get(&self, entity_id: u64) -> Result<T> {
        let offset =
            self.entity_index
                .get(&entity_id)
                .ok_or_else(|| EcsDbError::ComponentNotFound {
                    entity_id,
                    component_type: std::any::type_name::<T>().to_string(),
                })?;

        // Read bytes from buffer
        let bytes = self.buffer.read(*offset, self.buffer.record_size)?;

        // Deserialize component
        field_codec::decode(&bytes)
    }

    /// Commits pending writes, making them visible to readers.
    pub fn commit(&mut self) {
        self.buffer.commit();
    }

    /// Commits pending writes and associates the new buffer with a generation number.
    pub fn commit_with_generation(&mut self, generation: u64) {
        self.buffer.commit_with_generation(generation);
    }

    /// Returns the generation number of the current read buffer.
    pub fn generation(&self) -> u64 {
        self.buffer.generation()
    }

    /// Returns a snapshot of the current read buffer.
    pub fn snapshot(&self) -> std::sync::Arc<Vec<u8>> {
        self.buffer.current_read_buffer()
    }

    /// Returns the number of components stored in this table.
    pub fn len(&self) -> usize {
        self.entity_index.len()
    }

    /// Returns true if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entity_index.is_empty()
    }

    /// Returns true if the entity has a component in this table.
    pub fn contains_entity(&self, entity_id: u64) -> bool {
        self.entity_index.contains_key(&entity_id)
    }

    /// Returns mapping from entity ID to byte offset in the read buffer.
    /// Used for snapshot serialization.
    pub fn entity_mapping(&self) -> Vec<(u64, usize)> {
        self.entity_index
            .iter()
            .map(|(&id, &offset)| (id, offset))
            .collect()
    }

    /// Returns the record size used by this table.
    pub fn record_size(&self) -> usize {
        self.buffer.record_size
    }

    /// Compacts the storage buffer, moving active records to fill gaps.
    /// Updates internal entity index to reflect new offsets.
    pub fn compact(&mut self) {
        let mapping = self.buffer.compact();
        // Update entity_index offsets
        for offset in self.entity_index.values_mut() {
            if let Some(new_offset) = mapping.get(offset) {
                *offset = *new_offset;
            }
        }
    }

    /// Returns the fragmentation ratio (free slots / total slots) as a value between 0.0 and 1.0.
    pub fn fragmentation_ratio(&self) -> f32 {
        self.buffer.fragmentation_ratio()
    }

    /// Returns true if fragmentation exceeds the given threshold (0.0 to 1.0).
    pub fn is_fragmented(&self, threshold: f32) -> bool {
        self.buffer.is_fragmented(threshold)
    }

    /// Returns a snapshot of the write buffer state for rollback.
    pub fn snapshot_write_state(&self) -> (Vec<u8>, u64, Vec<usize>, u64) {
        self.buffer.snapshot_state()
    }

    /// Restores write buffer state from a snapshot.
    pub fn restore_write_state(
        &mut self,
        write_buffer: Vec<u8>,
        next_record_offset: u64,
        free_list: Vec<usize>,
        active_count: u64,
    ) {
        self.buffer
            .restore_state(write_buffer, next_record_offset, free_list, active_count);
        // After restore, entity index may be invalid because offsets changed.
        // Since rollback restores exact state, offsets should match existing entity index.
        // We assume no compaction occurred during the batch.
    }

    /// Loads snapshot data into the table, replacing the current buffer and index.
    /// This resets both read and write buffers to the provided snapshot data.
    pub fn load_snapshot(
        &mut self,
        buffer_data: Vec<u8>,
        entity_mapping: Vec<(u64, usize)>,
        free_slots: Vec<usize>,
    ) -> Result<()> {
        // Validate buffer size matches record size
        if !buffer_data.len().is_multiple_of(self.buffer.record_size) {
            return Err(EcsDbError::SchemaError(format!(
                "Buffer size {} is not a multiple of record size {}",
                buffer_data.len(),
                self.buffer.record_size
            )));
        }
        // Load into buffer
        self.buffer.load_snapshot(buffer_data, free_slots)?;
        // Rebuild entity index
        self.entity_index.clear();
        for (entity_id, offset) in entity_mapping {
            self.entity_index.insert(entity_id, offset);
        }
        Ok(())
    }
}

// Implement ZeroCopyComponent for simple primitives as example.
// Users must implement this trait manually for their custom components.

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
    struct TestComponent {
        x: f32,
        y: f32,
        id: u32,
    }

    impl Component for TestComponent {
        const TABLE_ID: u16 = 1;
        const TABLE_NAME: &'static str = "test_component";
    }

    // For testing, we'll implement ZeroCopyComponent (unsafe but we know layout)
    unsafe impl ZeroCopyComponent for TestComponent {
        fn static_size() -> usize {
            std::mem::size_of::<TestComponent>()
        }

        fn alignment() -> usize {
            std::mem::align_of::<TestComponent>()
        }
    }

    #[test]
    fn test_table_insert_get() -> Result<()> {
        let mut table = ComponentTable::<TestComponent>::with_static_size(1024);

        let comp = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 42,
        };
        table.insert(1, &comp)?;
        table.commit(); // Make visible

        // Retrieve component
        let retrieved = table.get(1)?;
        assert_eq!(retrieved, comp);

        // Update component
        let updated = TestComponent {
            x: 3.0,
            y: 4.0,
            id: 43,
        };
        table.update(1, &updated)?;
        table.commit(); // Make visible
        let retrieved = table.get(1)?;
        assert_eq!(retrieved, updated);

        // Delete component
        table.delete(1)?;
        table.commit(); // Make visible
        assert!(table.get(1).is_err());

        Ok(())
    }

    #[test]
    fn test_table_commit() -> Result<()> {
        let mut table = ComponentTable::<TestComponent>::with_static_size(1024);

        let comp = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 42,
        };
        table.insert(1, &comp)?;

        // Commit should not affect reads
        table.commit();

        let retrieved = table.get(1)?;
        assert_eq!(retrieved, comp);

        Ok(())
    }

    #[test]
    fn test_table_compact() -> Result<()> {
        let mut table = ComponentTable::<TestComponent>::with_static_size(1024);
        // Insert a few components
        let comp_a = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 1,
        };
        let comp_b = TestComponent {
            x: 3.0,
            y: 4.0,
            id: 2,
        };
        let comp_c = TestComponent {
            x: 5.0,
            y: 6.0,
            id: 3,
        };
        table.insert(1, &comp_a)?;
        table.insert(2, &comp_b)?;
        table.insert(3, &comp_c)?;
        table.commit(); // Make inserts visible
                        // Delete middle component to create a gap
        table.delete(2)?;
        table.commit(); // Make delete visible
                        // Ensure component 2 is gone
        assert!(table.get(2).is_err());
        // Compact (operates on write buffer)
        table.compact();
        table.commit(); // Make compaction visible
                        // Verify components 1 and 3 still accessible
        assert_eq!(table.get(1)?, comp_a);
        assert_eq!(table.get(3)?, comp_c);
        Ok(())
    }
}
