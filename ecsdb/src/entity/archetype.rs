//! Archetype tracking for ECS.
//!
//! Archetypes group entities by their component composition for cache‑friendly iteration.

use std::collections::HashMap;

/// Bitmask representing a set of component types.
/// Each bit corresponds to a component table ID (0‑63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeMask(pub u64);

impl ArchetypeMask {
    /// Creates an empty archetype mask (no components).
    pub fn empty() -> Self {
        Self(0)
    }

    /// Creates a mask with a single component type.
    pub fn with_component(table_id: u16) -> Self {
        debug_assert!(table_id < 64);
        Self(1 << table_id)
    }

    /// Adds a component type to the mask.
    pub fn add_component(&mut self, table_id: u16) {
        debug_assert!(table_id < 64);
        self.0 |= 1 << table_id;
    }

    /// Removes a component type from the mask.
    pub fn remove_component(&mut self, table_id: u16) {
        debug_assert!(table_id < 64);
        self.0 &= !(1 << table_id);
    }

    /// Returns true if the mask contains the given component type.
    pub fn contains(&self, table_id: u16) -> bool {
        debug_assert!(table_id < 64);
        (self.0 & (1 << table_id)) != 0
    }

    /// Returns the raw mask value.
    pub fn bits(&self) -> u64 {
        self.0
    }
}

/// Archetype identifier, currently just the mask.
pub type ArchetypeId = ArchetypeMask;

/// Tracks which entities belong to which archetype.
pub struct ArchetypeRegistry {
    /// Map from archetype mask to list of entity IDs.
    archetype_entities: HashMap<ArchetypeMask, Vec<u64>>,
    /// Map from entity ID to its archetype mask.
    entity_archetype: HashMap<u64, ArchetypeMask>,
}

impl ArchetypeRegistry {
    /// Creates a new empty archetype registry.
    pub fn new() -> Self {
        Self {
            archetype_entities: HashMap::new(),
            entity_archetype: HashMap::new(),
        }
    }

    /// Adds an entity with an initial archetype mask (usually empty).
    pub fn add_entity(&mut self, entity_id: u64, archetype: ArchetypeMask) {
        self.entity_archetype.insert(entity_id, archetype);
        self.archetype_entities
            .entry(archetype)
            .or_default()
            .push(entity_id);
    }

    /// Removes an entity from its archetype.
    pub fn remove_entity(&mut self, entity_id: u64) -> Option<ArchetypeMask> {
        let archetype = self.entity_archetype.remove(&entity_id)?;
        if let Some(entities) = self.archetype_entities.get_mut(&archetype) {
            if let Some(pos) = entities.iter().position(|&id| id == entity_id) {
                entities.swap_remove(pos);
            }
        }
        Some(archetype)
    }

    /// Updates an entity's archetype by adding a component.
    /// Returns the old archetype mask.
    pub fn add_component(&mut self, entity_id: u64, table_id: u16) -> Option<ArchetypeMask> {
        let old = self.entity_archetype.get_mut(&entity_id)?;
        let old_mask = *old;
        old.add_component(table_id);
        let new_mask = *old;

        if old_mask != new_mask {
            // Move entity to new archetype list
            self.move_entity(entity_id, old_mask, new_mask);
        }
        Some(old_mask)
    }

    /// Updates an entity's archetype by removing a component.
    /// Returns the old archetype mask.
    pub fn remove_component(&mut self, entity_id: u64, table_id: u16) -> Option<ArchetypeMask> {
        let old = self.entity_archetype.get_mut(&entity_id)?;
        let old_mask = *old;
        old.remove_component(table_id);
        let new_mask = *old;

        if old_mask != new_mask {
            self.move_entity(entity_id, old_mask, new_mask);
        }
        Some(old_mask)
    }

    /// Moves an entity from one archetype list to another.
    fn move_entity(&mut self, entity_id: u64, from: ArchetypeMask, to: ArchetypeMask) {
        // Remove from old list
        if let Some(entities) = self.archetype_entities.get_mut(&from) {
            if let Some(pos) = entities.iter().position(|&id| id == entity_id) {
                entities.swap_remove(pos);
            }
        }
        // Add to new list
        self.archetype_entities
            .entry(to)
            .or_default()
            .push(entity_id);
    }

    /// Returns the archetype mask for the given entity, if known.
    pub fn get_archetype(&self, entity_id: u64) -> Option<ArchetypeMask> {
        self.entity_archetype.get(&entity_id).copied()
    }

    /// Returns all entities belonging to a given archetype.
    pub fn entities_in_archetype(&self, archetype: ArchetypeMask) -> &[u64] {
        self.archetype_entities
            .get(&archetype)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the number of entities in a given archetype.
    pub fn archetype_size(&self, archetype: ArchetypeMask) -> usize {
        self.archetype_entities
            .get(&archetype)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

impl Default for ArchetypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archetype_mask() {
        let mut mask = ArchetypeMask::empty();
        assert!(!mask.contains(0));
        assert!(!mask.contains(5));

        mask.add_component(5);
        assert!(mask.contains(5));
        assert!(!mask.contains(4));
        assert_eq!(mask.bits(), 1 << 5);

        mask.remove_component(5);
        assert!(!mask.contains(5));
        assert_eq!(mask.bits(), 0);
    }

    #[test]
    fn test_archetype_registry() {
        let mut reg = ArchetypeRegistry::new();

        // Add entity with empty archetype
        reg.add_entity(1, ArchetypeMask::empty());
        assert_eq!(reg.get_archetype(1), Some(ArchetypeMask::empty()));
        assert_eq!(reg.archetype_size(ArchetypeMask::empty()), 1);

        // Add component
        reg.add_component(1, 3);
        let expected = ArchetypeMask::with_component(3);
        assert_eq!(reg.get_archetype(1), Some(expected));
        assert_eq!(reg.archetype_size(ArchetypeMask::empty()), 0);
        assert_eq!(reg.archetype_size(expected), 1);

        // Remove component
        reg.remove_component(1, 3);
        assert_eq!(reg.get_archetype(1), Some(ArchetypeMask::empty()));
        assert_eq!(reg.archetype_size(expected), 0);
        assert_eq!(reg.archetype_size(ArchetypeMask::empty()), 1);

        // Remove entity
        reg.remove_entity(1);
        assert_eq!(reg.get_archetype(1), None);
        assert_eq!(reg.archetype_size(ArchetypeMask::empty()), 0);
    }
}
