//! Tests for persistence module.

use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use crate::config::DbConfig;
use crate::database::Database;
use crate::persistence::PersistenceManager;
use crate::table::Field;
use crate::types::TypeRegistry;
use ntest::timeout;

#[timeout(1000)]
#[test]
fn test_save_and_load_schema() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry with test types
    let type_registry = Arc::new(TypeRegistry::new());

    // Register built-in types
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let string_layout = db.type_registry().get("string").unwrap().clone();

    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
        Field::new("name".to_string(), "string".to_string(), string_layout, 8),
    ];

    db.create_table("test_table".to_string(), fields, None)
        .unwrap();

    // Save schema
    persistence.save_schema(&db).unwrap();

    // Verify schema file exists
    let schema_path = temp_dir.path().join("schema.json");
    assert!(schema_path.exists());

    // Load schema into new database
    let type_registry2 = Arc::new(TypeRegistry::new());

    // Register built-in types in the new registry
    crate::types::register_builtin_types(&type_registry2).unwrap();

    let db2 = persistence.load_schema(type_registry2).unwrap();

    // Verify table was loaded
    assert_eq!(db2.table_count(), 1);
    let table_names = db2.table_names();
    assert_eq!(table_names, vec!["test_table".to_string()]);

    // Verify table structure
    let table = db2.get_table("test_table").unwrap();
    assert_eq!(table.name, "test_table");
    assert_eq!(table.fields.len(), 2);
    assert_eq!(table.fields[0].name, "id");
    assert_eq!(table.fields[0].type_id, "u64");
    assert_eq!(table.fields[1].name, "name");
    assert_eq!(table.fields[1].type_id, "string");
}

#[timeout(1000)]
#[test]
fn test_flush_and_load_table_data() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry with test types
    let type_registry = Arc::new(TypeRegistry::new());

    // Register built-in types
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();

    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("test_table".to_string(), fields, None)
        .unwrap();

    // Get table and add some data
    let table = db.get_table_mut("test_table").unwrap();

    // Create a record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    table.create_record(&data).unwrap();

    // Flush table data
    persistence.flush_table_data(&table).unwrap();

    // Verify data file exists
    let data_path = temp_dir.path().join("data").join("test_table.bin");
    assert!(data_path.exists());

    // Create new table and load data
    let type_registry2 = Arc::new(TypeRegistry::new());
    crate::types::register_builtin_types(&type_registry2).unwrap();

    let db2 = Database::with_type_registry(type_registry2);
    let u64_layout = db2.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];
    db2.create_table("test_table".to_string(), fields, None)
        .unwrap();

    let table2 = db2.get_table("test_table").unwrap();
    persistence.load_table_data(&table2).unwrap();

    // Verify data was loaded
    assert_eq!(table2.record_count(), 1);
    assert_eq!(table2.current_next_id(), 2); // next_id should be restored to max id + 1
}

#[timeout(1000)]
#[test]
fn test_atomic_rename_schema() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());

    // Register built-in types
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();

    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("test_table".to_string(), fields, None)
        .unwrap();

    // Save schema multiple times to test atomic rename
    for i in 0..3 {
        persistence.save_schema(&db).unwrap();

        // Verify schema file exists and is valid JSON
        let schema_path = temp_dir.path().join("schema.json");
        assert!(schema_path.exists());

        let contents = fs::read_to_string(&schema_path).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(schema["version"], 1);
        assert!(schema["tables"].is_object());

        // Verify temp file doesn't exist after rename
        let temp_path = temp_dir.path().join("schema.json.tmp");
        assert!(
            !temp_path.exists(),
            "Temp file should not exist after iteration {}",
            i
        );
    }
}

#[timeout(1000)]
#[test]
fn test_flush_all_tables() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());

    // Register built-in types
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Create database with multiple tables
    let db = Database::with_type_registry(type_registry);
    let u64_layout = db.type_registry().get("u64").unwrap().clone();

    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout.clone(),
        0,
    )];

    db.create_table("table1".to_string(), fields.clone(), None)
        .unwrap();
    db.create_table("table2".to_string(), fields, None).unwrap();

    // Add data to tables - need to drop mutable references before flushing
    {
        let table1 = db.get_table_mut("table1").unwrap();
        let mut data = vec![0u8; table1.record_size];
        data[0..8].copy_from_slice(&1u64.to_le_bytes());
        table1.create_record(&data).unwrap();
    }

    {
        let table2 = db.get_table_mut("table2").unwrap();
        let mut data = vec![0u8; table2.record_size];
        data[0..8].copy_from_slice(&2u64.to_le_bytes());
        table2.create_record(&data).unwrap();
    }

    // Flush all tables
    persistence.flush_all_tables(&db).unwrap();

    // Verify both data files exist
    let data_dir = temp_dir.path().join("data");
    assert!(data_dir.exists());

    let table1_path = data_dir.join("table1.bin");
    let table2_path = data_dir.join("table2.bin");

    assert!(table1_path.exists());
    assert!(table2_path.exists());

    // Verify data was written
    let table1_data = fs::read(&table1_path).unwrap();
    assert_eq!(table1_data.len(), 8); // 1 record * 8 bytes

    let table2_data = fs::read(&table2_path).unwrap();
    assert_eq!(table2_data.len(), 8); // 1 record * 8 bytes
}

#[timeout(1000)]
#[test]
fn test_custom_types_persistence() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry with custom type
    let type_registry = Arc::new(TypeRegistry::new());

    // Register built-in types
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Register a custom type (3xf32)
    crate::types::register_3xf32_type(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table with custom type field
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let custom_type_layout = db.type_registry().get("3xf32").unwrap().clone();

    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
        Field::new(
            "position".to_string(),
            "3xf32".to_string(),
            custom_type_layout,
            8,
        ),
    ];

    db.create_table("entities".to_string(), fields, None)
        .unwrap();

    // Save schema
    persistence.save_schema(&db).unwrap();

    // Verify schema file exists and contains custom types
    let schema_path = temp_dir.path().join("schema.json");
    assert!(schema_path.exists());

    let contents = fs::read_to_string(&schema_path).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // Check version
    assert_eq!(schema["version"], 1);

    // Check tables
    assert!(schema["tables"].is_object());
    assert!(schema["tables"]["entities"].is_object());

    // Check custom types section
    assert!(schema["custom_types"].is_object());
    let custom_types = &schema["custom_types"];

    // Verify 3xf32 custom type is present
    assert!(custom_types["3xf32"].is_object());
    let type_schema = &custom_types["3xf32"];
    assert_eq!(type_schema["size"], 12);
    assert_eq!(type_schema["align"], 4);
    assert_eq!(type_schema["pod"], true);

    // Verify built-in types are NOT in custom_types
    assert!(!custom_types["u64"].is_object());
    assert!(!custom_types["string"].is_object());

    // Now test loading the schema with custom types
    let type_registry2 = Arc::new(TypeRegistry::new());

    // Register built-in types before loading schema
    crate::types::register_builtin_types(&type_registry2).unwrap();

    // Load schema (should register custom types automatically)
    let db2 = persistence.load_schema(type_registry2).unwrap();

    // Verify custom type is registered
    assert!(db2.type_registry().contains("3xf32"));

    // Verify table was loaded
    assert_eq!(db2.table_count(), 1);
    let table = db2.get_table("entities").unwrap();
    assert_eq!(table.name, "entities");
    assert_eq!(table.fields.len(), 2);
    assert_eq!(table.fields[0].type_id, "u64");
    assert_eq!(table.fields[1].type_id, "3xf32");
}

#[timeout(1000)]
#[test]
fn test_corruption_detection_with_checksums() {
    let temp_dir = tempdir().unwrap();
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());
    crate::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("test_table".to_string(), fields, None)
        .unwrap();

    // Save schema (initial save without checksums)
    persistence.save_schema(&db).unwrap();

    // Get table and add some data
    let table = db.get_table_mut("test_table").unwrap();
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    table.create_record(&data).unwrap();

    // Flush table data (this should calculate and store checksum)
    persistence.flush_table_data(&table).unwrap();

    // Verify checksum was stored in schema
    let schema_path = temp_dir.path().join("schema.json");
    let contents = fs::read_to_string(&schema_path).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(schema["checksums"].is_object());
    assert!(schema["checksums"]["test_table"].is_number());

    // Load data successfully (should verify checksum)
    let type_registry2 = Arc::new(TypeRegistry::new());
    crate::types::register_builtin_types(&type_registry2).unwrap();
    let db2 = Database::with_type_registry(type_registry2);
    let u64_layout = db2.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];
    db2.create_table("test_table".to_string(), fields, None)
        .unwrap();
    let table2 = db2.get_table("test_table").unwrap();
    persistence.load_table_data(&table2).unwrap();

    // Verify data was loaded
    assert_eq!(table2.record_count(), 1);

    // Now corrupt the data file
    let data_path = temp_dir.path().join("data").join("test_table.bin");
    let mut corrupted_data = fs::read(&data_path).unwrap();
    // Corrupt one byte
    if !corrupted_data.is_empty() {
        corrupted_data[0] = corrupted_data[0].wrapping_add(1);
    }
    fs::write(&data_path, &corrupted_data).unwrap();

    // Try to load corrupted data - should fail with DataCorruption error
    let type_registry3 = Arc::new(TypeRegistry::new());
    crate::types::register_builtin_types(&type_registry3).unwrap();
    let db3 = Database::with_type_registry(type_registry3);
    let u64_layout = db3.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "id".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];
    db3.create_table("test_table".to_string(), fields, None)
        .unwrap();
    let table3 = db3.get_table("test_table").unwrap();

    let result = persistence.load_table_data(&table3);
    assert!(result.is_err());
    match result {
        Err(crate::error::DbError::DataCorruption(_)) => {
            // Expected error
        }
        Err(e) => panic!("Expected DataCorruption error, got: {:?}", e),
        Ok(_) => panic!("Expected error when loading corrupted data"),
    }
}
