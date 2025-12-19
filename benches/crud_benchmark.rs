//! Criterion benchmarks for CRUD operations.
//!
//! Performance regression tests for in-memory database:
//! - Baseline throughput: >10M reads/sec/core
//! - Write throughput: >1M writes/sec/core

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};
use std::sync::Arc;
use std::time::Instant;

/// Creates a test table with simple schema for benchmarking.
fn create_benchmark_table(db: &Database) {
    let registry = TypeRegistry::new();
    
    // Create u64 type layout
    let u64_layout = unsafe {
        TypeLayout::new(
            "u64".to_string(),
            8,
            8,
            true,
            |src, dst| {
                dst.extend_from_slice(std::slice::from_raw_parts(src, 8));
                8
            },
            |src, dst| {
                if src.len() >= 8 {
                    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 8);
                    8
                } else {
                    0
                }
            },
            Some(std::any::TypeId::of::<u64>()),
        )
    };
    
    // Create fields
    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
    ];
    
    db.create_table("benchmark".to_string(), fields, Some(1_000_000), usize::MAX)
        .expect("Failed to create benchmark table");
}

/// Benchmark: Baseline read throughput (>10M reads/sec/core target)
fn benchmark_baseline_read_throughput(c: &mut Criterion) {
    let db = Database::new();
    create_benchmark_table(&db);
    
    // Pre-populate with test data
    let table = db.get_table("benchmark").unwrap();
    let record_size = table.record_size;
    
    // Create 1000 records
    for i in 0..1000 {
        let mut data = vec![0u8; record_size];
        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        table.create_record(&data).unwrap();
    }
    
    c.bench_function("baseline_read_throughput", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            
            for i in 0..iters {
                let idx = (i % 1000) as usize;
                let result = table.read_record(idx);
                black_box(result);
            }
            
            start.elapsed()
        })
    });
    
    // Performance assertion
    let mut group = c.benchmark_group("throughput_assertions");
    group.bench_function("assert_baseline_read_throughput", |b| {
        b.iter(|| {
            let start = Instant::now();
            let iterations = 10_000;
            
            for i in 0..iterations {
                let idx = (i % 1000) as usize;
                let result = table.read_record(idx);
                black_box(result);
            }
            
            let elapsed = start.elapsed();
            let reads_per_sec = iterations as f64 / elapsed.as_secs_f64();
            
            // Assert >10M reads/sec/core
            assert!(
                reads_per_sec > 10_000_000.0,
                "Baseline read throughput too low: {:.2} reads/sec (target: >10M reads/sec/core)",
                reads_per_sec
            );
        });
    });
    group.finish();
}

/// Benchmark: Write throughput (>1M writes/sec/core target)
fn benchmark_write_throughput(c: &mut Criterion) {
    let db = Database::new();
    create_benchmark_table(&db);
    
    let table = db.get_table("benchmark").unwrap();
    let record_size = table.record_size;
    
    c.bench_function("write_throughput", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            
            for i in 0..iters {
                let mut data = vec![0u8; record_size];
                data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                let result = table.create_record(&data);
                black_box(result);
            }
            
            start.elapsed()
        })
    });
    
    // Performance assertion
    let mut group = c.benchmark_group("throughput_assertions");
    group.bench_function("assert_write_throughput", |b| {
        b.iter(|| {
            let start = Instant::now();
            let iterations = 1_000;
            
            for i in 0..iterations {
                let mut data = vec![0u8; record_size];
                data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                let result = table.create_record(&data);
                black_box(result);
            }
            
            let elapsed = start.elapsed();
            let writes_per_sec = iterations as f64 / elapsed.as_secs_f64();
            
            // Assert >1M writes/sec/core
            assert!(
                writes_per_sec > 1_000_000.0,
                "Write throughput too low: {:.2} writes/sec (target: >1M writes/sec/core)",
                writes_per_sec
            );
        });
    });
    group.finish();
}

/// Benchmark: Concurrent read/write operations
fn benchmark_concurrent_operations(c: &mut Criterion) {
    let db = Database::new();
    create_benchmark_table(&db);
    
    let table = db.get_table("benchmark").unwrap();
    let record_size = table.record_size;
    
    // Pre-populate with some data
    for i in 0..100 {
        let mut data = vec![0u8; record_size];
        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        table.create_record(&data).unwrap();
    }
    
    c.bench_function("mixed_read_write", |b| {
        b.iter(|| {
            // Mixed workload: 80% reads, 20% writes
            for i in 0..100 {
                if i % 5 == 0 {
                    // Write operation
                    let mut data = vec![0u8; record_size];
                    data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                    let result = table.create_record(&data);
                    black_box(result);
                } else {
                    // Read operation
                    let idx = (i % 100) as usize;
                    let result = table.read_record(idx);
                    black_box(result);
                }
            }
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(std::time::Duration::from_secs(3))
        .measurement_time(std::time::Duration::from_secs(10));
    targets = 
        benchmark_baseline_read_throughput,
        benchmark_write_throughput,
        benchmark_concurrent_operations
);

criterion_main!(benches);