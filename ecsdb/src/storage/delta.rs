use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single change to a component table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOp {
    Insert {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
    },
    Update {
        table_id: u16,
        entity_id: u64,
        field_offset: usize,
        old_data: Vec<u8>,
        new_data: Vec<u8>,
    },
    Delete {
        table_id: u16,
        entity_id: u64,
        old_data: Vec<u8>,
    },
    CreateEntity {
        entity_id: u64,
    },
    DeleteEntity {
        entity_id: u64,
    },
}

/// A collection of changes that belong to a single transaction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Delta {
    pub ops: Vec<DeltaOp>,
    pub version: u64,
    pub timestamp: u64, // monotonic timestamp
}

impl Delta {
    pub fn new(version: u64, timestamp: u64) -> Self {
        Self {
            ops: Vec::new(),
            version,
            timestamp,
        }
    }

    pub fn push(&mut self, op: DeltaOp) {
        self.ops.push(op);
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Serialize delta to bytes using bincode.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(crate::error::EcsDbError::SerializationError)
    }

    /// Deserialize delta from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(crate::error::EcsDbError::SerializationError)
    }
}

/// Tracks changes made during a transaction.
/// This is attached to the write buffer and records each modification.
pub struct DeltaTracker {
    delta: Delta,
    // Map from entity ID to its previous component data (for updates/deletes)
    // This is a simplified approach; in production we'd snapshot before values.
    before_images: HashMap<(u16, u64), Vec<u8>>,
}

impl DeltaTracker {
    pub fn new(version: u64, timestamp: u64) -> Self {
        Self {
            delta: Delta::new(version, timestamp),
            before_images: HashMap::new(),
        }
    }

    /// Record an insert operation.
    pub fn record_insert(&mut self, table_id: u16, entity_id: u64, data: &[u8]) {
        self.delta.push(DeltaOp::Insert {
            table_id,
            entity_id,
            data: data.to_vec(),
        });
    }

    /// Record an update operation. Requires the old data for the field.
    pub fn record_update(
        &mut self,
        table_id: u16,
        entity_id: u64,
        field_offset: usize,
        old_data: &[u8],
        new_data: &[u8],
    ) {
        self.delta.push(DeltaOp::Update {
            table_id,
            entity_id,
            field_offset,
            old_data: old_data.to_vec(),
            new_data: new_data.to_vec(),
        });
    }

    /// Record a delete operation. Requires the old data.
    pub fn record_delete(&mut self, table_id: u16, entity_id: u64, old_data: &[u8]) {
        self.delta.push(DeltaOp::Delete {
            table_id,
            entity_id,
            old_data: old_data.to_vec(),
        });
    }

    /// Record creation of an entity.
    pub fn record_create_entity(&mut self, entity_id: u64) {
        self.delta.push(DeltaOp::CreateEntity { entity_id });
    }

    /// Record deletion of an entity.
    pub fn record_delete_entity(&mut self, entity_id: u64) {
        self.delta.push(DeltaOp::DeleteEntity { entity_id });
    }

    /// Take the accumulated delta, resetting the tracker.
    pub fn take_delta(&mut self) -> Delta {
        std::mem::take(&mut self.delta)
    }

    /// Store a before-image for a component (used for updates/deletes).
    pub fn store_before_image(&mut self, table_id: u16, entity_id: u64, data: Vec<u8>) {
        self.before_images.insert((table_id, entity_id), data);
    }

    /// Retrieve a before-image (if any).
    pub fn get_before_image(&self, table_id: u16, entity_id: u64) -> Option<Vec<u8>> {
        self.before_images.get(&(table_id, entity_id)).cloned()
    }

    /// Clear before-images.
    pub fn clear_before_images(&mut self) {
        self.before_images.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{EcsDbError, Result};

    #[test]
    fn test_delta_op_serialization() -> Result<()> {
        let op = DeltaOp::Insert {
            table_id: 1,
            entity_id: 100,
            data: vec![1, 2, 3],
        };
        let bytes = bincode::serialize(&op).map_err(EcsDbError::SerializationError)?;
        let decoded: DeltaOp =
            bincode::deserialize(&bytes).map_err(EcsDbError::SerializationError)?;
        match decoded {
            DeltaOp::Insert {
                table_id,
                entity_id,
                data,
            } => {
                assert_eq!(table_id, 1);
                assert_eq!(entity_id, 100);
                assert_eq!(data, vec![1, 2, 3]);
            }
            _ => panic!("Unexpected variant"),
        }
        Ok(())
    }

    #[test]
    fn test_delta_serialization() -> Result<()> {
        let mut delta = Delta::new(5, 12345);
        delta.push(DeltaOp::Insert {
            table_id: 2,
            entity_id: 200,
            data: vec![4, 5, 6],
        });
        let bytes = delta.serialize()?;
        let decoded = Delta::deserialize(&bytes)?;
        assert_eq!(decoded.version, 5);
        assert_eq!(decoded.timestamp, 12345);
        assert_eq!(decoded.ops.len(), 1);
        Ok(())
    }

    #[test]
    fn test_delta_tracker_record_insert() {
        let mut tracker = DeltaTracker::new(1, 1000);
        tracker.record_insert(3, 300, &[7, 8, 9]);
        let delta = tracker.take_delta();
        assert_eq!(delta.version, 1);
        assert_eq!(delta.timestamp, 1000);
        assert_eq!(delta.ops.len(), 1);
        match &delta.ops[0] {
            DeltaOp::Insert {
                table_id,
                entity_id,
                data,
            } => {
                assert_eq!(*table_id, 3);
                assert_eq!(*entity_id, 300);
                assert_eq!(data, &vec![7, 8, 9]);
            }
            _ => panic!("Unexpected op"),
        }
    }

    #[test]
    fn test_delta_tracker_record_update() {
        let mut tracker = DeltaTracker::new(2, 2000);
        tracker.record_update(4, 400, 0, &[1], &[2]);
        let delta = tracker.take_delta();
        assert_eq!(delta.ops.len(), 1);
        match &delta.ops[0] {
            DeltaOp::Update {
                table_id,
                entity_id,
                field_offset,
                old_data,
                new_data,
            } => {
                assert_eq!(*table_id, 4);
                assert_eq!(*entity_id, 400);
                assert_eq!(*field_offset, 0);
                assert_eq!(old_data, &vec![1]);
                assert_eq!(new_data, &vec![2]);
            }
            _ => panic!("Unexpected op"),
        }
    }

    #[test]
    fn test_delta_tracker_record_delete() {
        let mut tracker = DeltaTracker::new(3, 3000);
        tracker.record_delete(5, 500, &[10, 11]);
        let delta = tracker.take_delta();
        assert_eq!(delta.ops.len(), 1);
        match &delta.ops[0] {
            DeltaOp::Delete {
                table_id,
                entity_id,
                old_data,
            } => {
                assert_eq!(*table_id, 5);
                assert_eq!(*entity_id, 500);
                assert_eq!(old_data, &vec![10, 11]);
            }
            _ => panic!("Unexpected op"),
        }
    }

    #[test]
    fn test_delta_tracker_record_create_delete_entity() {
        let mut tracker = DeltaTracker::new(4, 4000);
        tracker.record_create_entity(600);
        tracker.record_delete_entity(600);
        let delta = tracker.take_delta();
        assert_eq!(delta.ops.len(), 2);
        match &delta.ops[0] {
            DeltaOp::CreateEntity { entity_id } => assert_eq!(*entity_id, 600),
            _ => panic!("Unexpected op"),
        }
        match &delta.ops[1] {
            DeltaOp::DeleteEntity { entity_id } => assert_eq!(*entity_id, 600),
            _ => panic!("Unexpected op"),
        }
    }

    #[test]
    fn test_delta_tracker_before_images() {
        let mut tracker = DeltaTracker::new(5, 5000);
        tracker.store_before_image(6, 700, vec![20, 21]);
        assert_eq!(tracker.get_before_image(6, 700), Some(vec![20, 21]));
        assert_eq!(tracker.get_before_image(6, 701), None);
        tracker.clear_before_images();
        assert_eq!(tracker.get_before_image(6, 700), None);
    }

    #[test]
    fn test_delta_tracker_take_delta_resets() {
        let mut tracker = DeltaTracker::new(6, 6000);
        tracker.record_insert(7, 800, &[30]);
        let delta1 = tracker.take_delta();
        assert_eq!(delta1.ops.len(), 1);
        assert!(tracker.take_delta().is_empty());
    }
}
