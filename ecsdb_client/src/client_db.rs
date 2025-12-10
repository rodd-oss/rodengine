//! Client‑side in‑memory database that mirrors server state.

use crate::error::{ClientError, Result};
use ecsdb::component::{Component, ZeroCopyComponent};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// In‑memory component table (entity ID → serialized component data).
type ComponentTable = HashMap<u64, Vec<u8>>;

/// Client‑side database that holds a subset of server state.
pub struct ClientDB {
    /// Schema (immutable after initial sync).
    schema: Arc<ecsdb::schema::DatabaseSchema>,
    /// Component tables indexed by table ID.
    tables: Arc<RwLock<HashMap<u16, ComponentTable>>>,
    /// Entities known to this client.
    entities: Arc<RwLock<HashSet<u64>>>,
    /// Current database version (last applied delta version).
    version: Arc<RwLock<u64>>,
    /// Network client for communicating with server.
    #[allow(dead_code)]
    network_client: Option<Arc<NetworkClient>>,
}

/// Network client (placeholder).
struct NetworkClient;

impl ClientDB {
    /// Creates a new empty client database (no schema yet).
    pub fn new() -> Self {
        Self {
            schema: Arc::new(ecsdb::schema::DatabaseSchema {
                name: String::new(),
                version: String::new(),
                tables: Vec::new(),
                enums: HashMap::new(),
                custom_types: HashMap::new(),
            }),
            tables: Arc::new(RwLock::new(HashMap::new())),
            entities: Arc::new(RwLock::new(HashSet::new())),
            version: Arc::new(RwLock::new(0)),
            network_client: None,
        }
    }

    /// Connects to a remote server and performs initial sync.
    pub async fn connect(&mut self, _addr: &str) -> Result<()> {
        // TODO: establish TCP connection, authenticate, receive schema and snapshot.
        Ok(())
    }

    /// Applies a delta received from the server.
    pub async fn apply_delta(&self, delta: ecsdb::storage::delta::Delta) -> Result<()> {
        let mut tables = self.tables.write().await;
        let mut entities = self.entities.write().await;
        let mut version = self.version.write().await;

        for op in delta.ops {
            match op {
                ecsdb::storage::delta::DeltaOp::Insert {
                    table_id,
                    entity_id,
                    data,
                } => {
                    let table = tables.entry(table_id).or_insert_with(HashMap::new);
                    table.insert(entity_id, data);
                    entities.insert(entity_id);
                }
                ecsdb::storage::delta::DeltaOp::Update {
                    table_id,
                    entity_id,
                    field_offset: _,
                    old_data: _,
                    new_data,
                } => {
                    if let Some(table) = tables.get_mut(&table_id) {
                        table.insert(entity_id, new_data);
                    }
                }
                ecsdb::storage::delta::DeltaOp::Delete {
                    table_id,
                    entity_id,
                    old_data: _,
                } => {
                    if let Some(table) = tables.get_mut(&table_id) {
                        table.remove(&entity_id);
                    }
                }
                ecsdb::storage::delta::DeltaOp::CreateEntity { entity_id } => {
                    entities.insert(entity_id);
                }
                ecsdb::storage::delta::DeltaOp::DeleteEntity { entity_id } => {
                    entities.remove(&entity_id);
                    // Also remove from all tables
                    for table in tables.values_mut() {
                        table.remove(&entity_id);
                    }
                }
            }
        }

        *version = delta.version;
        Ok(())
    }

    /// Retrieves a component for an entity.
    pub async fn get<T: Component + ZeroCopyComponent>(&self, entity_id: u64) -> Result<T> {
        let table_id = T::TABLE_ID;
        let tables = self.tables.read().await;
        let table = tables
            .get(&table_id)
            .ok_or_else(|| ClientError::ComponentNotFound {
                entity_id,
                component_type: std::any::type_name::<T>().to_string(),
            })?;
        let data = table
            .get(&entity_id)
            .ok_or_else(|| ClientError::ComponentNotFound {
                entity_id,
                component_type: std::any::type_name::<T>().to_string(),
            })?;
        ecsdb::storage::field_codec::decode(data).map_err(|e| {
            ClientError::SerializationError(format!("Failed to decode component: {}", e))
        })
    }

    /// Returns whether the entity exists in the client's view.
    pub async fn contains_entity(&self, entity_id: u64) -> bool {
        let entities = self.entities.read().await;
        entities.contains(&entity_id)
    }

    /// Returns the current version (last applied delta version).
    pub async fn version(&self) -> u64 {
        *self.version.read().await
    }

    /// Returns the schema (available after initial sync).
    pub fn schema(&self) -> &Arc<ecsdb::schema::DatabaseSchema> {
        &self.schema
    }
}

impl Default for ClientDB {
    fn default() -> Self {
        Self::new()
    }
}
