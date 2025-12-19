//! Cache line contention prevention verification benchmark.
//!
//! Performance regression tests:
//! - Cache line contention prevention: No performance degradation from false sharing
//! - Test with 64-byte records (exact cache line size) vs misaligned records
//! - Measure performance degradation from false sharing
//! - Verify no performance degradation when records are cache-aligned

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};
use std::hint::black_box;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Creates a table with records of specified size
fn create_table_with_record_size(
    db: &Database,
    table_name: &str,
    record_size: usize,
    initial_capacity: Option<usize>,
) {
    let _registry = TypeRegistry::new();

    // Create u8 type layout for building variable-sized records
    let u8_layout = unsafe {
        TypeLayout::new(
            "u8".to_string(),
            1,
            1,
            true,
            |src, dst| {
                dst.push(*src);
                1
            },
            |src, dst| {
                if src.is_empty() {
                    return 0;
                }
                *dst = src[0];
                1
            },
            Some(std::any::TypeId::of::<u8>()),
        )
    };

    // Create fields to achieve desired record size
    let mut fields = Vec::new();
    for i in 0..record_size {
        fields.push(Field::new(
            format!("field_{}", i),
            "u8".to_string(),
            u8_layout.clone(),
            i,
        ));
    }

    db.create_table(table_name.to_string(), fields, initial_capacity, usize::MAX)
        .expect("Failed to create benchmark table");
}

/// Benchmark: Single writer thread baseline
fn benchmark_single_writer_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_writer_baseline");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));

    for record_size in [63, 64, 128].iter() {
        group.bench_with_input(
            BenchmarkId::new("record_size", record_size),
            record_size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let db = Database::new();
                    create_table_with_record_size(&db, "single_writer", size, Some(10_000));
                    let table = db.get_table("single_writer").unwrap();
                    let record_size = table.record_size;

                    let start = Instant::now();

                    for i in 0..iters {
                        let mut data = vec![0u8; record_size];
                        for (j, item) in data.iter_mut().enumerate().take(record_size) {
                            *item = ((i as usize + j) % 256) as u8;
                        }
                        let result = table.create_record(&data);
                        let _ = black_box(result);
                    }

                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Two writers on adjacent records (potential false sharing)
fn benchmark_two_writers_adjacent(c: &mut Criterion) {
    let mut group = c.benchmark_group("two_writers_adjacent");
    group.sample_size(15);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for record_size in [63, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("record_size", record_size),
            record_size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let db = Arc::new(Database::new());
                    create_table_with_record_size(&db, "adjacent_writers", size, Some(10_000));

                    // Pre-populate with some records so writers have something to work on
                    let table = db.get_table("adjacent_writers").unwrap();
                    let record_size = table.record_size;

                    // Create 1000 records to start
                    for i in 0..1000 {
                        let mut data = vec![0u8; record_size];
                        for (j, item) in data.iter_mut().enumerate().take(record_size) {
                            *item = ((i + j) % 256) as u8;
                        }
                        table.create_record(&data).unwrap();
                    }

                    let start = Instant::now();

                    // Spawn two threads writing to adjacent records
                    let db1 = db.clone();
                    let db2 = db.clone();

                    let handle1 = thread::spawn(move || {
                        let table = db1.get_table("adjacent_writers").unwrap();
                        let record_size = table.record_size;

                        for i in 0..(iters / 2) as usize {
                            // Write to record 0 (even indices)
                            let mut data = vec![0u8; record_size];
                            for (j, item) in data.iter_mut().enumerate().take(record_size) {
                                *item = ((i * 2 + j) % 256) as u8;
                            }
                            table.update_record(0, &data).unwrap();
                        }
                    });

                    let handle2 = thread::spawn(move || {
                        let table = db2.get_table("adjacent_writers").unwrap();
                        let record_size = table.record_size;

                        for i in 0..(iters / 2) as usize {
                            // Write to record 1 (adjacent to record 0)
                            let mut data = vec![0u8; record_size];
                            for (j, item) in data.iter_mut().enumerate().take(record_size) {
                                *item = ((i * 2 + 1 + j) % 256) as u8;
                            }
                            table.update_record(1, &data).unwrap();
                        }
                    });

                    handle1.join().unwrap();
                    handle2.join().unwrap();

                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Two writers on distant records (no false sharing)
fn benchmark_two_writers_distant(c: &mut Criterion) {
    let mut group = c.benchmark_group("two_writers_distant");
    group.sample_size(15);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for record_size in [63, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("record_size", record_size),
            record_size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let db = Arc::new(Database::new());
                    create_table_with_record_size(&db, "distant_writers", size, Some(10_000));

                    // Pre-populate with many records so writers can work on distant ones
                    let table = db.get_table("distant_writers").unwrap();
                    let record_size = table.record_size;

                    // Create 10000 records to start
                    for i in 0..10000 {
                        let mut data = vec![0u8; record_size];
                        for (j, item) in data.iter_mut().enumerate().take(record_size) {
                            *item = ((i + j) % 256) as u8;
                        }
                        table.create_record(&data).unwrap();
                    }

                    let start = Instant::now();

                    // Spawn two threads writing to distant records (separated by many cache lines)
                    let db1 = db.clone();
                    let db2 = db.clone();

                    let handle1 = thread::spawn(move || {
                        let table = db1.get_table("distant_writers").unwrap();
                        let record_size = table.record_size;

                        for i in 0..(iters / 2) as usize {
                            // Write to record 0
                            let mut data = vec![0u8; record_size];
                            for (j, item) in data.iter_mut().enumerate().take(record_size) {
                                *item = ((i * 2 + j) % 256) as u8;
                            }
                            table.update_record(0, &data).unwrap();
                        }
                    });

                    let handle2 = thread::spawn(move || {
                        let table = db2.get_table("distant_writers").unwrap();
                        let record_size = table.record_size;

                        for i in 0..(iters / 2) as usize {
                            // Write to record 1000 (distant from record 0)
                            let mut data = vec![0u8; record_size];
                            for (j, item) in data.iter_mut().enumerate().take(record_size) {
                                *item = ((i * 2 + 1 + j) % 256) as u8;
                            }
                            table.update_record(1000, &data).unwrap();
                        }
                    });

                    handle1.join().unwrap();
                    handle2.join().unwrap();

                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Four writers on adjacent records (worst-case false sharing)
fn benchmark_four_writers_adjacent(c: &mut Criterion) {
    let mut group = c.benchmark_group("four_writers_adjacent");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for record_size in [63, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("record_size", record_size),
            record_size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let db = Arc::new(Database::new());
                    create_table_with_record_size(&db, "four_writers", size, Some(10_000));

                    // Pre-populate with some records
                    let table = db.get_table("four_writers").unwrap();
                    let record_size = table.record_size;

                    // Create 1000 records to start
                    for i in 0..1000 {
                        let mut data = vec![0u8; record_size];
                        for (j, item) in data.iter_mut().enumerate().take(record_size) {
                            *item = ((i + j) % 256) as u8;
                        }
                        table.create_record(&data).unwrap();
                    }

                    let start = Instant::now();

                    // Spawn four threads writing to adjacent records
                    let mut handles = Vec::new();
                    let iterations_per_thread = (iters / 4) as usize;

                    for thread_id in 0..4 {
                        let db_clone = db.clone();
                        let handle = thread::spawn(move || {
                            let table = db_clone.get_table("four_writers").unwrap();
                            let record_size = table.record_size;

                            for i in 0..iterations_per_thread {
                                // Each thread writes to its own adjacent record
                                let mut data = vec![0u8; record_size];
                                for (j, item) in data.iter_mut().enumerate().take(record_size) {
                                    *item =
                                        ((thread_id * iterations_per_thread + i + j) % 256) as u8;
                                }
                                table.update_record(thread_id, &data).unwrap();
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Performance assertion: Verify cache line contention prevention
fn benchmark_cache_contention_assertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_contention_assertion");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("verify_false_sharing_prevention", |b| {
        b.iter(|| {
            // Test 1: 64-byte records (cache-aligned) - adjacent writers
            let db64 = Arc::new(Database::new());
            create_table_with_record_size(&db64, "test64", 64, Some(10_000));
            let table64 = db64.get_table("test64").unwrap();

            // Pre-populate
            for i in 0..100 {
                let mut data = vec![0u8; 64];
                for (j, item) in data.iter_mut().enumerate().take(64) {
                    *item = ((i + j) % 256) as u8;
                }
                table64.create_record(&data).unwrap();
            }

            let start64 = Instant::now();

            let db64_1 = db64.clone();
            let db64_2 = db64.clone();

            let handle64_1 = thread::spawn(move || {
                let table = db64_1.get_table("test64").unwrap();
                for _ in 0..10_000 {
                    let data = vec![0u8; 64];
                    table.update_record(0, &data).unwrap();
                }
            });

            let handle64_2 = thread::spawn(move || {
                let table = db64_2.get_table("test64").unwrap();
                for _ in 0..10_000 {
                    let data = vec![0u8; 64];
                    table.update_record(1, &data).unwrap();
                }
            });

            handle64_1.join().unwrap();
            handle64_2.join().unwrap();

            let time64 = start64.elapsed();
            let ops_per_sec64 = 20_000.0 / time64.as_secs_f64();

            // Test 2: 63-byte records (misaligned) - adjacent writers
            let db63 = Arc::new(Database::new());
            create_table_with_record_size(&db63, "test63", 63, Some(10_000));
            let table63 = db63.get_table("test63").unwrap();

            // Pre-populate
            for i in 0..100 {
                let mut data = vec![0u8; 63];
                for (j, item) in data.iter_mut().enumerate().take(63) {
                    *item = ((i + j) % 256) as u8;
                }
                table63.create_record(&data).unwrap();
            }

            let start63 = Instant::now();

            let db63_1 = db63.clone();
            let db63_2 = db63.clone();

            let handle63_1 = thread::spawn(move || {
                let table = db63_1.get_table("test63").unwrap();
                for _ in 0..10_000 {
                    let data = vec![0u8; 63];
                    table.update_record(0, &data).unwrap();
                }
            });

            let handle63_2 = thread::spawn(move || {
                let table = db63_2.get_table("test63").unwrap();
                for _ in 0..10_000 {
                    let data = vec![0u8; 63];
                    table.update_record(1, &data).unwrap();
                }
            });

            handle63_1.join().unwrap();
            handle63_2.join().unwrap();

            let time63 = start63.elapsed();
            let ops_per_sec63 = 20_000.0 / time63.as_secs_f64();

            // Calculate performance difference
            let performance_diff = ((ops_per_sec63 - ops_per_sec64) / ops_per_sec64 * 100.0).abs();

            println!("Cache contention test results:");
            println!("  64-byte records (aligned): {:.2} ops/sec", ops_per_sec64);
            println!("  63-byte records (misaligned): {:.2} ops/sec", ops_per_sec63);
            println!("  Performance difference: {:.2}%", performance_diff);

            // Assert that performance degradation is minimal (< 20%)
            // In a well-optimized system with cache line alignment, there should be
            // minimal performance difference between aligned and misaligned records
            assert!(
                performance_diff < 20.0,
                "Cache line contention detected: Performance degradation of {:.2}% (max allowed: 20%)",
                performance_diff
            );

            println!("  ✅ PASS: Cache line contention prevention verified");
        });
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    targets =
        benchmark_single_writer_baseline,
        benchmark_two_writers_adjacent,
        benchmark_two_writers_distant,
        benchmark_four_writers_adjacent,
        benchmark_cache_contention_assertion
);

criterion_main!(benches);
