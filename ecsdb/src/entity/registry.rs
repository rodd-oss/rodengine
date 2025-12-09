use crate::error::{EcsDbError, Result};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct EntityVersion(pub u32);

#[derive(Debug, Clone)]
pub struct EntityRecord {
    pub id: EntityId,
    pub version: EntityVersion,
    pub archetype_hash: u64,
}

pub struct EntityRegistry {
    // Entities stored sequentially
    records: Vec<EntityRecord>,

    // Index: entity_id → offset in records
    index: HashMap<EntityId, usize>,

    // Reusable entity slots
    freelist: Vec<(EntityId, EntityVersion)>,

    // Next ID to allocate
    next_id: u64,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            records: Vec::with_capacity(10000),
            index: HashMap::with_capacity(10000),
            freelist: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_entity(&mut self, archetype_hash: u64) -> Result<EntityId> {
        let (entity_id, version) = if let Some((id, ver)) = self.freelist.pop() {
            // Reuse slot
            (id, ver)
        } else {
            // Allocate new
            let id = EntityId(self.next_id);
            self.next_id += 1;
            (id, EntityVersion(0))
        };

        let record = EntityRecord {
            id: entity_id,
            version,
            archetype_hash,
        };

        let offset = self.records.len();
        self.records.push(record);
        self.index.insert(entity_id, offset);

        Ok(entity_id)
    }

    pub fn delete_entity(&mut self, entity_id: EntityId) -> Result<()> {
        let offset = self
            .index
            .remove(&entity_id)
            .ok_or(EcsDbError::EntityNotFound(entity_id.0))?;

        // Mark as deleted, bump version
        if let Some(record) = self.records.get_mut(offset) {
            record.version = EntityVersion(record.version.0 + 1);
            self.freelist.push((entity_id, record.version));
        }

        Ok(())
    }

    pub fn get_entity(&self, entity_id: EntityId) -> Result<EntityRecord> {
        let offset = self
            .index
            .get(&entity_id)
            .ok_or(EcsDbError::EntityNotFound(entity_id.0))?;

        Ok(self.records[*offset].clone())
    }

    /// Returns true if the entity exists (not deleted).
    pub fn contains_entity(&self, entity_id: EntityId) -> bool {
        self.index.contains_key(&entity_id)
    }

    pub fn entity_count(&self) -> usize {
        self.records.len()
    }

    /// Returns a slice of all entity records (for snapshotting).
    pub fn records(&self) -> &[EntityRecord] {
        &self.records
    }
}

impl Default for EntityRegistry {
    fn default() -> Self {
        Self::new()
    }
}
