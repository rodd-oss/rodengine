use crate::component::{Component, ZeroCopyComponent};
use crate::entity::{archetype::ArchetypeRegistry, EntityId, EntityRegistry};
use crate::error::{EcsDbError, Result};
use crate::json;
use crate::replication::ReplicationManager;
use crate::schema::{parser::SchemaParser, types::FieldDefinition, DatabaseSchema};
use crate::storage::delta::DeltaTracker;
use crate::storage::layout::{compute_record_layout, RecordLayout};
use crate::storage::table::ComponentTable;
use crate::transaction::{WriteOpWithoutResponse, WriteQueue};
use dashmap::DashMap;
use log;
use serde_json;

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

    /// Optional replication manager for multi‑client sync.
    replication_manager: Option<Arc<ReplicationManager>>,
}
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

    /// Returns mapping from entity ID to byte offset in the read buffer.
    /// Used for snapshot serialization.
    fn entity_mapping(&self) -> Vec<(u64, usize)>;

    /// Loads snapshot data into the table, replacing the current buffer and index.
    fn load_snapshot(
        &mut self,
        buffer_data: Vec<u8>,
        entity_mapping: Vec<(u64, usize)>,
        free_slots: Vec<usize>,
    ) -> Result<()>;

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

    /// Returns the name of the table.
    fn table_name(&self) -> &str;

    /// Returns the field definitions for this table.
    fn field_definitions(&self) -> &[FieldDefinition];

    /// Returns the record layout for field offset calculations.
    fn record_layout(&self) -> &RecordLayout;

    /// Validate foreign key constraints for raw component data.
    fn validate_foreign_keys(
        &self,
        entity_registry: &parking_lot::RwLock<EntityRegistry>,
        data: &[u8],
    ) -> Result<()>;
}

impl Database {
    /// Creates a new database from a schema file.
    pub fn from_schema_file(path: &str) -> Result<Self> {
        let schema = SchemaParser::from_file(path)?;
        Self::from_schema(schema)
    }

    /// Opens a database with persistence, recovering from snapshot and WAL.
    /// If no snapshot exists, returns an error (use `from_schema_file` to create new).
    pub fn open_with_persistence(config: crate::config::PersistenceConfig) -> Result<Self> {
        use crate::persistence::manager::PersistenceManager;
        let manager = PersistenceManager::new(config);
        manager.recover()
    }

    /// Enables replication with the given configuration.
    /// This starts listening for client connections and begins broadcasting deltas.
    pub async fn enable_replication(
        &mut self,
        config: crate::replication::ReplicationConfig,
    ) -> Result<()> {
        let mut manager = crate::replication::ReplicationManager::new(config);
        manager.start().await?;
        self.replication_manager = Some(std::sync::Arc::new(manager));
        Ok(())
    }

    /// Returns a reference to the replication manager, if enabled.
    pub fn replication_manager(&self) -> Option<&Arc<ReplicationManager>> {
        self.replication_manager.as_ref()
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

        // Create write queue with separate single and batch processors
        let tables_clone_single = tables_clone.clone();
        let entity_registry_clone_single = entity_registry_clone.clone();
        let archetype_registry_clone_single = archetype_registry_clone.clone();

        let process_single = move |op: &WriteOpWithoutResponse| -> Result<()> {
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
                    if !entity_registry_clone_single
                        .read()
                        .contains_entity(crate::entity::EntityId(entity_id))
                    {
                        return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                    }

                    // Validate foreign key constraints
                    match tables_clone_single.as_ref().get(&table_id) {
                        Some(table) => {
                            table.validate_foreign_keys(&entity_registry_clone_single, &data)?;
                        }
                        None => {
                            return Err(crate::error::EcsDbError::ComponentNotFound {
                                entity_id,
                                component_type: format!("table_id={}", table_id),
                            })
                        }
                    }

                    match tables_clone_single.as_ref().get_mut(&table_id) {
                        Some(mut table) => {
                            let result = table.insert(entity_id, data);
                            if result.is_ok() {
                                archetype_registry_clone_single
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
                    if !entity_registry_clone_single
                        .read()
                        .contains_entity(crate::entity::EntityId(entity_id))
                    {
                        return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                    }

                    // Validate foreign key constraints
                    match tables_clone_single.as_ref().get(&table_id) {
                        Some(table) => {
                            table.validate_foreign_keys(&entity_registry_clone_single, &data)?;
                        }
                        None => {
                            return Err(crate::error::EcsDbError::ComponentNotFound {
                                entity_id,
                                component_type: format!("table_id={}", table_id),
                            })
                        }
                    }

                    match tables_clone_single.as_ref().get_mut(&table_id) {
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
                    match tables_clone_single.as_ref().get_mut(&table_id) {
                        Some(mut table) => {
                            let result = table.delete(entity_id);
                            if result.is_ok() {
                                archetype_registry_clone_single
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
        };

        // Batch processor that ensures atomic rollback
        let process_batch = move |ops: &[WriteOpWithoutResponse]| -> Result<()> {
            use std::collections::HashMap;

            // Determine which tables are affected
            let mut affected_table_ids = Vec::new();
            for op in ops {
                let table_id = match op {
                    WriteOpWithoutResponse::Insert { table_id, .. } => *table_id,
                    WriteOpWithoutResponse::Update { table_id, .. } => *table_id,
                    WriteOpWithoutResponse::Delete { table_id, .. } => *table_id,
                };
                if !affected_table_ids.contains(&table_id) {
                    affected_table_ids.push(table_id);
                }
            }

            // Snapshot write buffer state for affected tables
            let mut table_snapshots = HashMap::new();
            for &table_id in &affected_table_ids {
                if let Some(table) = tables_clone.as_ref().get(&table_id) {
                    let snapshot = table.snapshot_write_state();
                    table_snapshots.insert(table_id, snapshot);
                }
            }

            // Snapshot archetype registry (clone)
            let archetype_snapshot = archetype_registry_clone.read().clone();

            // Helper to apply a single operation (using captured clones)
            let apply_op = |op: &WriteOpWithoutResponse| -> Result<()> {
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

                        // Validate foreign key constraints
                        match tables_clone.as_ref().get(&table_id) {
                            Some(table) => {
                                table.validate_foreign_keys(&entity_registry_clone, &data)?;
                            }
                            None => {
                                return Err(crate::error::EcsDbError::ComponentNotFound {
                                    entity_id,
                                    component_type: format!("table_id={}", table_id),
                                })
                            }
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

                        // Validate foreign key constraints
                        match tables_clone.as_ref().get(&table_id) {
                            Some(table) => {
                                table.validate_foreign_keys(&entity_registry_clone, &data)?;
                            }
                            None => {
                                return Err(crate::error::EcsDbError::ComponentNotFound {
                                    entity_id,
                                    component_type: format!("table_id={}", table_id),
                                })
                            }
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
            };

            // Apply each operation, collecting errors
            for op in ops.iter() {
                if let Err(err) = apply_op(op) {
                    // Rollback: restore table snapshots
                    for (&table_id, snapshot) in &table_snapshots {
                        if let Some(mut table) = tables_clone.as_ref().get_mut(&table_id) {
                            table.restore_write_state(
                                snapshot.0.clone(),
                                snapshot.1,
                                snapshot.2.clone(),
                                snapshot.3,
                            );
                        }
                    }
                    // Rollback archetype registry
                    *archetype_registry_clone.write() = archetype_snapshot.clone();
                    return Err(err);
                }
            }

            // All operations succeeded
            Ok(())
        };

        let write_queue = WriteQueue::spawn_with_batch(process_single, process_batch);

        Ok(Self {
            schema: Arc::new(schema),
            entity_registry,
            archetype_registry,
            tables,
            write_queue,
            pending_ops: parking_lot::RwLock::new(Vec::new()),
            version: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            replication_manager: None,
        })
    }

    /// Applies a write operation directly (bypassing write queue).
    /// Used for WAL replay during recovery.
    pub(crate) fn apply_write_op(&self, op: &WriteOpWithoutResponse) -> Result<()> {
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
                if !self
                    .entity_registry
                    .read()
                    .contains_entity(crate::entity::EntityId(entity_id))
                {
                    return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                }

                // Validate foreign key constraints
                match self.tables.as_ref().get(&table_id) {
                    Some(table) => {
                        table.validate_foreign_keys(&self.entity_registry, &data)?;
                    }
                    None => {
                        return Err(crate::error::EcsDbError::ComponentNotFound {
                            entity_id,
                            component_type: format!("table_id={}", table_id),
                        })
                    }
                }

                match self.tables.as_ref().get_mut(&table_id) {
                    Some(mut table) => {
                        let result = table.insert(entity_id, data);
                        if result.is_ok() {
                            self.archetype_registry
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
                if !self
                    .entity_registry
                    .read()
                    .contains_entity(crate::entity::EntityId(entity_id))
                {
                    return Err(crate::error::EcsDbError::EntityNotFound(entity_id));
                }

                // Validate foreign key constraints
                match self.tables.as_ref().get(&table_id) {
                    Some(table) => {
                        table.validate_foreign_keys(&self.entity_registry, &data)?;
                    }
                    None => {
                        return Err(crate::error::EcsDbError::ComponentNotFound {
                            entity_id,
                            component_type: format!("table_id={}", table_id),
                        })
                    }
                }

                match self.tables.as_ref().get_mut(&table_id) {
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
                match self.tables.as_ref().get_mut(&table_id) {
                    Some(mut table) => {
                        let result = table.delete(entity_id);
                        if result.is_ok() {
                            self.archetype_registry
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
    }

    /// Registers a component type with the database.
    /// Creates a table for this component type if it doesn't exist.
    pub fn register_component<T: Component + ZeroCopyComponent>(&self) -> Result<()> {
        let table_id = T::TABLE_ID;

        if self.tables.contains_key(&table_id) {
            return Ok(()); // Already registered
        }

        // Look up table definition from schema
        let table_def = self.schema.find_table(T::TABLE_NAME).ok_or_else(|| {
            EcsDbError::SchemaError(format!("Table '{}' not found in schema", T::TABLE_NAME))
        })?;

        // Compute record layout for field offset calculations
        let record_layout = compute_record_layout(&table_def.fields, &self.schema.custom_types)?;

        // Create component table with initial capacity
        let table = ComponentTable::<T>::with_static_size(1024);

        // Wrap in type-erased handle
        let handle = Box::new(TableHandleImpl::<T> {
            table,
            table_name: table_def.name.clone(),
            field_definitions: table_def.fields.clone(),
            record_layout,
        });

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

        // Broadcast delta to replication clients (if enabled)
        let delta = delta_tracker.take_delta();
        if !delta.is_empty() {
            #[cfg(debug_assertions)]
            println!(
                "Delta generated for version {}: {} ops",
                new_version,
                delta.ops.len()
            );
            if let Some(rm) = &self.replication_manager {
                let delta_clone = delta.clone();
                let rm = rm.clone();
                tokio::spawn(async move {
                    if let Err(e) = rm.broadcast_delta(delta_clone).await {
                        log::error!("Failed to broadcast delta: {}", e);
                    }
                });
            }
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

    /// Sets the database version (used during recovery).
    pub(crate) fn set_version(&self, new_version: u64) {
        self.version
            .store(new_version, std::sync::atomic::Ordering::Release);
    }

    /// Returns the number of component tables.
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    /// Returns the table ID for a given table name, if it exists.
    pub fn get_table_id_by_name(&self, table_name: &str) -> Option<u16> {
        for entry in self.tables.iter() {
            if entry.value().table_name() == table_name {
                return Some(*entry.key());
            }
        }
        None
    }

    /// Returns the number of entities that have a component in the given table.
    pub fn get_entity_count_for_table(&self, table_id: u16) -> usize {
        if let Some(table) = self.tables.get(&table_id) {
            table.entity_mapping().len()
        } else {
            0
        }
    }

    /// Returns a list of entity IDs and their component data for a given table, with pagination.
    /// Returns (entity_id, serialized component data) pairs.
    pub fn get_entities_for_table(
        &self,
        table_id: u16,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(u64, Vec<u8>)>> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| EcsDbError::SchemaError(format!("Table {} not found", table_id)))?;

        let mapping = table.entity_mapping();
        let total = mapping.len();
        let start = offset.min(total);
        let end = (offset + limit).min(total);

        let mut results = Vec::with_capacity(end - start);
        for &(entity_id, _) in &mapping[start..end] {
            let data = table.get(entity_id)?;
            results.push((entity_id, data));
        }
        Ok(results)
    }

    /// Returns a list of entity IDs and their component data as JSON for a given table, with pagination.
    /// Returns (entity_id, JSON value) pairs.
    pub fn get_entities_json_for_table(
        &self,
        table_name: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(u64, serde_json::Value)>> {
        let table_id = self
            .get_table_id_by_name(table_name)
            .ok_or_else(|| EcsDbError::SchemaError(format!("Table '{}' not found", table_name)))?;
        let table_def = self.schema.find_table(table_name).ok_or_else(|| {
            EcsDbError::SchemaError(format!("Table '{}' not found in schema", table_name))
        })?;

        // Compute layout for this table
        let layout = compute_record_layout(&table_def.fields, &self.schema.custom_types)?;

        // Get raw entity data
        let raw_data = self.get_entities_for_table(table_id, limit, offset)?;

        // Convert each component to JSON
        let mut results = Vec::with_capacity(raw_data.len());
        for (entity_id, bytes) in raw_data {
            let json = json::component_bytes_to_json_with_layout(
                &bytes,
                &table_def.fields,
                &layout,
                &self.schema.custom_types,
            )?;
            results.push((entity_id, json));
        }
        Ok(results)
    }

    /// Insert component data from JSON for a given entity.
    pub fn insert_from_json(
        &self,
        table_name: &str,
        entity_id: u64,
        json: serde_json::Value,
    ) -> Result<()> {
        let table_id = self
            .get_table_id_by_name(table_name)
            .ok_or_else(|| EcsDbError::SchemaError(format!("Table '{}' not found", table_name)))?;
        let table_def = self.schema.find_table(table_name).ok_or_else(|| {
            EcsDbError::SchemaError(format!("Table '{}' not found in schema", table_name))
        })?;

        // Compute layout for this table
        let layout = compute_record_layout(&table_def.fields, &self.schema.custom_types)?;

        // Convert JSON to bytes
        let bytes = json::json_to_component_bytes_with_layout(
            &json,
            &table_def.fields,
            &layout,
            &self.schema.custom_types,
        )?;

        // Insert via write queue
        self.write_queue.insert(table_id, entity_id, bytes)?;
        Ok(())
    }

    /// Update component data from JSON for a given entity.
    pub fn update_from_json(
        &self,
        table_name: &str,
        entity_id: u64,
        json: serde_json::Value,
    ) -> Result<()> {
        let table_id = self
            .get_table_id_by_name(table_name)
            .ok_or_else(|| EcsDbError::SchemaError(format!("Table '{}' not found", table_name)))?;
        let table_def = self.schema.find_table(table_name).ok_or_else(|| {
            EcsDbError::SchemaError(format!("Table '{}' not found in schema", table_name))
        })?;

        let layout = compute_record_layout(&table_def.fields, &self.schema.custom_types)?;

        let bytes = json::json_to_component_bytes_with_layout(
            &json,
            &table_def.fields,
            &layout,
            &self.schema.custom_types,
        )?;

        self.write_queue.update(table_id, entity_id, bytes)?;
        Ok(())
    }

    /// Delete component for a given entity and table.
    pub fn delete_by_table(&self, table_name: &str, entity_id: u64) -> Result<()> {
        let table_id = self
            .get_table_id_by_name(table_name)
            .ok_or_else(|| EcsDbError::SchemaError(format!("Table '{}' not found", table_name)))?;
        self.write_queue.delete(table_id, entity_id)?;
        Ok(())
    }

    /// Returns a reference to the database schema.
    pub fn schema(&self) -> &Arc<DatabaseSchema> {
        &self.schema
    }

    /// Creates a snapshot of the current database state.
    /// The snapshot captures schema, entity registry, archetype registry, and all component tables.
    /// This should be called after a commit to ensure consistency.
    pub fn create_snapshot(
        &self,
    ) -> crate::error::Result<crate::persistence::snapshot::DatabaseSnapshot> {
        use crate::persistence::snapshot::{DatabaseSnapshot, TableSnapshot};
        use std::collections::HashSet;
        let schema = self.schema.as_ref().clone();
        let entity_registry = self.entity_registry.read().clone();
        let archetype_registry = self.archetype_registry.read().clone();
        let mut tables = Vec::new();
        for entry in self.tables.iter() {
            let table_id = *entry.key();
            let table = entry.value();
            let table_name = table.table_name().to_string();
            let record_size = table.record_size();
            let buffer_data = table.snapshot().as_ref().clone();
            let entity_mapping = table.entity_mapping();
            // Compute free slots: slots not referenced in entity_mapping
            let occupied_offsets: HashSet<usize> =
                entity_mapping.iter().map(|&(_, offset)| offset).collect();
            let total_slots = buffer_data.len() / record_size;
            let mut free_slots = Vec::new();
            for slot_index in 0..total_slots {
                let offset = slot_index * record_size;
                if !occupied_offsets.contains(&offset) {
                    free_slots.push(offset);
                }
            }
            let active_count = entity_mapping.len();
            tables.push(TableSnapshot {
                table_id,
                table_name,
                record_size,
                buffer_data,
                entity_mapping,
                free_slots,
                active_count,
            });
        }
        let version = self.version.load(std::sync::atomic::Ordering::SeqCst);
        Ok(DatabaseSnapshot {
            schema,
            entity_registry,
            archetype_registry,
            tables,
            version,
        })
    }

    /// Creates a new database from a snapshot.
    pub fn from_snapshot(snapshot: crate::persistence::snapshot::DatabaseSnapshot) -> Result<Self> {
        // Create empty database from schema
        let db = Self::from_schema(snapshot.schema)?;
        // Load each table snapshot
        for table_snapshot in snapshot.tables {
            let table_id = table_snapshot.table_id;
            // Find the table in the database (should exist due to schema)
            if let Some(mut table) = db.tables.as_ref().get_mut(&table_id) {
                table.load_snapshot(
                    table_snapshot.buffer_data,
                    table_snapshot.entity_mapping,
                    table_snapshot.free_slots,
                )?;
            } else {
                return Err(crate::error::EcsDbError::SnapshotError(format!(
                    "Table {} not found in database",
                    table_snapshot.table_name
                )));
            }
        }
        // Replace entity and archetype registries
        *db.entity_registry.write() = snapshot.entity_registry;
        *db.archetype_registry.write() = snapshot.archetype_registry;
        // Set database version to snapshot version
        db.version
            .store(snapshot.version, std::sync::atomic::Ordering::SeqCst);
        Ok(db)
    }
}
/// Type-erased wrapper around ComponentTable<T>.
struct TableHandleImpl<T: Component + ZeroCopyComponent> {
    table: ComponentTable<T>,
    table_name: String,
    field_definitions: Vec<FieldDefinition>,
    record_layout: RecordLayout,
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

    fn entity_mapping(&self) -> Vec<(u64, usize)> {
        self.table.entity_mapping()
    }

    fn load_snapshot(
        &mut self,
        buffer_data: Vec<u8>,
        entity_mapping: Vec<(u64, usize)>,
        free_slots: Vec<usize>,
    ) -> Result<()> {
        self.table
            .load_snapshot(buffer_data, entity_mapping, free_slots)
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

    fn table_name(&self) -> &str {
        &self.table_name
    }

    fn field_definitions(&self) -> &[FieldDefinition] {
        &self.field_definitions
    }

    fn record_layout(&self) -> &RecordLayout {
        &self.record_layout
    }

    fn validate_foreign_keys(
        &self,
        entity_registry: &parking_lot::RwLock<EntityRegistry>,
        data: &[u8],
    ) -> Result<()> {
        for (field_def, field_layout) in self
            .field_definitions
            .iter()
            .zip(&self.record_layout.fields)
        {
            if let Some(_fk) = &field_def.foreign_key {
                // For now, assume foreign key references entities table and field is u64 entity ID
                // Extract u64 from data at field offset
                let offset = field_layout.offset;
                if offset + 8 > data.len() {
                    return Err(EcsDbError::SchemaError(
                        "Data too short for foreign key field".into(),
                    ));
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[offset..offset + 8]);
                let referenced_entity_id = u64::from_le_bytes(bytes);
                // Check entity exists
                if !entity_registry
                    .read()
                    .contains_entity(crate::entity::EntityId(referenced_entity_id))
                {
                    return Err(EcsDbError::ReferentialIntegrityViolation(format!(
                        "Foreign key references non-existent entity {}",
                        referenced_entity_id
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{FieldType, TableDefinition};
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
        // Create in-memory database with schema containing test_component table
        let schema = DatabaseSchema {
            name: "test".to_string(),
            version: "1.0".to_string(),
            tables: vec![TableDefinition {
                name: "test_component".to_string(),
                fields: vec![
                    FieldDefinition {
                        name: "x".to_string(),
                        field_type: FieldType::F32,
                        nullable: false,
                        indexed: false,
                        primary_key: false,
                        foreign_key: None,
                    },
                    FieldDefinition {
                        name: "y".to_string(),
                        field_type: FieldType::F32,
                        nullable: false,
                        indexed: false,
                        primary_key: false,
                        foreign_key: None,
                    },
                    FieldDefinition {
                        name: "id".to_string(),
                        field_type: FieldType::U32,
                        nullable: false,
                        indexed: false,
                        primary_key: false,
                        foreign_key: None,
                    },
                ],
                parent_table: None,
                description: None,
            }],
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

        // Test JSON conversion
        let json_data = db.get_entities_json_for_table("test_component", 10, 0)?;
        assert_eq!(json_data.len(), 1);
        let (retrieved_entity_id, json_value) = &json_data[0];
        assert_eq!(*retrieved_entity_id, entity_id.0);
        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert_eq!(obj["x"].as_f64().unwrap(), 1.0);
        assert_eq!(obj["y"].as_f64().unwrap(), 2.0);
        assert_eq!(obj["id"].as_u64().unwrap(), 42);

        Ok(())
    }
}
