//! Memory overhead benchmark for in-memory database.
//!
//! Measures memory usage overhead beyond raw data size.
//! Target: <5% overhead beyond raw data size.
//!
//! Note: Overhead is measured as (buffer_capacity - raw_data_size) / raw_data_size.
//! For small allocations, overhead percentage may be higher due to fixed costs.
//! The benchmark focuses on realistic workloads where raw data size is significant.

use criterion::{criterion_group, criterion_main, Criterion};
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeLayout;
use std::hint::black_box;

/// Creates a test table with exact pre-allocation to avoid growth overhead.
fn create_test_table_exact(
    db: &Database,
    table_name: &str,
    record_size: usize,
    record_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple u8 type layout
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

    // Create multiple u8 fields to achieve the desired record size
    let mut fields = Vec::new();
    for i in 0..record_size {
        fields.push(Field::new(
            format!("field_{}", i),
            "u8".to_string(),
            u8_layout.clone(),
            i,
        ));
    }

    // Create table with exact capacity
    db.create_table(
        table_name.to_string(),
        fields,
        Some(record_count),
        usize::MAX,
    )?;

    let table = db.get_table(table_name)?;

    // Verify table was created with correct record size
    assert_eq!(
        table.record_size, record_size,
        "Table record size {} doesn't match requested {}",
        table.record_size, record_size
    );

    // Pre-allocate exact capacity by creating all records at once
    // This avoids growth overhead
    let mut all_data = Vec::with_capacity(record_size * record_count);
    for i in 0..record_count {
        let mut record_data = vec![0u8; record_size];
        // Fill with simple pattern
        for (j, byte) in record_data.iter_mut().enumerate() {
            *byte = ((i + j) % 256) as u8;
        }
        all_data.extend_from_slice(&record_data);
    }

    // Create all records (simulating bulk insert)
    for chunk in all_data.chunks(record_size) {
        table.create_record(chunk)?;
    }

    Ok(())
}

/// Measures memory usage of a table.
///
/// Returns (buffer_capacity_bytes, actual_data_bytes, overhead_percentage)
fn measure_table_memory(
    db: &Database,
    table_name: &str,
) -> Result<(usize, usize, f64), Box<dyn std::error::Error>> {
    let table = db.get_table(table_name)?;

    // Actual data size in buffer = buffer length
    let actual_data_bytes = table.buffer.len();

    // Buffer capacity (allocated memory)
    let buffer_capacity = table.buffer.capacity();

    // Calculate overhead percentage
    // Overhead = (capacity - actual_data) / actual_data
    let overhead_bytes = buffer_capacity.saturating_sub(actual_data_bytes);
    let overhead_percentage = if actual_data_bytes > 0 {
        (overhead_bytes as f64 / actual_data_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok((buffer_capacity, actual_data_bytes, overhead_percentage))
}

/// Benchmark memory overhead for realistic workloads.
fn bench_memory_overhead_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead_realistic");
    group.sample_size(10);

    // Realistic test scenarios (record size in bytes, record count)
    let test_scenarios = [
        (64, 10_000),  // Small records, many rows
        (256, 5_000),  // Medium records
        (1024, 1_000), // Large records
        (4096, 100),   // Very large records
    ];

    for &(record_size, record_count) in &test_scenarios {
        group.bench_function(
            format!("size_{}_count_{}", record_size, record_count),
            |b| {
                b.iter(|| {
                    let db = Database::new();
                    let table_name = "test_table";

                    // Create table with exact pre-allocation
                    create_test_table_exact(&db, table_name, record_size, record_count)
                        .expect("Failed to create test table");

                    // Measure memory usage
                    let (buffer_capacity, raw_data, overhead) =
                        measure_table_memory(&db, table_name).expect("Failed to measure memory");

                    black_box((buffer_capacity, raw_data, overhead));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_memory_overhead_realistic,);

criterion_main!(benches);
