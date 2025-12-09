# ECS Database - Practical Implementation Guide

## 1. Getting Started

### 1.1 Project Setup

```bash
cargo new ecsdb --lib
cd ecsdb

# Add dependencies to Cargo.toml
cargo add tokio --features "full"
cargo add serde --features "derive"
cargo add serde_json
cargo add toml
cargo add bincode
cargo add thiserror
cargo add dashmap
cargo add parking_lot
```

### 1.2 Cargo.toml Structure

```toml
[package]
name = "ecsdb"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
bincode = "1"
thiserror = "1"
dashmap = "5"
parking_lot = "0.12"
uuid = { version = "1", features = ["v4"] }
zstd = "0.13"  # Compression
parking_lot = "0.12"
bytes = "1"

[dev-dependencies]
criterion = "0.5"
proptest = "1"
tokio-test = "0.4"

[[bench]]
name = "inserts"
harness = false
```

---

## 2. Core Module Implementations

### 2.1 Error Handling (`src/error.rs`)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EcsDbError {
    #[error("Entity not found: {0}")]
    EntityNotFound(u64),
    
    #[error("Component not found for entity {entity_id}: {component_type}")]
    ComponentNotFound { entity_id: u64, component_type: String },
    
    #[error("Schema validation failed: {0}")]
    SchemaError(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    
    #[error("Field type mismatch: expected {expected}, got {got}")]
    FieldTypeMismatch { expected: String, got: String },
    
    #[error("Field alignment error at offset {offset}")]
    AlignmentError { offset: usize },
    
    #[error("Write channel closed")]
    ChannelClosed,
    
    #[error("Timeout waiting for write confirmation")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, EcsDbError>;
```

### 2.2 Type System (`src/schema/types.rs`)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FieldType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Array {
        element_type: Box<FieldType>,
        length: usize,
    },
    Enum(String),  // References enum definition
    Struct(String), // References custom type
    Custom(String), // User-defined type
}

impl FieldType {
    /// Returns the size in bytes for this type
    pub fn size_bytes(&self) -> Result<usize> {
        match self {
            FieldType::U8 => Ok(1),
            FieldType::U16 => Ok(2),
            FieldType::U32 => Ok(4),
            FieldType::U64 => Ok(8),
            FieldType::I8 => Ok(1),
            FieldType::I16 => Ok(2),
            FieldType::I32 => Ok(4),
            FieldType::I64 => Ok(8),
            FieldType::F32 => Ok(4),
            FieldType::F64 => Ok(8),
            FieldType::Bool => Ok(1),
            FieldType::Array { element_type, length } => {
                let elem_size = element_type.size_bytes()?;
                Ok(elem_size * length)
            }
            FieldType::Enum(_) => Ok(4), // u32 discriminant
            FieldType::Struct(_) | FieldType::Custom(_) => {
                Err(EcsDbError::SchemaError(
                    "Custom types must be resolved before size calculation".into()
                ))
            }
        }
    }
    
    /// Returns the alignment requirement in bytes
    pub fn alignment(&self) -> usize {
        match self {
            FieldType::U8 | FieldType::I8 | FieldType::Bool => 1,
            FieldType::U16 | FieldType::I16 => 2,
            FieldType::U32 | FieldType::I32 | FieldType::F32 | FieldType::Enum(_) => 4,
            FieldType::U64 | FieldType::I64 | FieldType::F64 => 8,
            FieldType::Array { element_type, .. } => element_type.alignment(),
            _ => 8, // Conservative default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub indexed: bool,
    pub primary_key: bool,
    pub foreign_key: Option<String>, // References "table.field"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDefinition {
    pub name: String,
    pub fields: Vec<FieldDefinition>,
    pub parent_table: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub name: String,
    pub version: String,
    pub tables: Vec<TableDefinition>,
    pub enums: std::collections::HashMap<String, Vec<String>>,
    pub custom_types: std::collections::HashMap<String, Vec<FieldDefinition>>,
}
```

### 2.3 Schema Parser (`src/schema/parser.rs`)

```rust
use super::types::*;
use crate::error::Result;
use std::fs;

pub struct SchemaParser;

impl SchemaParser {
    pub fn from_file(path: &str) -> Result<DatabaseSchema> {
        let content = fs::read_to_string(path)?;
        Self::from_string(&content)
    }
    
    pub fn from_string(toml_str: &str) -> Result<DatabaseSchema> {
        let schema: toml::Value = toml::from_str(toml_str)
            .map_err(|e| EcsDbError::SchemaError(format!("TOML parse error: {}", e)))?;
        
        let database = schema.get("database")
            .ok_or_else(|| EcsDbError::SchemaError(
                "Missing [database] section".into()
            ))?;
        
        let name = database.get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| EcsDbError::SchemaError(
                "Missing database.name".into()
            ))?;
        
        let version = database.get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "1.0.0".to_string());
        
        // Parse custom types
        let mut custom_types = std::collections::HashMap::new();
        if let Some(types) = schema.get("custom_types") {
            for (type_name, fields) in types.as_table().unwrap_or(&Default::default()) {
                let field_defs = Self::parse_field_list(fields)?;
                custom_types.insert(type_name.clone(), field_defs);
            }
        }
        
        // Parse enums
        let mut enums = std::collections::HashMap::new();
        if let Some(enum_defs) = schema.get("enums") {
            for (enum_name, variants) in enum_defs.as_table().unwrap_or(&Default::default()) {
                if let Some(vars) = variants.get("variants").and_then(|v| v.as_array()) {
                    let variant_names = vars.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    enums.insert(enum_name.clone(), variant_names);
                }
            }
        }
        
        // Parse tables
        let mut tables = Vec::new();
        if let Some(table_defs) = schema.get("tables") {
            for (table_name, table_config) in table_defs.as_table().unwrap_or(&Default::default()) {
                let fields = Self::parse_field_list(table_config)?;
                
                let parent_table = table_config.get("parent_table")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                let description = table_config.get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                tables.push(TableDefinition {
                    name: table_name.clone(),
                    fields,
                    parent_table,
                    description,
                });
            }
        }
        
        Ok(DatabaseSchema {
            name,
            version,
            tables,
            enums,
            custom_types,
        })
    }
    
    fn parse_field_list(config: &toml::Value) -> Result<Vec<FieldDefinition>> {
        let mut fields = Vec::new();
        
        if let Some(field_array) = config.get("fields").and_then(|v| v.as_array()) {
            for field_val in field_array {
                let name = field_val.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| EcsDbError::SchemaError(
                        "Field missing 'name'".into()
                    ))?;
                
                let type_str = field_val.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EcsDbError::SchemaError(
                        "Field missing 'type'".into()
                    ))?;
                
                let field_type = Self::parse_type(type_str)?;
                
                let nullable = field_val.get("nullable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let indexed = field_val.get("indexed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let primary_key = field_val.get("primary_key")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let foreign_key = field_val.get("foreign_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                fields.push(FieldDefinition {
                    name,
                    field_type,
                    nullable,
                    indexed,
                    primary_key,
                    foreign_key,
                });
            }
        }
        
        Ok(fields)
    }
    
    fn parse_type(type_str: &str) -> Result<FieldType> {
        match type_str {
            "u8" => Ok(FieldType::U8),
            "u16" => Ok(FieldType::U16),
            "u32" => Ok(FieldType::U32),
            "u64" => Ok(FieldType::U64),
            "i8" => Ok(FieldType::I8),
            "i16" => Ok(FieldType::I16),
            "i32" => Ok(FieldType::I32),
            "i64" => Ok(FieldType::I64),
            "f32" => Ok(FieldType::F32),
            "f64" => Ok(FieldType::F64),
            "bool" => Ok(FieldType::Bool),
            s if s.starts_with('[') && s.ends_with(']') => {
                // Parse array: [T; N]
                let inner = &s[1..s.len()-1];
                let parts: Vec<&str> = inner.split(';').map(|p| p.trim()).collect();
                if parts.len() != 2 {
                    return Err(EcsDbError::SchemaError(
                        format!("Invalid array type syntax: {}", type_str)
                    ));
                }
                let element_type = Box::new(Self::parse_type(parts[0])?);
                let length = parts[1].parse()
                    .map_err(|_| EcsDbError::SchemaError(
                        format!("Invalid array length: {}", parts[1])
                    ))?;
                Ok(FieldType::Array { element_type, length })
            }
            s => Ok(FieldType::Custom(s.to_string())),
        }
    }
}
```

### 2.4 Storage Buffer (`src/storage/buffer.rs`)

```rust
use crate::error::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

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
            return Err(crate::error::EcsDbError::SchemaError(
                format!("Record size mismatch: expected {}, got {}", 
                    self.record_size, record.len())
            ));
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
            return Err(crate::error::EcsDbError::SchemaError(
                "Offset out of bounds".into()
            ));
        }
        
        let end = offset + record.len();
        self.write_buffer[offset..end].copy_from_slice(record);
        Ok(())
    }
    
    /// Get read-only access to a record from read buffer
    pub fn read(&self, offset: usize, size: usize) -> Result<Vec<u8>> {
        let read_buf = unsafe { &*self.read_buffer.load(Ordering::Acquire) };
        
        if offset + size > read_buf.len() {
            return Err(crate::error::EcsDbError::SchemaError(
                "Offset out of bounds".into()
            ));
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
pub struct ArcStorageBuffer {
    read_buffer: Arc<AtomicPtr<Arc<Vec<u8>>>>,
    write_buffer: Vec<u8>,
    record_count: u64,
    record_size: usize,
}

impl ArcStorageBuffer {
    pub fn new(record_size: usize, initial_capacity: usize) -> Self {
        let initial = Arc::new(vec![0u8; initial_capacity]);
        let ptr = Box::leak(Box::new(initial)) as *mut Arc<Vec<u8>>;
        
        Self {
            read_buffer: Arc::new(AtomicPtr::new(ptr)),
            write_buffer: vec![0u8; initial_capacity],
            record_count: 0,
            record_size,
        }
    }
    
    pub fn commit(&mut self) {
        let new_arc = Arc::new(self.write_buffer.clone());
        let new_ptr = Box::leak(Box::new(new_arc)) as *mut Arc<Vec<u8>>;
        
        // Atomic swap
        let old_ptr = self.read_buffer.swap(new_ptr, Ordering::Release);
        
        // Safe deallocation
        let _ = unsafe { Box::from_raw(old_ptr) };
    }
}
```

### 2.5 Entity Registry (`src/entity/registry.rs`)

```rust
use crate::error::Result;
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
        let offset = self.index.remove(&entity_id)
            .ok_or_else(|| crate::error::EcsDbError::EntityNotFound(entity_id.0))?;
        
        // Mark as deleted, bump version
        if let Some(record) = self.records.get_mut(offset) {
            record.version = EntityVersion(record.version.0 + 1);
            self.freelist.push((entity_id, record.version));
        }
        
        Ok(())
    }
    
    pub fn get_entity(&self, entity_id: EntityId) -> Result<EntityRecord> {
        let offset = self.index.get(&entity_id)
            .ok_or_else(|| crate::error::EcsDbError::EntityNotFound(entity_id.0))?;
        
        Ok(self.records[*offset].clone())
    }
    
    pub fn entity_count(&self) -> usize {
        self.records.len()
    }
}
```

### 2.6 Transaction Engine (`src/transaction/engine.rs`)

```rust
use crate::error::Result;
use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum TransactionOp {
    Insert {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
    },
    Update {
        table_id: u16,
        entity_id: u64,
        field_offset: usize,
        data: Vec<u8>,
    },
    Delete {
        table_id: u16,
        entity_id: u64,
    },
}

pub struct Transaction {
    operations: Vec<TransactionOp>,
    response_tx: mpsc::Sender<Result<u64>>, // Returns version on commit
}

impl Transaction {
    pub fn new(response_tx: mpsc::Sender<Result<u64>>) -> Self {
        Self {
            operations: Vec::new(),
            response_tx,
        }
    }
    
    pub fn insert(&mut self, table_id: u16, entity_id: u64, data: Vec<u8>) {
        self.operations.push(TransactionOp::Insert {
            table_id,
            entity_id,
            data,
        });
    }
    
    pub fn update(&mut self, table_id: u16, entity_id: u64, 
                  field_offset: usize, data: Vec<u8>) {
        self.operations.push(TransactionOp::Update {
            table_id,
            entity_id,
            field_offset,
            data,
        });
    }
    
    pub fn delete(&mut self, table_id: u16, entity_id: u64) {
        self.operations.push(TransactionOp::Delete {
            table_id,
            entity_id,
        });
    }
    
    pub fn commit(self) -> Result<u64> {
        // Send to write thread
        // (Would be implemented with MPSC channel in real code)
        Ok(0)
    }
}

pub struct TransactionEngine {
    wal: Vec<TransactionOp>, // Write-ahead log
    version: u64,
}

impl TransactionEngine {
    pub fn new() -> Self {
        Self {
            wal: Vec::new(),
            version: 0,
        }
    }
    
    pub fn process_transaction(&mut self, txn: Transaction) -> Result<u64> {
        // Log operations
        for op in txn.operations {
            self.wal.push(op);
        }
        
        // Bump version
        self.version += 1;
        
        Ok(self.version)
    }
}
```

---

## 3. API Design Examples

### 3.1 Public Database API

```rust
pub struct Database {
    // ... internal fields
}

impl Database {
    pub fn new() -> DatabaseBuilder {
        DatabaseBuilder::default()
    }
    
    pub async fn insert<T: Component>(
        &self,
        entity_id: u64,
        component: T,
    ) -> Result<()> {
        // Serialize component
        let data = bincode::serialize(&component)?;
        
        // Queue write
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.write_queue.send(WriteOp::Insert {
            table_id: T::TABLE_ID,
            entity_id,
            data,
            response: tx,
        })?;
        
        // Wait for write thread response
        rx.await.ok()?
    }
    
    pub fn read<T: Component, F, R>(&self, entity_id: u64, f: F) -> Result<R>
    where
        F: FnOnce(&T) -> Result<R>,
    {
        let table = self.get_table::<T>()?;
        let data = table.get(entity_id)?;
        let component: T = bincode::deserialize(&data)?;
        f(&component)
    }
    
    pub fn transaction(&self) -> TransactionBuilder {
        TransactionBuilder::new(self.clone())
    }
}

pub struct TransactionBuilder {
    db: Arc<Database>,
    operations: Vec<TransactionOp>,
}

impl TransactionBuilder {
    pub fn insert<T: Component>(mut self, entity_id: u64, component: T) -> Result<Self> {
        let data = bincode::serialize(&component)?;
        self.operations.push(TransactionOp::Insert {
            table_id: T::TABLE_ID,
            entity_id,
            data,
        });
        Ok(self)
    }
    
    pub async fn commit(self) -> Result<u64> {
        // Submit all operations atomically
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.db.write_queue.send(WriteOp::Commit {
            operations: self.operations,
            response: tx,
        })?;
        
        rx.await.ok()?
    }
}

pub trait Component: serde::Serialize + serde::de::DeserializeOwned {
    const TABLE_ID: u16;
    const TABLE_NAME: &'static str;
}

// User-defined component
#[derive(Serialize, Deserialize, Clone)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

impl Component for Transform {
    const TABLE_ID: u16 = 0;
    const TABLE_NAME: &'static str = "transform";
}
```

---

## 4. Complete Minimal Example

```rust
// src/lib.rs

pub mod error;
pub mod schema;
pub mod storage;
pub mod entity;
pub mod transaction;

use error::Result;
use std::sync::Arc;

pub struct Database {
    schema: Arc<schema::DatabaseSchema>,
    entity_registry: entity::EntityRegistry,
    write_queue: tokio::sync::mpsc::UnboundedSender<TransactionOp>,
}

#[derive(Clone)]
pub enum TransactionOp {
    Insert { table: String, entity_id: u64, data: Vec<u8> },
    Update { table: String, entity_id: u64, data: Vec<u8> },
    Delete { table: String, entity_id: u64 },
}

pub struct DatabaseBuilder {
    schema_path: Option<String>,
}

impl DatabaseBuilder {
    pub fn schema_file(mut self, path: &str) -> Self {
        self.schema_path = Some(path.to_string());
        self
    }
    
    pub async fn build(self) -> Result<Database> {
        let schema = if let Some(path) = self.schema_path {
            schema::parser::SchemaParser::from_file(&path)?
        } else {
            return Err(error::EcsDbError::SchemaError(
                "Schema file required".into()
            ));
        };
        
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        
        Ok(Database {
            schema: Arc::new(schema),
            entity_registry: entity::EntityRegistry::new(),
            write_queue: tx,
        })
    }
}

impl Database {
    pub fn builder() -> DatabaseBuilder {
        DatabaseBuilder {
            schema_path: None,
        }
    }
}

// tests/integration.rs

#[tokio::test]
async fn test_basic_create() -> Result<()> {
    let db = Database::builder()
        .schema_file("tests/fixtures/simple_schema.toml")
        .build()
        .await?;
    
    // Test that database initialized correctly
    assert!(db.schema.tables.len() > 0);
    
    Ok(())
}
```

---

## 5. Performance Benchmarking Template

```rust
// benches/inserts.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ecsdb::Database;

fn bench_single_insert(c: &mut Criterion) {
    c.bench_function("insert_single_record", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let db = Database::builder()
                .schema_file("schema.toml")
                .build()
                .await
                .unwrap();
            
            // Insert a single record
            let data = black_box(vec![0u8; 64]);
            // ... actual insert operation
        });
    });
}

criterion_group!(benches, bench_single_insert);
criterion_main!(benches);
```

---

## 6. Next Steps

1. **Implement entity registry** - Core ID generation and versioning
2. **Build storage layer** - Vec<u8> buffer management  
3. **Add CRUD operations** - Insert, read, update, delete
4. **Implement double buffer** - Read/write separation
5. **Add WAL** - Write-ahead logging for durability
6. **Lock-free queue** - MPSC for write operations
7. **Replication layer** - Multi-client sync
8. **Dashboard UI** - Tauri + Vue 3
