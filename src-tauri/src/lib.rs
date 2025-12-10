// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use ecsdb::db::Database;
use ecsdb::replication::{ReplicationConfig, ReplicationManager};
use ecsdb::replication::client::ClientInfo;
use ecsdb::replication::conflict::Conflict;
use serde_json::{self, Value};
use std::result::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Application state shared across commands
struct AppState {
    db: Mutex<Option<Arc<Database>>>,
    replication_manager: Mutex<Option<Arc<Mutex<ReplicationManager>>>>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Initialize database with a schema file.
/// Returns the number of tables loaded.
#[tauri::command]
async fn init_database(
    schema_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<u64, String> {
    let db = Database::from_schema_file(&schema_path)
        .map_err(|e| format!("Failed to load schema: {}", e))?;

    // Store database in application state
    let mut db_lock = state.db.lock().await;
    *db_lock = Some(Arc::new(db));

    // For demonstration, return version (currently 0)
    Ok(0)
}

/// Create a new entity in the database.
/// Returns the entity ID as u64.
#[tauri::command]
async fn create_entity(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;

    let entity_id = db
        .create_entity()
        .map_err(|e| format!("Failed to create entity: {}", e))?;

    Ok(entity_id.0)
}

/// Returns the database schema as JSON.
#[tauri::command]
async fn get_schema(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    let schema = db.schema();
    serde_json::to_value(schema.as_ref()).map_err(|e| format!("Failed to serialize schema: {}", e))
}

/// Returns list of tables in the database.
#[tauri::command]
async fn get_tables(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    let schema = db.schema();
    let table_names = schema.tables.iter().map(|t| t.name.clone()).collect();
    Ok(table_names)
}

/// Returns the number of entities in a given table.
#[tauri::command]
async fn get_entity_count(
    table_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    let table_id = db
        .get_table_id_by_name(&table_name)
        .ok_or_else(|| format!("Table '{}' not found", table_name))?;
    Ok(db.get_entity_count_for_table(table_id))
}

/// Returns entity data for a given table with pagination.
#[tauri::command]
async fn fetch_entities(
    table_name: String,
    limit: usize,
    offset: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<(u64, Vec<u8>)>, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    let table_id = db
        .get_table_id_by_name(&table_name)
        .ok_or_else(|| format!("Table '{}' not found", table_name))?;
    db.get_entities_for_table(table_id, limit, offset)
        .map_err(|e| format!("Failed to fetch entities: {}", e))
}

/// Returns entity data as JSON for a given table with pagination.
#[tauri::command]
async fn fetch_entities_json(
    table_name: String,
    limit: usize,
    offset: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<(u64, Value)>, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    db.get_entities_json_for_table(&table_name, limit, offset)
        .map_err(|e| format!("Failed to fetch entities: {}", e))
}

/// Insert component data from JSON.
#[tauri::command]
async fn insert_component(
    table_name: String,
    entity_id: u64,
    json: Value,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    db.insert_from_json(&table_name, entity_id, json)
        .map_err(|e| format!("Failed to insert component: {}", e))?;
    Ok(())
}

/// Update component data from JSON.
#[tauri::command]
async fn update_component(
    table_name: String,
    entity_id: u64,
    json: Value,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    db.update_from_json(&table_name, entity_id, json)
        .map_err(|e| format!("Failed to update component: {}", e))?;
    Ok(())
}

/// Delete component for a given entity and table.
#[tauri::command]
async fn delete_component(
    table_name: String,
    entity_id: u64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    db.delete_by_table(&table_name, entity_id)
        .map_err(|e| format!("Failed to delete component: {}", e))?;
    Ok(())
}

/// Commit pending writes to make them visible.
#[tauri::command]
async fn commit_database(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db_lock = state.db.lock().await;
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;
    db.commit()
        .map_err(|e| format!("Failed to commit: {}", e))
}

/// Starts the replication server with default configuration.
#[tauri::command]
async fn start_replication(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut manager_lock = state.replication_manager.lock().await;
    if manager_lock.is_some() {
        return Err("Replication already started".to_string());
    }
    let config = ReplicationConfig::default();
    let mut manager = ReplicationManager::new(config);
    manager
        .start()
        .await
        .map_err(|e| format!("Failed to start replication: {}", e))?;
    *manager_lock = Some(Arc::new(Mutex::new(manager)));
    Ok(())
}

/// Stops the replication server.
#[tauri::command]
async fn stop_replication(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut manager_lock = state.replication_manager.lock().await;
    if let Some(manager) = manager_lock.take() {
        let mut manager = manager.lock().await;
        manager
            .stop()
            .await
            .map_err(|e| format!("Failed to stop replication: {}", e))?;
    }
    Ok(())
}

/// Returns the number of connected clients.
#[tauri::command]
async fn get_connected_clients(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let manager_lock = state.replication_manager.lock().await;
    let manager = manager_lock.as_ref().ok_or("Replication not started")?;
    let manager = manager.lock().await;
    Ok(manager.connected_clients().await)
}

/// Returns serializable information for all connected clients.
#[tauri::command]
async fn get_clients(state: tauri::State<'_, AppState>) -> Result<Vec<ClientInfo>, String> {
    let manager_lock = state.replication_manager.lock().await;
    let manager = manager_lock.as_ref().ok_or("Replication not started")?;
    let manager = manager.lock().await;
    Ok(manager.get_clients().await)
}

/// Returns the number of pending delta batches in the broadcast queue.
#[tauri::command]
async fn get_pending_delta_count(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let manager_lock = state.replication_manager.lock().await;
    let manager = manager_lock.as_ref().ok_or("Replication not started")?;
    let manager = manager.lock().await;
    Ok(manager.pending_delta_count().await)
}

/// Returns the conflict log entries.
#[tauri::command]
async fn get_conflict_log(state: tauri::State<'_, AppState>) -> Result<Vec<Conflict>, String> {
    let manager_lock = state.replication_manager.lock().await;
    let manager = manager_lock.as_ref().ok_or("Replication not started")?;
    let manager = manager.lock().await;
    let log = manager.conflict_resolver().log();
    Ok(log.conflicts().to_vec())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db: Mutex::new(None),
            replication_manager: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            init_database,
            create_entity,
            get_schema,
            get_tables,
            get_entity_count,
            fetch_entities,
            fetch_entities_json,
            insert_component,
            update_component,
            delete_component,
            commit_database,
            start_replication,
            stop_replication,
            get_connected_clients,
            get_clients,
            get_pending_delta_count,
            get_conflict_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
