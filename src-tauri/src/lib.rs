// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use ecsdb::db::Database;
use std::result::Result;
use std::sync::{Arc, Mutex};

/// Application state shared across commands
struct AppState {
    db: Mutex<Option<Arc<Database>>>,
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
    let mut db_lock = state.db.lock().unwrap();
    *db_lock = Some(Arc::new(db));

    // For demonstration, return version (currently 0)
    Ok(0)
}

/// Create a new entity in the database.
/// Returns the entity ID as u64.
#[tauri::command]
async fn create_entity(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db_lock = state.db.lock().unwrap();
    let db = db_lock
        .as_ref()
        .ok_or("Database not initialized. Call init_database first.")?;

    let entity_id = db
        .create_entity()
        .map_err(|e| format!("Failed to create entity: {}", e))?;

    Ok(entity_id.0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            init_database,
            create_entity
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
