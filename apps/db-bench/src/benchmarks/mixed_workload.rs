use crate::utils::create_benchmark_table;
use in_mem_db_core::database::Database;
use std::time::Instant;

/// Run mixed workload test
pub fn run_mixed_workload_test(operations: usize, read_percent: u8) {
    println!("Running mixed workload test...");
    println!(
        "Total operations: {}, Read percentage: {}%",
        operations, read_percent
    );

    if operations == 0 {
        println!("No operations to perform, test skipped.");
        return;
    }

    let db = Database::new();
    create_benchmark_table(&db, "mixed_workload", Some(operations));

    let table = db
        .get_table("mixed_workload")
        .expect("Failed to get table 'mixed_workload'");
    let record_size = table.record_size;

    // Pre-populate some records for reading
    let pre_populate_count = operations / 2;
    println!("Pre-populating {} records...", pre_populate_count);

    if pre_populate_count == 0 {
        println!(
            "Warning: No records pre-populated for reading (operations={})",
            operations
        );
    }

    for i in 0..pre_populate_count {
        let mut data = vec![0u8; record_size];
        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
        table
            .create_record(&data)
            .expect("Failed to create record in mixed_workload table");
    }

    println!("Starting mixed workload benchmark...");
    let start = Instant::now();

    let mut read_ops = (operations as f64 * (read_percent as f64 / 100.0)) as usize;
    if pre_populate_count == 0 && read_ops > 0 {
        println!(
            "Warning: Cannot perform reads with no pre-populated records, setting read_ops to 0"
        );
        read_ops = 0;
    }
    let write_ops = operations - read_ops;

    let mut reads_done = 0;
    let mut writes_done = 0;

    for i in 0..operations {
        if i % 100 < read_percent as usize && reads_done < read_ops {
            // Read operation
            let idx = i % pre_populate_count;
            let result = table.read_record(idx);
            let _ = std::hint::black_box(result);
            reads_done += 1;
        } else if writes_done < write_ops {
            // Write operation
            let idx = pre_populate_count + writes_done;
            let mut data = vec![0u8; record_size];
            data[0..8].copy_from_slice(&(idx as u64).to_le_bytes());
            let result = table.create_record(&data);
            let _ = std::hint::black_box(result);
            writes_done += 1;
        }
    }

    let elapsed = start.elapsed();
    if elapsed > std::time::Duration::from_secs(30) {
        println!("Warning: test took longer than 30 seconds");
    }
    let ops_per_sec = if elapsed.as_secs_f64() == 0.0 {
        0.0
    } else {
        operations as f64 / elapsed.as_secs_f64()
    };

    println!("Results:");
    println!("  Total time: {:?}", elapsed);
    println!("  Operations per second: {:.2}", ops_per_sec);
    println!("  Reads performed: {}", reads_done);
    println!("  Writes performed: {}", writes_done);
    println!("  Final record count: {}", table.record_count());
}
