# ECS Database - Deployment & Operations Guide

## 1. Production Deployment

### 1.1 Embedded Library Deployment

#### Option A: Cargo Dependency
```toml
[dependencies]
ecsdb = { version = "0.1", features = ["replication", "dashboard"] }
tokio = { version = "1", features = ["full"] }
```

#### Option B: Local Monorepo
```bash
# Add as path dependency
[dependencies]
ecsdb = { path = "../ecsdb", features = ["full"] }
```

### 1.2 Docker Deployment (for distributed servers)

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ecsdb-server /usr/local/bin/

ENV ECSDB_BIND=0.0.0.0:8080
ENV ECSDB_DATA_DIR=/data

VOLUME ["/data"]
EXPOSE 8080

CMD ["ecsdb-server"]
```

### 1.3 Configuration Management

```toml
# config.toml
[database]
name = "game_world"
data_dir = "/data/ecsdb"
schema_file = "/config/schema.toml"

[memory]
initial_capacity_mb = 256
max_capacity_mb = 2048
buffer_growth_factor = 2.0

[transactions]
batch_size = 1000
batch_timeout_ms = 10
wal_enabled = true
wal_sync_on_commit = false  # Async WAL for performance

[replication]
enabled = true
bind = "0.0.0.0:8080"
max_clients = 1000
delta_compression = "zstd"

[persistence]
snapshots_enabled = true
snapshot_interval_ops = 100000
snapshot_interval_secs = 300
wal_archive_enabled = true
compaction_enabled = true
```

### 1.4 Environment Variables

```bash
# Optional: override config file values
ECSDB_DATA_DIR=/mnt/nvme/ecsdb
ECSDB_MAX_MEMORY_MB=4096
ECSDB_REPLICATION_ENABLED=true
ECSDB_REPLICATION_BIND=0.0.0.0:9000
ECSDB_WAL_SYNC=false
ECSDB_LOG_LEVEL=info
```

---

## 2. Schema Management

### 2.1 Example Game Schema

```toml
[database]
name = "rpg_game"
version = "2.1.0"

# Base entity table
[tables.entities]
description = "Core entity registry for all game objects"

[[tables.entities.fields]]
name = "id"
type = "u64"
primary_key = true

[[tables.entities.fields]]
name = "version"
type = "u32"

[[tables.entities.fields]]
name = "entity_type"
type = "u32"

# Player component
[tables.player]
parent_table = "entities"
description = "Player-specific data"

[[tables.player.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.player.fields]]
name = "name"
type = "[u8; 64]"  # Fixed-size string

[[tables.player.fields]]
name = "level"
type = "u32"

[[tables.player.fields]]
name = "experience"
type = "u64"

# Transform component
[tables.transform]
parent_table = "entities"
description = "Position and rotation in world space"

[[tables.transform.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.transform.fields]]
name = "position"
type = "[f32; 3]"

[[tables.transform.fields]]
name = "rotation"
type = "[f32; 4]"  # Quaternion

[[tables.transform.fields]]
name = "scale"
type = "[f32; 3]"

[[tables.transform.fields]]
name = "dirty"
type = "bool"

# Physics component
[tables.physics]
parent_table = "entities"
description = "Physics simulation data"

[[tables.physics.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.physics.fields]]
name = "velocity"
type = "[f32; 3]"

[[tables.physics.fields]]
name = "acceleration"
type = "[f32; 3]"

[[tables.physics.fields]]
name = "mass"
type = "f32"

# Health component
[tables.health]
parent_table = "entities"
description = "Health and damage system"

[[tables.health.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.health.fields]]
name = "hp"
type = "u32"

[[tables.health.fields]]
name = "max_hp"
type = "u32"

[[tables.health.fields]]
name = "armor"
type = "u32"

[[tables.health.fields]]
name = "status_effects"
type = "u32"  # Bitmask

# Inventory component
[tables.inventory]
parent_table = "entities"
description = "Item carrying system"

[[tables.inventory.fields]]
name = "entity_id"
type = "u64"
foreign_key = "entities.id"
indexed = true

[[tables.inventory.fields]]
name = "items"
type = "[u32; 64]"  # Item IDs, 0 = empty slot

[[tables.inventory.fields]]
name = "item_counts"
type = "[u32; 64]"  # Stack counts

# Enum definitions
[enums.entity_type]
variants = ["player", "npc", "enemy", "item", "projectile"]

[enums.status_effect]
variants = ["poison", "burn", "freeze", "stun", "blind"]

# Custom type definitions
[custom_types.vec3]
[[custom_types.vec3.fields]]
name = "x"
type = "f32"

[[custom_types.vec3.fields]]
name = "y"
type = "f32"

[[custom_types.vec3.fields]]
name = "z"
type = "f32"

[custom_types.quat]
[[custom_types.quat.fields]]
name = "x"
type = "f32"

[[custom_types.quat.fields]]
name = "y"
type = "f32"

[[custom_types.quat.fields]]
name = "z"
type = "f32"

[[custom_types.quat.fields]]
name = "w"
type = "f32"
```

### 2.2 Schema Validation

```rust
use ecsdb::schema::{SchemaParser, SchemaValidator};

fn load_and_validate_schema(path: &str) -> Result<DatabaseSchema> {
    // Parse TOML
    let schema = SchemaParser::from_file(path)?;
    
    // Validate
    let mut validator = SchemaValidator::new();
    validator.validate(&schema)?;
    
    // Check for common mistakes
    validator.check_foreign_keys(&schema)?;
    validator.check_field_alignment(&schema)?;
    validator.check_reserved_names(&schema)?;
    
    Ok(schema)
}
```

### 2.3 Schema Evolution & Migrations

```toml
# migrations/v2_0_to_v2_1.toml
[migration]
from_version = "2.0.0"
to_version = "2.1.0"
description = "Add experience field to players"

[[operations]]
type = "add_field"
table = "player"
field = { name = "experience", type = "u64" }
default_value = 0

[[operations]]
type = "add_index"
table = "transform"
field = "position"

[[operations]]
type = "drop_field"
table = "old_data"
field = "deprecated_field"
```

---

## 3. Operational Procedures

### 3.1 Database Initialization

```rust
use ecsdb::{Database, config::DatabaseConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = DatabaseConfig::from_file("config.toml")?;
    
    // Build database
    let mut db = Database::builder()
        .config(config)
        .schema_file("schema.toml")?
        .enable_replication()?
        .enable_persistence()?
        .build()
        .await?;
    
    // Create example entity
    let entity_id = db.create_entity(0)?;
    println!("Created entity: {}", entity_id.0);
    
    Ok(())
}
```

### 3.2 Data Import/Export

```rust
// Export to JSON
pub async fn export_to_json(db: &Database, path: &str) -> Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    
    let mut json = json!({
        "schema": db.schema().name,
        "version": db.version(),
        "entities": [],
        "components": {}
    });
    
    // Export all entities and components
    // ... implementation
    
    let json_str = serde_json::to_string_pretty(&json)?;
    file.write_all(json_str.as_bytes()).await?;
    
    Ok(())
}

// Import from JSON
pub async fn import_from_json(db: &mut Database, path: &str) -> Result<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    
    let mut txn = db.transaction();
    
    if let Some(entities) = json["entities"].as_array() {
        for entity in entities {
            let entity_id = entity["id"].as_u64().unwrap();
            txn.create_entity(entity_id)?;
        }
    }
    
    txn.commit().await?;
    
    Ok(())
}
```

### 3.3 Backup & Recovery

```rust
pub async fn create_snapshot(db: &Database, backup_dir: &str) -> Result<()> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_path = format!("{}/snapshot_{}.ecsdb", backup_dir, timestamp);
    
    db.snapshot(&backup_path).await?;
    println!("Snapshot created: {}", backup_path);
    
    Ok(())
}

pub async fn restore_from_snapshot(db: &mut Database, 
                                  backup_path: &str) -> Result<()> {
    db.restore(&backup_path).await?;
    println!("Database restored from: {}", backup_path);
    
    Ok(())
}

// Automated backup schedule
pub async fn backup_scheduler(db: Arc<Database>, 
                             backup_dir: String, 
                             interval_secs: u64) {
    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(interval_secs)
    );
    
    loop {
        interval.tick().await;
        if let Err(e) = create_snapshot(&db, &backup_dir).await {
            eprintln!("Backup failed: {}", e);
        }
    }
}
```

### 3.4 Monitoring & Metrics

```rust
pub struct DatabaseMetrics {
    pub total_entities: u64,
    pub total_operations: u64,
    pub avg_operation_latency_us: f64,
    pub avg_replication_lag_ms: f64,
    pub write_buffer_utilization: f32,
    pub disk_usage_mb: u64,
}

impl Database {
    pub fn metrics(&self) -> DatabaseMetrics {
        DatabaseMetrics {
            total_entities: self.entity_registry.entity_count() as u64,
            total_operations: self.operation_count(),
            avg_operation_latency_us: self.avg_latency_us(),
            avg_replication_lag_ms: self.avg_replication_lag_ms(),
            write_buffer_utilization: self.buffer_utilization(),
            disk_usage_mb: self.disk_usage_mb(),
        }
    }
}

// Expose Prometheus metrics
pub async fn metrics_server(db: Arc<Database>) {
    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(
            move || {
                let metrics = db.metrics();
                let output = format!(
                    "# HELP ecsdb_entities Total entities\n\
                     # TYPE ecsdb_entities gauge\n\
                     ecsdb_entities {}\n\
                     # HELP ecsdb_latency_us Operation latency\n\
                     # TYPE ecsdb_latency_us gauge\n\
                     ecsdb_latency_us {}\n",
                    metrics.total_entities,
                    metrics.avg_operation_latency_us
                );
                async { output }
            }
        ));
    
    axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

### 3.5 Logging & Debugging

```rust
use tracing::{info, warn, error, debug, trace};

// Initialize logging
pub fn init_logging(level: &str) {
    tracing_subscriber::fmt()
        .with_max_level(
            level.parse().unwrap_or(tracing::Level::INFO)
        )
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

// Usage in code
pub async fn process_transaction(txn: &Transaction) {
    debug!("Processing transaction with {} operations", txn.operations.len());
    
    match apply_transaction(txn).await {
        Ok(version) => {
            info!("Transaction committed: version={}", version);
        }
        Err(e) => {
            error!("Transaction failed: {}", e);
        }
    }
}
```

---

## 4. Performance Tuning

### 4.1 Configuration Recommendations

```toml
# High-throughput game server
[memory]
initial_capacity_mb = 2048
max_capacity_mb = 8192

[transactions]
batch_size = 5000
batch_timeout_ms = 5  # More aggressive batching

[replication]
delta_compression = "zstd"
level = 10  # Zstd compression level

[persistence]
wal_sync_on_commit = false  # Async only
snapshot_interval_ops = 500000
compaction_enabled = true
compaction_threads = 4
```

### 4.2 Profiling

```bash
# Profile with perf
cargo build --release
perf record -g ./target/release/my_app
perf report

# Memory profiling
valgrind --tool=massif ./target/release/my_app
massif-visualizer massif.out.*

# Benchmark with criterion
cargo bench --bench inserts -- --profile-time 10
```

### 4.3 Bottleneck Identification

```rust
pub struct ProfileGuard {
    name: String,
    start: std::time::Instant,
}

impl ProfileGuard {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for ProfileGuard {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_micros();
        println!("{}: {} μs", self.name, elapsed);
    }
}

// Usage
pub fn insert_entity(db: &Database) {
    let _profile = ProfileGuard::new("insert_entity");
    // ... actual work
}
```

---

## 5. Troubleshooting

### 5.1 Common Issues

| Issue | Symptom | Solution |
|-------|---------|----------|
| **OOM Errors** | Process killed, memory usage growing | Reduce batch_size, enable snapshots |
| **Slow Writes** | High latency for inserts | Increase batch_size, disable WAL sync |
| **Replication Lag** | Clients out of sync | Increase delta_compression, add bandwidth |
| **Schema Conflicts** | Migration errors | Validate schema before deploying |
| **Corrupted Data** | Recovery fails | Use older snapshot, check checksums |

### 5.2 Diagnostic Commands

```bash
# Check database size
du -sh /data/ecsdb

# Monitor in real-time
watch -n 1 'curl localhost:9090/metrics | grep ecsdb'

# Check WAL file integrity
ecsdb-verify /data/ecsdb/wal_archive/

# Dump schema
ecsdb-dump-schema /data/ecsdb/schema.toml
```

### 5.3 Recovery Procedures

```bash
# Restore from latest snapshot
ecsdb-restore /data/ecsdb/snapshots/latest.ecsdb

# Rebuild from WAL if snapshots unavailable
ecsdb-rebuild-from-wal /data/ecsdb/wal_archive/

# Validate and repair
ecsdb-fsck /data/ecsdb/
```

---

## 6. Security Considerations

### 6.1 Access Control

```rust
// Simple authentication for replication clients
pub struct ReplicationAuth {
    allowed_ips: Vec<IpAddr>,
    api_keys: HashMap<String, ReplicationPermissions>,
}

pub struct ReplicationPermissions {
    pub can_read: bool,
    pub can_write: bool,
    pub can_admin: bool,
}
```

### 6.2 Data Encryption

```rust
// Optional: encrypt WAL and snapshots
pub struct EncryptedStorage {
    cipher: aes_gcm::Aes256Gcm,
    key: [u8; 32],
    nonce: [u8; 12],
}

impl EncryptedStorage {
    pub fn encrypt_wal_entry(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Encrypt with AES-256-GCM
        Ok(self.cipher.encrypt(&self.nonce.into(), data)?)
    }
}
```

---

## 7. Dashboard Setup (Tauri + Vue 3)

### 7.1 Project Structure

```
dashboard/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Tauri window setup
│   │   ├── commands.rs       # Exported commands
│   │   └── db.rs             # Database bindings
│   ├── tauri.conf.json
│   └── Cargo.toml
├── src/
│   ├── components/
│   │   ├── SchemaEditor.vue
│   │   ├── EntityBrowser.vue
│   │   └── QueryBuilder.vue
│   ├── App.vue
│   ├── main.ts
│   └── views/
│       ├── Dashboard.vue
│       ├── Tables.vue
│       └── Replication.vue
├── package.json
└── vite.config.ts
```

### 7.2 Tauri Commands

```rust
// src-tauri/src/commands.rs

#[tauri::command]
fn load_schema(path: String) -> Result<DatabaseSchema> {
    SchemaParser::from_file(&path)
}

#[tauri::command]
async fn create_entity(db: State<'_, Arc<Database>>, 
                      entity_type: u32) -> Result<u64> {
    db.create_entity(entity_type)
        .map(|e| e.0)
}

#[tauri::command]
async fn get_metrics(db: State<'_, Arc<Database>>) -> Result<DatabaseMetrics> {
    Ok(db.metrics())
}

#[tauri::command]
async fn export_data(db: State<'_, Arc<Database>>, 
                    format: String) -> Result<String> {
    match format.as_str() {
        "json" => db.export_json().await,
        "csv" => db.export_csv().await,
        _ => Err(EcsDbError::SchemaError("Unknown format".into())),
    }
}
```

### 7.3 Vue Components

```vue
<!-- src/components/SchemaEditor.vue -->
<template>
  <div class="schema-editor">
    <h2>Schema Editor</h2>
    
    <div v-for="table in schema.tables" :key="table.name" class="table-card">
      <h3>{{ table.name }}</h3>
      
      <div class="fields">
        <div v-for="field in table.fields" :key="field.name" class="field">
          <input v-model="field.name" placeholder="Field name" />
          <select v-model="field.field_type">
            <option>u32</option>
            <option>u64</option>
            <option>f32</option>
            <option>f64</option>
            <option>bool</option>
          </select>
          <button @click="removeField(table.name, field.name)">Delete</button>
        </div>
        <button @click="addField(table.name)">+ Add Field</button>
      </div>
    </div>
    
    <button @click="saveSchema" class="primary">Save Schema</button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/tauri'

const schema = ref({})

onMounted(async () => {
  schema.value = await invoke('load_schema', { path: 'schema.toml' })
})
</script>
```

---

## 8. Continuous Integration/Deployment

### 8.1 GitHub Actions

```yaml
# .github/workflows/test.yml

name: Test & Release

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Test
        run: cargo test --all-features
      
      - name: Benchmark
        run: cargo bench --no-run
      
      - name: Build Release
        run: cargo build --release
      
      - name: Upload Artifact
        uses: actions/upload-artifact@v3
        with:
          name: ecsdb-${{ matrix.os }}
          path: target/release/
```

### 8.2 Automated Deployment

```bash
#!/bin/bash
# deploy.sh

set -e

VERSION=$(grep version Cargo.toml | head -1 | awk '{print $3}' | tr -d '"')

# Build release binary
cargo build --release

# Create docker image
docker build -t ecsdb:${VERSION} .

# Push to registry
docker push myregistry/ecsdb:${VERSION}

# Deploy to kubernetes
kubectl set image deployment/ecsdb \
  ecsdb=myregistry/ecsdb:${VERSION} \
  --record
```

---

## 9. Scaling Strategies

### 9.1 Horizontal Scaling

- **Read replicas**: Secondary databases for read-heavy workloads
- **Sharding**: Partition entities across multiple database instances
- **Federated databases**: Multiple regions with eventual consistency

### 9.2 Vertical Scaling

- **Increase memory**: More buffer capacity
- **More cores**: Parallel persistence workers
- **NVMe storage**: Faster WAL and snapshot I/O

### 9.3 Performance Optimization

```rust
// Archetype optimization: group entities by component set
pub fn optimize_by_archetype(db: &Database) -> Result<()> {
    // Reorganize entities with same components nearby
    // This improves CPU cache locality
    db.reorder_by_archetype()?;
    Ok(())
}
```

---

## 10. Maintenance Schedule

| Task | Frequency | Purpose |
|------|-----------|---------|
| **Snapshots** | Every 5 minutes | Quick recovery |
| **WAL Compaction** | Every hour | Reduce file count |
| **Full Backup** | Every 24 hours | Disaster recovery |
| **Performance Analysis** | Weekly | Identify trends |
| **Schema Audit** | Monthly | Ensure consistency |
| **Capacity Planning** | Quarterly | Prevent out-of-memory |

