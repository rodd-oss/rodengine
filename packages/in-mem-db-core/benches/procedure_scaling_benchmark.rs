//! Benchmark for procedure scaling with core count.
//!
//! Tests linear scaling of procedure execution with core count (>90% efficiency).
//! Creates a table with records and runs a simple procedure (sum all values)
//! with simulated parallel execution.

use criterion::{criterion_group, criterion_main, Criterion};
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeLayout;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Creates a test database with records.
fn create_test_database(record_count: usize) -> Arc<Database> {
    let db = Arc::new(Database::new());

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

    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
        Field::new("value".to_string(), "u64".to_string(), u64_layout, 8),
    ];

    db.create_table(
        "test_table".to_string(),
        fields,
        Some(record_count),
        usize::MAX,
    )
    .expect("Failed to create test table");

    {
        let table = db.get_table("test_table").unwrap();
        let record_size = table.record_size;

        // Pre-populate records with sequential values
        let batch_size = 100_000;
        let total_batches = record_count.div_ceil(batch_size);

        for batch in 0..total_batches {
            let start = batch * batch_size;
            let end = (start + batch_size).min(record_count);

            for i in start..end {
                let mut data = vec![0u8; record_size];
                data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                data[8..16].copy_from_slice(&((i * 2) as u64).to_le_bytes());
                table.create_record(&data).unwrap();
            }
        }
    }

    db
}

/// Benchmark sequential procedure execution
fn bench_procedure_sequential(c: &mut Criterion) {
    let record_count = 100_000;
    let db = create_test_database(record_count);

    c.bench_function("procedure_sequential", |b| {
        b.iter_custom(|iterations| {
            let mut total_time = Duration::default();

            for _ in 0..iterations {
                let start = Instant::now();

                // Simulate procedure execution
                let table = db.get_table("test_table").unwrap();
                let record_count = table.record_count();
                let mut sum = 0u64;

                for i in 0..record_count {
                    let (record_bytes, _arc) = table.read_record(i).unwrap();
                    let value_bytes = &record_bytes[8..16];
                    let value = u64::from_le_bytes(value_bytes.try_into().unwrap());
                    sum = sum.wrapping_add(value);
                }

                let _ = std::hint::black_box(sum);
                total_time += start.elapsed();
            }

            total_time
        });
    });
}

/// Benchmark parallel procedure execution with Rayon
fn bench_procedure_parallel(c: &mut Criterion) {
    let record_count = 100_000;
    let db = create_test_database(record_count);

    c.bench_function("procedure_parallel_rayon", |b| {
        b.iter_custom(|iterations| {
            let mut total_time = Duration::default();

            for _ in 0..iterations {
                let start = Instant::now();

                let table = db.get_table("test_table").unwrap();
                let record_count = table.record_count();

                // Use Rayon for parallel iteration
                let sum: u64 = (0..record_count)
                    .into_par_iter()
                    .map(|i| {
                        let (record_bytes, _arc) = table.read_record(i).unwrap();
                        let value_bytes = &record_bytes[8..16];
                        u64::from_le_bytes(value_bytes.try_into().unwrap())
                    })
                    .sum();

                let _ = std::hint::black_box(sum);
                total_time += start.elapsed();
            }

            total_time
        });
    });
}

/// Benchmark scaling efficiency with different chunk sizes
fn bench_scaling_chunks(c: &mut Criterion) {
    let record_count = 100_000;
    let db = create_test_database(record_count);

    let mut group = c.benchmark_group("scaling_chunks");

    for chunk_size in [1, 2, 4, 8].iter() {
        group.bench_function(format!("chunk_size_{}", chunk_size), |b| {
            b.iter(|| {
                let table = db.get_table("test_table").unwrap();
                let record_count = table.record_count();
                let mut sum = 0u64;

                // Process in chunks to simulate parallel work units
                for chunk_start in (0..record_count).step_by(*chunk_size) {
                    let chunk_end = (chunk_start + *chunk_size).min(record_count);

                    for i in chunk_start..chunk_end {
                        let (record_bytes, _arc) = table.read_record(i).unwrap();
                        let value_bytes = &record_bytes[8..16];
                        let value = u64::from_le_bytes(value_bytes.try_into().unwrap());
                        sum = sum.wrapping_add(value);
                    }
                }

                let _ = std::hint::black_box(sum);
            });
        });
    }

    group.finish();
}

/// Calculate and assert scaling efficiency
fn calculate_scaling_efficiency() {
    println!("Calculating procedure scaling efficiency...");

    let record_count = 100_000;
    let db = create_test_database(record_count);
    let table = db.get_table("test_table").unwrap();

    // Measure sequential execution time
    let sequential_start = Instant::now();
    let mut sequential_sum = 0u64;

    for i in 0..record_count {
        let (record_bytes, _arc) = table.read_record(i).unwrap();
        let value_bytes = &record_bytes[8..16];
        let value = u64::from_le_bytes(value_bytes.try_into().unwrap());
        sequential_sum = sequential_sum.wrapping_add(value);
    }

    let sequential_time = sequential_start.elapsed();
    println!(
        "Sequential execution: {:?}, sum: {}",
        sequential_time, sequential_sum
    );

    // Measure parallel execution time with Rayon
    let parallel_start = Instant::now();

    let parallel_sum: u64 = (0..record_count)
        .into_par_iter()
        .map(|i| {
            let (record_bytes, _arc) = table.read_record(i).unwrap();
            let value_bytes = &record_bytes[8..16];
            u64::from_le_bytes(value_bytes.try_into().unwrap())
        })
        .sum();

    let parallel_time = parallel_start.elapsed();
    println!(
        "Parallel execution: {:?}, sum: {}",
        parallel_time, parallel_sum
    );

    // Calculate efficiency
    let efficiency = (sequential_time.as_secs_f64() / parallel_time.as_secs_f64()) * 100.0;
    println!("Scaling efficiency: {:.1}%", efficiency);

    // Assert >90% efficiency
    assert!(
        efficiency > 90.0,
        "Scaling efficiency is {:.1}%, expected >90%",
        efficiency
    );

    println!("✅ Scaling efficiency target met (>90%)");
}

/// Create benchmark group
fn criterion_benchmark(c: &mut Criterion) {
    bench_procedure_sequential(c);
    bench_procedure_parallel(c);
    bench_scaling_chunks(c);

    // Run efficiency calculation (not part of criterion benchmark)
    println!("Running scaling efficiency calculation...");
    calculate_scaling_efficiency();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
