use crate::component::{Component, ZeroCopyComponent};
use crate::entity::{archetype::ArchetypeRegistry, EntityId, EntityRegistry};
use crate::error::{EcsDbError, Result};
use crate::schema::{parser::SchemaParser, DatabaseSchema};
use crate::storage::delta::DeltaTracker;
use crate::storage::table::ComponentTable;
use crate::transaction::{WriteOpWithoutResponse, WriteQueue};
use dashmap::DashMap;

use std::sync::Arc;

/// Main database handle providing concurrent access to ECS data.
pub struct Database {
    /// Schema definition (immutable after creation)
    schema: Arc<DatabaseSchema>,

    /// Entity registry (protected by mutex for now)
    entity_registry: Arc<parking_lot::RwLock<EntityRegistry>>,

    /// Archetype registry (protected by mutex)
    archetype_registry: Arc<parking_lot::RwLock<ArchetypeRegistry>>,

    /// Component tables indexed by table ID
    tables: Arc<DashMap<u16, Box<dyn TableHandle + Send + Sync>>>,

    /// Write queue for lock-free write operations
    write_queue: WriteQueue,

    /// Pending write operations (batch) waiting for commit
    pending_ops: parking_lot::RwLock<Vec<WriteOpWithoutResponse>>,

    /// Current database version (incremented on each commit)
    version: Arc<std::sync::atomic::AtomicU64>,
}

/// Trait for type-erased component table operations.
pub trait TableHandle {
    /// Insert component data for an entity.
    fn insert(&mut self, entity_id: u64, data: Vec<u8>) -> Result<()>;

    /// Update component data for an entity.
    fn update(&mut self, entity_id: u64, data: Vec<u8>) -> Result<()>;

    /// Delete component for an entity.
    fn delete(&mut self, entity_id: u64) -> Result<()>;

    /// Get component data for an entity.
    fn get(&self, entity_id: u64) -> Result<Vec<u8>>;

    /// Commit pending writes to read buffer.
    fn commit(&mut self);

    /// Commit pending writes and associate the new buffer with a generation number.
    fn commit_with_generation(&mut self, generation: u64);

    /// Returns the table's record size.
    fn record_size(&self) -> usize;

    /// Returns a snapshot of the current read buffer.
    fn snapshot(&self) -> Arc<Vec<u8>>;

    /// Returns the generation number of the current read buffer.
    fn generation(&self) -> u64;

    /// Returns true if the entity has a component in this table.
    fn contains_entity(&self, entity_id: u64) -> bool;

    /// Returns the fragmentation ratio (free slots / total slots) as a value between 0.0 and 1.0.
    fn fragmentation_ratio(&self) -> f32;

    /// Returns true if fragmentation exceeds the given threshold.
    fn is_fragmented(&self, threshold: f32) -> bool;

    /// Compacts the storage buffer, moving active records to fill gaps.
    fn compact(&mut self);

    /// Returns a snapshot of the write buffer state for rollback.
    fn snapshot_write_state(&self) -> (Vec<u8>, u64, Vec<usize>, u64);

    /// Restores write buffer state from a snapshot.
    fn restore_write_state(
        &mut self,
        write_buffer: Vec<u8>,
        next_record_offset: u64,
        free_list: Vec<usize>,
        active_count: u64,
    );
}

impl Database {
    /// Creates a new database from a schema file.
    pub fn from_schema_file(path: &str) -> Result<Self> {
        let schema = SchemaParser::from_file(path)?;
        Self::from_schema(schema)
    }

    /// Creates a new database from an existing schema.
    pub fn from_schema(schema: DatabaseSchema) -> Result<Self> {
        let entity_registry = Arc::new(parking_lot::RwLock::new(EntityRegistry::new()));
        let archetype_registry = Arc::new(parking_lot::RwLock::new(ArchetypeRegistry::new()));
        let tables: Arc<DashMap<u16, Box<dyn TableHandle + Send + Sync>>> =
            Arc::new(DashMap::new());

        // Clone Arcs for capture in closure
        let tables_clone = tables.clone();
        let entity_registry_clone = entity_registry.clone();
        let archetype_registry_clone = archetype_registry.clone();

        // Create write queue with processing closure
        let write_queue = WriteQueue::spawn(move |op| {
            // Apply operation to tables and entity registry
            match op {
                WriteOpWithoutResponse::Insert {
                    table_id,
                    entity_id,
                    data,
                } => {
                    let table_id = *table_id;
                    let entity_id = *entity_id;
                    let data = data.clone();

                    // Referential integrity: ensure entity exists
                    if !entity_registry_clone
                        .read()
                        .contains_entity(crate::entity::EntityId(entity_id))
                    {
                        return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                    }

                    match tables_clone.as_ref().get_mut(&table_id) {
                        Some(mut table) => {
                            let result = table.insert(entity_id, data);
                            if result.is_ok() {
                                archetype_registry_clone
                                    .write()
                                    .add_component(entity_id, table_id);
                            }
                            result
                        }
                        None => Err(crate::error::EcsDbError::ComponentNotFound {
                            entity_id,
                            component_type: format!("table_id={}", table_id),
                        }),
                    }
                }
                WriteOpWithoutResponse::Update {
                    table_id,
                    entity_id,
                    data,
                } => {
                    let table_id = *table_id;
                    let entity_id = *entity_id;
                    let data = data.clone();

                    // Referential integrity: ensure entity exists
                    if !entity_registry_clone
                        .read()
                        .contains_entity(crate::entity::EntityId(entity_id))
                    {
                        return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                    }

                    match tables_clone.as_ref().get_mut(&table_id) {
                        Some(mut table) => table.update(entity_id, data),
                        None => Err(crate::error::EcsDbError::ComponentNotFound {
                            entity_id,
                            component_type: format!("table_id={}", table_id),
                        }),
                    }
                }
                WriteOpWithoutResponse::Delete {
                    table_id,
                    entity_id,
                } => {
                    let table_id = *table_id;
                    let entity_id = *entity_id;
                    match tables_clone.as_ref().get_mut(&table_id) {
                        Some(mut table) => {
                            let result = table.delete(entity_id);
                            if result.is_ok() {
                                archetype_registry_clone
                                    .write()
                                    .remove_component(entity_id, table_id);
                            }
                            result
                        }
                        None => Err(crate::error::EcsDbError::ComponentNotFound {
                            entity_id,
                            component_type: format!("table_id={}", table_id),
                        }),
                    }
                }
            }
        });

        Ok(Self {
            schema: Arc::new(schema),
            entity_registry,
            archetype_registry,
            tables,
            write_queue,
            pending_ops: parking_lot::RwLock::new(Vec::new()),
            version: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Registers a component type with the database.
    /// Creates a table for this component type if it doesn't exist.
    pub fn register_component<T: Component + ZeroCopyComponent>(&self) -> Result<()> {
        let table_id = T::TABLE_ID;

        if self.tables.contains_key(&table_id) {
            return Ok(()); // Already registered
        }

        // Create component table with initial capacity
        let table = ComponentTable::<T>::with_static_size(1024);

        // Wrap in type-erased handle
        let handle = Box::new(TableHandleImpl::<T> { table });

        self.tables.insert(table_id, handle);
        Ok(())
    }

    /// Creates a new entity and returns its ID.
    pub fn create_entity(&self) -> Result<EntityId> {
        let mut registry = self.entity_registry.write();
        let entity_id = registry.create_entity(0)?;
        let mut archetype_reg = self.archetype_registry.write();
        archetype_reg.add_entity(
            entity_id.0,
            crate::entity::archetype::ArchetypeMask::empty(),
        );
        Ok(entity_id)
    }

    /// Deletes an entity, enforcing referential integrity.
    /// If the entity has any components, returns an error (restrict).
    pub fn delete_entity(&self, entity_id: u64) -> Result<()> {
        // Check if any component table has a component for this entity
        for table in self.tables.iter() {
            if table.contains_entity(entity_id) {
                return Err(crate::error::EcsDbError::ReferentialIntegrityViolation(
                    format!("Entity {} still has components", entity_id),
                ));
            }
        }

        // Remove entity from archetype registry
        let mut archetype_reg = self.archetype_registry.write();
        archetype_reg.remove_entity(entity_id);
        // Delete entity from registry
        let mut registry = self.entity_registry.write();
        registry.delete_entity(crate::entity::EntityId(entity_id))
    }

    /// Inserts a component for an entity.
    /// The operation is queued and will be applied on the next commit.
    pub fn insert<T: Component + ZeroCopyComponent>(
        &self,
        entity_id: u64,
        component: &T,
    ) -> Result<()> {
        // Serialize component
        let data = crate::storage::field_codec::encode(component)?;

        // Queue write operation
        let mut queue = self.pending_ops.write();
        queue.push(WriteOpWithoutResponse::Insert {
            table_id: T::TABLE_ID,
            entity_id,
            data,
        });

        Ok(())
    }

    /// Updates a component for an entity.
    pub fn update<T: Component + ZeroCopyComponent>(
        &self,
        entity_id: u64,
        component: &T,
    ) -> Result<()> {
        let data = crate::storage::field_codec::encode(component)?;

        let mut queue = self.pending_ops.write();
        queue.push(WriteOpWithoutResponse::Update {
            table_id: T::TABLE_ID,
            entity_id,
            data,
        });

        Ok(())
    }

    /// Deletes a component for an entity.
    pub fn delete<T: Component + ZeroCopyComponent>(&self, entity_id: u64) -> Result<()> {
        let mut queue = self.pending_ops.write();
        queue.push(WriteOpWithoutResponse::Delete {
            table_id: T::TABLE_ID,
            entity_id,
        });

        Ok(())
    }

    /// Retrieves a component for an entity.
    /// Returns the deserialized component.
    pub fn get<T: Component + ZeroCopyComponent>(&self, entity_id: u64) -> Result<T> {
        let table_id = T::TABLE_ID;

        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| EcsDbError::ComponentNotFound {
                entity_id,
                component_type: std::any::type_name::<T>().to_string(),
            })?;

        let data = table.get(entity_id)?;
        crate::storage::field_codec::decode(&data)
    }

    /// Commits all pending write operations atomically.
    pub fn commit(&self) -> Result<u64> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut pending = self.pending_ops.write();
        if pending.is_empty() {
            return Ok(self.version.load(std::sync::atomic::Ordering::Acquire));
        }

        let version_before = self.version.load(std::sync::atomic::Ordering::Acquire);
        let new_version = version_before + 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let mut delta_tracker = DeltaTracker::new(new_version, timestamp);

        // Compute deltas before applying changes (read from current committed state)
        for op in pending.iter() {
            match op {
                WriteOpWithoutResponse::Insert {
                    table_id,
                    entity_id,
                    data,
                } => {
                    delta_tracker.record_insert(*table_id, *entity_id, data);
                }
                WriteOpWithoutResponse::Update {
                    table_id,
                    entity_id,
                    data,
                } => {
                    // Read current component before update (for delta)
                    if let Some(table) = self.tables.get(table_id) {
                        let old_data = table.get(*entity_id).ok();
                        if let Some(old) = old_data {
                            delta_tracker.record_update(*table_id, *entity_id, 0, &old, data);
                        } else {
                            delta_tracker.record_insert(*table_id, *entity_id, data);
                        }
                    }
                }
                WriteOpWithoutResponse::Delete {
                    table_id,
                    entity_id,
                } => {
                    // Read current component before delete (for delta)
                    if let Some(table) = self.tables.get(table_id) {
                        let old_data = table.get(*entity_id).ok();
                        if let Some(old) = old_data {
                            delta_tracker.record_delete(*table_id, *entity_id, &old);
                        }
                    }
                }
            }
        }

        // Send batch atomically via write queue
        let batch = pending.drain(..).collect();
        self.write_queue.commit_batch(new_version, batch)?;

        // Commit all tables with the new generation number (after all operations applied)
        for mut table in self.tables.iter_mut() {
            table.commit_with_generation(new_version);
        }

        // Store new version
        self.version
            .store(new_version, std::sync::atomic::Ordering::Release);

        // TODO: store or broadcast delta
        let delta = delta_tracker.take_delta();
        if !delta.is_empty() {
            // For now, just log (in production, send to replication)
            #[cfg(debug_assertions)]
            println!(
                "Delta generated for version {}: {} ops",
                new_version,
                delta.ops.len()
            );
        }

        Ok(new_version)
    }

    /// Compacts tables where fragmentation exceeds the given threshold (0.0 to 1.0).
    /// Returns the number of tables compacted.
    pub fn compact_if_fragmented(&self, threshold: f32) -> usize {
        let mut compacted = 0;
        for mut table in self.tables.iter_mut() {
            if table.is_fragmented(threshold) {
                table.compact();
                compacted += 1;
            }
        }
        compacted
    }

    /// Returns the current database version.
    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Returns the number of component tables.
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    /// Returns a reference to the database schema.
    pub fn schema(&self) -> &Arc<DatabaseSchema> {
        &self.schema
    }
}

/// Type-erased wrapper around ComponentTable<T>.
struct TableHandleImpl<T: Component + ZeroCopyComponent> {
    table: ComponentTable<T>,
}

impl<T: Component + ZeroCopyComponent> TableHandle for TableHandleImpl<T> {
    fn insert(&mut self, entity_id: u64, data: Vec<u8>) -> Result<()> {
        // Deserialize component (we need to validate size)
        let component: T = crate::storage::field_codec::decode(&data)?;
        self.table.insert(entity_id, &component)?;
        Ok(())
    }

    fn update(&mut self, entity_id: u64, data: Vec<u8>) -> Result<()> {
        let component: T = crate::storage::field_codec::decode(&data)?;
        self.table.update(entity_id, &component)?;
        Ok(())
    }

    fn delete(&mut self, entity_id: u64) -> Result<()> {
        self.table.delete(entity_id)?;
        Ok(())
    }

    fn get(&self, entity_id: u64) -> Result<Vec<u8>> {
        let component = self.table.get(entity_id)?;
        crate::storage::field_codec::encode(&component)
    }

    fn commit(&mut self) {
        self.table.commit();
    }

    fn commit_with_generation(&mut self, generation: u64) {
        self.table.commit_with_generation(generation);
    }

    fn record_size(&self) -> usize {
        self.table.record_size()
    }

    fn snapshot(&self) -> Arc<Vec<u8>> {
        self.table.snapshot()
    }

    fn generation(&self) -> u64 {
        self.table.generation()
    }

    fn contains_entity(&self, entity_id: u64) -> bool {
        self.table.contains_entity(entity_id)
    }

    fn fragmentation_ratio(&self) -> f32 {
        self.table.fragmentation_ratio()
    }

    fn is_fragmented(&self, threshold: f32) -> bool {
        self.table.is_fragmented(threshold)
    }

    fn compact(&mut self) {
        self.table.compact()
    }

    fn snapshot_write_state(&self) -> (Vec<u8>, u64, Vec<usize>, u64) {
        self.table.snapshot_write_state()
    }

    fn restore_write_state(
        &mut self,
        write_buffer: Vec<u8>,
        next_record_offset: u64,
        free_list: Vec<usize>,
        active_count: u64,
    ) {
        self.table
            .restore_write_state(write_buffer, next_record_offset, free_list, active_count)
    }
}

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

    unsafe impl ZeroCopyComponent for TestComponent {
        fn static_size() -> usize {
            std::mem::size_of::<TestComponent>()
        }

        fn alignment() -> usize {
            std::mem::align_of::<TestComponent>()
        }
    }

    #[test]
    fn test_database_basic() -> Result<()> {
        // Create in-memory database with empty schema
        let schema = DatabaseSchema {
            name: "test".to_string(),
            version: "1.0".to_string(),
            tables: Vec::new(),
            enums: std::collections::HashMap::new(),
            custom_types: std::collections::HashMap::new(),
        };

        let db = Database::from_schema(schema)?;

        // Register component type
        db.register_component::<TestComponent>()?;

        // Create entity
        let entity_id = db.create_entity()?;

        // Insert component
        let comp = TestComponent {
            x: 1.0,
            y: 2.0,
            id: 42,
        };
        db.insert(entity_id.0, &comp)?;

        // Commit to make visible
        db.commit()?;

        // Retrieve component
        let retrieved = db.get::<TestComponent>(entity_id.0)?;
        assert_eq!(retrieved, comp);

        Ok(())
    }
}
