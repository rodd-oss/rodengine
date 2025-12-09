//! Sparse component storage for ECS.
//!
//! Sparse storage stores components only for entities that have them,
//! using indirect indexing via a sparse set.

use std::collections::HashMap;

/// A sparse set mapping entity IDs to component indices.
/// Entities without components are not stored.
pub struct SparseSet<T> {
    /// Dense array of components, indexed by `dense_index`.
    components: Vec<T>,
    /// Map from entity ID to index in `components`.
    entity_to_index: HashMap<u64, usize>,
    /// Map from index in `components` back to entity ID.
    index_to_entity: Vec<u64>,
}

impl<T> SparseSet<T> {
    /// Creates a new empty sparse set.
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            entity_to_index: HashMap::new(),
            index_to_entity: Vec::new(),
        }
    }

    /// Creates a sparse set with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            components: Vec::with_capacity(capacity),
            entity_to_index: HashMap::with_capacity(capacity),
            index_to_entity: Vec::with_capacity(capacity),
        }
    }

    /// Returns true if the entity has a component in this sparse set.
    pub fn contains(&self, entity_id: u64) -> bool {
        self.entity_to_index.contains_key(&entity_id)
    }

    /// Inserts a component for the given entity.
    /// If the entity already has a component, it is replaced.
    pub fn insert(&mut self, entity_id: u64, component: T) {
        if let Some(&index) = self.entity_to_index.get(&entity_id) {
            // Replace existing component
            self.components[index] = component;
        } else {
            // Add new component
            let index = self.components.len();
            self.components.push(component);
            self.index_to_entity.push(entity_id);
            self.entity_to_index.insert(entity_id, index);
        }
    }

    /// Gets a reference to the component for the given entity, if any.
    pub fn get(&self, entity_id: u64) -> Option<&T> {
        self.entity_to_index
            .get(&entity_id)
            .map(|&index| &self.components[index])
    }

    /// Gets a mutable reference to the component for the given entity, if any.
    pub fn get_mut(&mut self, entity_id: u64) -> Option<&mut T> {
        self.entity_to_index
            .get(&entity_id)
            .copied()
            .map(|index| &mut self.components[index])
    }

    /// Removes the component for the given entity.
    /// Returns the removed component if it existed.
    pub fn remove(&mut self, entity_id: u64) -> Option<T> {
        self.entity_to_index.remove(&entity_id).map(|index| {
            // Swap with last element to keep dense storage contiguous
            let last_index = self.components.len() - 1;
            if index != last_index {
                // Move last component to this slot
                self.components.swap(index, last_index);
                self.index_to_entity.swap(index, last_index);

                // Update mapping for the moved entity
                let moved_entity = self.index_to_entity[index];
                self.entity_to_index.insert(moved_entity, index);
            }

            // Remove last element (now the duplicate or the one we're removing)
            self.components.pop().unwrap()
        })
    }

    /// Returns the number of components stored.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Returns true if the sparse set is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Returns an iterator over all entity-component pairs.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        self.index_to_entity
            .iter()
            .zip(&self.components)
            .map(|(&entity, component)| (entity, component))
    }

    /// Returns a mutable iterator over all entity-component pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u64, &mut T)> {
        self.index_to_entity
            .iter()
            .zip(self.components.iter_mut())
            .map(|(&entity, component)| (entity, component))
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_set_basic() {
        let mut set = SparseSet::new();
        assert!(!set.contains(1));
        assert_eq!(set.len(), 0);

        set.insert(1, "component_a");
        assert!(set.contains(1));
        assert_eq!(set.len(), 1);
        assert_eq!(set.get(1), Some(&"component_a"));

        set.insert(2, "component_b");
        assert_eq!(set.len(), 2);
        assert_eq!(set.get(2), Some(&"component_b"));

        // Replace component
        set.insert(1, "component_a2");
        assert_eq!(set.get(1), Some(&"component_a2"));
        assert_eq!(set.len(), 2);

        // Remove component
        let removed = set.remove(1);
        assert_eq!(removed, Some("component_a2"));
        assert!(!set.contains(1));
        assert_eq!(set.len(), 1);

        // Remove non-existent
        assert!(set.remove(999).is_none());
    }

    #[test]
    fn test_sparse_set_iter() {
        let mut set = SparseSet::new();
        set.insert(1, "a");
        set.insert(2, "b");
        set.insert(3, "c");

        let mut pairs: Vec<(u64, &&str)> = set.iter().map(|(e, c)| (e, c)).collect();
        pairs.sort_by_key(|&(e, _)| e);
        assert_eq!(pairs, vec![(1, &"a"), (2, &"b"), (3, &"c")]);

        for (_, component) in set.iter_mut() {
            *component = "updated";
        }

        assert_eq!(set.get(1), Some(&"updated"));
        assert_eq!(set.get(2), Some(&"updated"));
        assert_eq!(set.get(3), Some(&"updated"));
    }
}
