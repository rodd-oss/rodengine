use crate::utils::create_benchmark_table;
use in_mem_db_core::database::Database;
use std::time::Instant;

/// Run baseline read throughput test
pub fn run_baseline_read_test(iterations: usize, record_count: usize) {
    println!("Running baseline read throughput test...");
    println!(
        "Iterations: {}, Pre-populated records: {}",
        iterations, record_count
    );

    if record_count == 0 {
        eprintln!("Error: record_count must be greater than 0");
        std::process::exit(1);
    }

    let db = Database::new();
    create_benchmark_table(&db, "baseline_read", Some(record_count));

    let table = db
        .get_table("baseline_read")
        .expect("Failed to get table 'baseline_read'");
    let record_size = table.record_size;

    // Pre-populate records
    println!("Pre-populating {} records...", record_count);
    let start_populate = Instant::now();

    let batch_size = 10_000;
    let total_batches = record_count.div_ceil(batch_size);

    for batch in 0..total_batches {
        let start = batch * batch_size;
        let end = (start + batch_size).min(record_count);

        for i in start..end {
            let mut data = vec![0u8; record_size];
            data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            table
                .create_record(&data)
                .expect("Failed to create record in baseline_read table");
        }

        if batch % 10 == 0 || batch == total_batches - 1 {
            println!("  Progress: {}/{} records", end, record_count);
        }
    }

    let populate_time = start_populate.elapsed();
    println!("Pre-population complete in {:?}", populate_time);
    println!("Actual records in table: {}", table.record_count());

    // Run read benchmark
    println!("Starting read benchmark...");
    let start = Instant::now();

    for i in 0..iterations {
        let idx = i % record_count;
        let result = table.read_record(idx);
        let _ = std::hint::black_box(result);
    }

    let elapsed = start.elapsed();
    if elapsed > std::time::Duration::from_secs(30) {
        println!("Warning: test took longer than 30 seconds");
    }
    let reads_per_sec = if elapsed.as_secs_f64() == 0.0 {
        0.0
    } else {
        iterations as f64 / elapsed.as_secs_f64()
    };

    println!("Results:");
    println!("  Total time: {:?}", elapsed);
    println!("  Reads per second: {:.2}", reads_per_sec);
    println!("  Target: >10,000,000 reads/sec/core");

    // Performance assertion
    if reads_per_sec > 10_000_000.0 {
        println!("  ✅ PASS: Baseline read throughput meets target");
    } else {
        println!("  ❌ FAIL: Baseline read throughput below target");
        println!(
            "     Performance: {:.2} reads/sec (target: >10M reads/sec/core)",
            reads_per_sec
        );
    }
}
