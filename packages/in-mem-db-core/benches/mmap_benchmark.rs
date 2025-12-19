//! Benchmark comparing mmap vs traditional file reading for data loading.

use criterion::{criterion_group, criterion_main, Criterion};
use std::fs::File;
use std::hint::black_box;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::persistence::PersistenceManager;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeRegistry;

/// Creates a test database with a table and some data.
fn create_test_database(data_dir: PathBuf, record_count: usize) -> (Database, PersistenceManager) {
    let config = DbConfig {
        data_dir,
        ..Default::default()
    };

    let persistence = PersistenceManager::new(&config);

    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    let db = Database::with_type_registry(type_registry);

    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
        Field::new("value".to_string(), "u64".to_string(), u64_layout, 8),
    ];

    db.create_table("test_table".to_string(), fields, None, usize::MAX)
        .unwrap();

    // Add data
    {
        let table = db.get_table_mut("test_table").unwrap();
        for i in 0..record_count {
            let mut data = vec![0u8; table.record_size];
            data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            data[8..16].copy_from_slice(&((i * 2) as u64).to_le_bytes());
            table.create_record(&data).unwrap();
        }
    }

    (db, persistence)
}

/// Benchmark traditional file reading.
fn bench_read_to_end(c: &mut Criterion) {
    let temp_dir = tempdir().unwrap();
    let (db, persistence) = create_test_database(temp_dir.path().to_path_buf(), 1000);

    // Save schema and flush data
    persistence.save_schema(&db).unwrap();
    persistence.flush_all_tables(&db).unwrap();

    let data_path = temp_dir.path().join("data").join("test_table.bin");

    c.bench_function("read_to_end", |b| {
        b.iter(|| {
            let mut file = File::open(&data_path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(black_box(&mut data)).unwrap();
            black_box(data.len());
        });
    });
}

/// Benchmark memory-mapped file reading (when persist feature is enabled).
#[cfg(feature = "persist")]
fn bench_mmap(c: &mut Criterion) {
    use memmap2::Mmap;

    let temp_dir = tempdir().unwrap();
    let (db, persistence) = create_test_database(temp_dir.path().to_path_buf(), 1000);

    // Save schema and flush data
    persistence.save_schema(&db).unwrap();
    persistence.flush_all_tables(&db).unwrap();

    let data_path = temp_dir.path().join("data").join("test_table.bin");

    c.bench_function("mmap", |b| {
        b.iter(|| {
            let file = File::open(&data_path).unwrap();
            let mmap = unsafe { Mmap::map(&file).unwrap() };
            black_box(mmap.len());
        });
    });
}

/// Benchmark loading table data with traditional method.
fn bench_load_table_data_traditional(c: &mut Criterion) {
    let temp_dir = tempdir().unwrap();
    let (db, persistence) = create_test_database(temp_dir.path().to_path_buf(), 1000);

    // Save schema and flush data
    persistence.save_schema(&db).unwrap();
    persistence.flush_all_tables(&db).unwrap();

    // Create new database for loading
    let type_registry2 = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry2).unwrap();
    let db2 = Database::with_type_registry(type_registry2);

    let u64_layout = db2.type_registry().get("u64").unwrap().clone();
    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
        Field::new("value".to_string(), "u64".to_string(), u64_layout, 8),
    ];
    db2.create_table("test_table".to_string(), fields, None, usize::MAX)
        .unwrap();

    let table2 = db2.get_table("test_table").unwrap();

    c.bench_function("load_table_data_traditional", |b| {
        b.iter(|| {
            // Simulate traditional loading without mmap
            let data_path = temp_dir.path().join("data").join("test_table.bin");
            let mut file = File::open(&data_path).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();

            // Validate and store (simplified)
            assert!(data.len() % table2.record_size == 0);
            black_box(data.len());
        });
    });
}

/// Benchmark loading table data with mmap (when persist feature is enabled).
#[cfg(feature = "persist")]
fn bench_load_table_data_mmap(c: &mut Criterion) {
    use memmap2::Mmap;

    let temp_dir = tempdir().unwrap();
    let (db, persistence) = create_test_database(temp_dir.path().to_path_buf(), 1000);

    // Save schema and flush data
    persistence.save_schema(&db).unwrap();
    persistence.flush_all_tables(&db).unwrap();

    // Create new database for loading
    let type_registry2 = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry2).unwrap();
    let db2 = Database::with_type_registry(type_registry2);

    let u64_layout = db2.type_registry().get("u64").unwrap().clone();
    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
        Field::new("value".to_string(), "u64".to_string(), u64_layout, 8),
    ];
    db2.create_table("test_table".to_string(), fields, None, usize::MAX)
        .unwrap();

    let table2 = db2.get_table("test_table").unwrap();

    c.bench_function("load_table_data_mmap", |b| {
        b.iter(|| {
            // Simulate mmap loading
            let data_path = temp_dir.path().join("data").join("test_table.bin");
            let file = File::open(&data_path).unwrap();
            let mmap = unsafe { Mmap::map(&file).unwrap() };

            // Validate (simplified)
            assert!(mmap.len() % table2.record_size == 0);
            black_box(mmap.len());
        });
    });
}

/// Create benchmark group.
fn criterion_benchmark(c: &mut Criterion) {
    bench_read_to_end(c);

    #[cfg(feature = "persist")]
    bench_mmap(c);

    bench_load_table_data_traditional(c);

    #[cfg(feature = "persist")]
    bench_load_table_data_mmap(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
