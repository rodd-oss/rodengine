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
