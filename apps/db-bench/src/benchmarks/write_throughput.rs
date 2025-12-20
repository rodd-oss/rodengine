use crate::utils::create_benchmark_table;
use in_mem_db_core::database::Database;
use std::time::Instant;

/// Run write throughput test
pub fn run_write_throughput_test(iterations: usize) {
    println!("Running write throughput test...");
    println!("Iterations: {}", iterations);

    let db = Database::new();
    create_benchmark_table(&db, "write_throughput", Some(iterations * 2));

    let table = db
        .get_table("write_throughput")
        .expect("Failed to get table 'write_throughput'");
    let record_size = table.record_size;

    println!("Starting write benchmark...");
    let start = Instant::now();

    for i in 0..iterations {
        let mut data = vec![0u8; record_size];
        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        let result = table.create_record(&data);
        let _ = std::hint::black_box(result);
    }

    let elapsed = start.elapsed();
    if elapsed > std::time::Duration::from_secs(30) {
        println!("Warning: test took longer than 30 seconds");
    }
    let writes_per_sec = if elapsed.as_secs_f64() == 0.0 {
        0.0
    } else {
        iterations as f64 / elapsed.as_secs_f64()
    };

    println!("Results:");
    println!("  Total time: {:?}", elapsed);
    println!("  Writes per second: {:.2}", writes_per_sec);
    println!("  Target: >1,000,000 writes/sec/core");

    // Performance assertion
    if writes_per_sec > 1_000_000.0 {
        println!("  ✅ PASS: Write throughput meets target");
    } else {
        println!("  ❌ FAIL: Write throughput below target");
        println!(
            "     Performance: {:.2} writes/sec (target: >1M writes/sec/core)",
            writes_per_sec
        );
    }
}
