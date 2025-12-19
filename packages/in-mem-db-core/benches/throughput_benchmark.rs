//! Throughput benchmarks for in-memory database core.
//!
//! Performance regression tests:
//! - Baseline throughput: >10M reads/sec/core
//! - Write throughput: >1M writes/sec/core

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};
use rand::Rng;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Creates a simple benchmark table with u64 field.
fn create_simple_table(db: &Database, table_name: &str, initial_capacity: Option<usize>) {
    let _registry = TypeRegistry::new();

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
    let fields = vec![Field::new(
        "value".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table(table_name.to_string(), fields, initial_capacity, usize::MAX)
        .expect("Failed to create benchmark table");
}

/// Benchmark: Single-threaded read throughput
fn benchmark_single_thread_read_throughput(c: &mut Criterion) {
    let db = Database::new();
    create_simple_table(&db, "read_bench", Some(1_000_000));

    let table = db.get_table("read_bench").unwrap();
    let record_size = table.record_size;

    // Pre-populate with 1M records
    println!("Pre-populating 1M records for read benchmark...");
    let batch_size = 10_000;
    for batch in 0..100 {
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let mut data = vec![0u8; record_size];
            data[0..8].copy_from_slice(&(idx as u64).to_le_bytes());
            table.create_record(&data).unwrap();
        }
    }
    println!(
        "Pre-population complete. Total records: {}",
        table.record_count()
    );

    let mut group = c.benchmark_group("single_thread_read_throughput");
    group.sample_size(50);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("sequential_reads", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();

            for i in 0..iters {
                let idx = (i % 1_000_000) as usize;
                let result = table.read_record(idx);
                let _ = black_box(result);
            }

            start.elapsed()
        })
    });

    group.bench_function("random_reads", |b| {
        let mut rng = rand::thread_rng();

        b.iter_custom(|iters| {
            let start = Instant::now();

            for _ in 0..iters {
                let idx = rng.gen_range(0..1_000_000);
                let result = table.read_record(idx);
                let _ = black_box(result);
            }

            start.elapsed()
        })
    });

    group.finish();

    // Performance assertion
    let mut assertion_group = c.benchmark_group("performance_assertions");
    assertion_group.bench_function("assert_baseline_read_throughput", |b| {
        b.iter(|| {
            let start = Instant::now();
            let iterations = 100_000;

            for i in 0..iterations {
                let idx = (i % 1_000_000) as usize;
                let result = table.read_record(idx);
                let _ = black_box(result);
            }

            let elapsed = start.elapsed();
            let reads_per_sec = iterations as f64 / elapsed.as_secs_f64();

            // Assert >10M reads/sec/core
            assert!(
                reads_per_sec > 10_000_000.0,
                "Baseline read throughput too low: {:.2} reads/sec (target: >10M reads/sec/core)",
                reads_per_sec
            );

            println!("Read throughput: {:.2} reads/sec", reads_per_sec);
        });
    });
    assertion_group.finish();
}

/// Benchmark: Single-threaded write throughput
fn benchmark_single_thread_write_throughput(c: &mut Criterion) {
    let db = Database::new();
    create_simple_table(&db, "write_bench", Some(10_000_000));

    let table = db.get_table("write_bench").unwrap();
    let record_size = table.record_size;

    let mut group = c.benchmark_group("single_thread_write_throughput");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for batch_size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_writes", batch_size),
            batch_size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let start = Instant::now();

                    for batch in 0..iters {
                        for i in 0..size {
                            let idx = batch * size + i;
                            let mut data = vec![0u8; record_size];
                            data[0..8].copy_from_slice(&idx.to_le_bytes());
                            let result = table.create_record(&data);
                            let _ = black_box(result);
                        }
                    }

                    start.elapsed()
                })
            },
        );
    }

    group.finish();

    // Performance assertion
    let mut assertion_group = c.benchmark_group("performance_assertions");
    assertion_group.bench_function("assert_write_throughput", |b| {
        b.iter(|| {
            let start = Instant::now();
            let iterations = 10_000;

            for i in 0..iterations {
                let mut data = vec![0u8; record_size];
                data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                let result = table.create_record(&data);
                let _ = black_box(result);
            }

            let elapsed = start.elapsed();
            let writes_per_sec = iterations as f64 / elapsed.as_secs_f64();

            // Assert >1M writes/sec/core
            assert!(
                writes_per_sec > 1_000_000.0,
                "Write throughput too low: {:.2} writes/sec (target: >1M writes/sec/core)",
                writes_per_sec
            );

            println!("Write throughput: {:.2} writes/sec", writes_per_sec);
        });
    });
    assertion_group.finish();
}

/// Benchmark: Parallel read throughput (requires parallel feature)
#[cfg(feature = "parallel")]
fn benchmark_parallel_read_throughput(c: &mut Criterion) {
    use rayon::prelude::*;

    let db = Database::new();
    create_simple_table(&db, "parallel_read_bench", Some(1_000_000));

    let table = db.get_table("parallel_read_bench").unwrap();

    // Pre-populate with 1M records
    println!("Pre-populating 1M records for parallel read benchmark...");
    let batch_size = 10_000;
    for batch in 0..100 {
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let mut data = vec![0u8; table.record_size];
            data[0..8].copy_from_slice(&(idx as u64).to_le_bytes());
            table.create_record(&data).unwrap();
        }
    }
    println!(
        "Pre-population complete. Total records: {}",
        table.record_count()
    );

    let mut group = c.benchmark_group("parallel_read_throughput");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &threads| {
                b.iter_custom(|iters| {
                    let start = Instant::now();

                    // Use Rayon's thread pool
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap();

                    pool.install(|| {
                        (0..iters).into_par_iter().for_each(|i| {
                            let idx = (i % 1_000_000) as usize;
                            let result = table.read_record(idx);
                            let _ = black_box(result);
                        });
                    });

                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory usage and allocation patterns
fn benchmark_memory_usage(c: &mut Criterion) {
    let db = Database::new();
    create_simple_table(&db, "memory_bench", Some(1000));

    let table = db.get_table("memory_bench").unwrap();
    let record_size = table.record_size;

    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));

    for record_count in [1_000, 10_000, 100_000, 1_000_000].iter() {
        group.bench_with_input(
            BenchmarkId::new("records", record_count),
            record_count,
            |b, &count| {
                b.iter_custom(|_iters| {
                    let start = Instant::now();

                    // Clear table first
                    let db = Database::new();
                    create_simple_table(&db, "temp_memory_bench", Some(count));
                    let table = db.get_table("temp_memory_bench").unwrap();

                    for i in 0..count {
                        let mut data = vec![0u8; record_size];
                        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                        table.create_record(&data).unwrap();
                    }

                    let elapsed = start.elapsed();

                    // Measure memory usage
                    let buffer = table.buffer.load();
                    let memory_used = buffer.len();
                    let memory_per_record = memory_used as f64 / count as f64;

                    println!(
                        "Records: {}, Memory: {} bytes, Bytes/record: {:.2}",
                        count, memory_used, memory_per_record
                    );

                    // Assert memory efficiency
                    assert!(
                        memory_per_record <= (record_size as f64 * 1.1),
                        "Memory usage too high: {:.2} bytes/record (expected: ~{} bytes)",
                        memory_per_record,
                        record_size
                    );

                    elapsed
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(50)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets =
        benchmark_single_thread_read_throughput,
        benchmark_single_thread_write_throughput,
        benchmark_memory_usage
);

#[cfg(feature = "parallel")]
criterion_group!(
    name = parallel_benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = benchmark_parallel_read_throughput
);

#[cfg(feature = "parallel")]
criterion_main!(benches, parallel_benches);

#[cfg(not(feature = "parallel"))]
criterion_main!(benches);
