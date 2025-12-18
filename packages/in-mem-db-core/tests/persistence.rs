//! Integration test for persistence features.

use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::persistence::PersistenceManager;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeRegistry;
use ntest::timeout;

#[timeout(1000)]
#[test]
fn test_persistence_integration() {
    let temp_dir = tempdir().unwrap();

    // Create configuration
    let config = DbConfig {
        data_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Create type registry with built-in types
    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Database::with_type_registry(type_registry);

    // Create a table with some fields
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let bool_layout = db.type_registry().get("bool").unwrap().clone();

    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
        Field::new("active".to_string(), "bool".to_string(), bool_layout, 8),
    ];

    db.create_table("users".to_string(), fields, None, usize::MAX)
        .unwrap();

    // Add some data
    {
        let table = db.get_table_mut("users").unwrap();

        // Create first record
        let mut data1 = vec![0u8; table.record_size];
        data1[0..8].copy_from_slice(&1u64.to_le_bytes());
        data1[8] = 1; // bool true
        table.create_record(&data1).unwrap();

        // Create second record
        let mut data2 = vec![0u8; table.record_size];
        data2[0..8].copy_from_slice(&2u64.to_le_bytes());
        data2[8] = 0; // bool false
        table.create_record(&data2).unwrap();
    }

    // Save schema and flush data
    persistence.save_schema(&db).unwrap();
    persistence.flush_all_tables(&db).unwrap();

    // Verify files exist
    let schema_path = temp_dir.path().join("schema.json");
    let data_path = temp_dir.path().join("data").join("users.bin");

    assert!(schema_path.exists());
    assert!(data_path.exists());

    // Verify schema file content
    let schema_content = fs::read_to_string(&schema_path).unwrap();
    assert!(schema_content.contains("\"users\""));
    assert!(schema_content.contains("\"id\""));
    assert!(schema_content.contains("\"active\""));

    // Verify data file size
    let data_size = fs::metadata(&data_path).unwrap().len();
    let table = db.get_table("users").unwrap();
    assert_eq!(data_size as usize, 2 * table.record_size);

    // Now load into a new database
    let type_registry2 = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry2).unwrap();

    let db2 = persistence.load_schema(type_registry2).unwrap();

    // Load table data
    let table2 = db2.get_table("users").unwrap();
    persistence.load_table_data(&table2).unwrap();

    // Verify data was loaded correctly
    assert_eq!(table2.record_count(), 2);
    assert_eq!(table2.current_next_id(), 3); // next_id should be max id + 1

    // Read and verify records
    let (record1, _arc1) = table2.read_record(0).unwrap();
    let id1 = u64::from_le_bytes(record1[0..8].try_into().unwrap());
    let active1 = record1[8] != 0;

    assert_eq!(id1, 1);
    assert!(active1);

    let (record2, _arc2) = table2.read_record(1).unwrap();
    let id2 = u64::from_le_bytes(record2[0..8].try_into().unwrap());
    let active2 = record2[8] != 0;

    assert_eq!(id2, 2);
    assert!(!active2);

    println!("Persistence integration test passed!");
}
