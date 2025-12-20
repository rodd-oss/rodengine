use crate::utils::parse_comma_separated;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeLayout;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Run cache line contention test
pub fn run_cache_contention_test(
    thread_counts_str: &str,
    record_sizes_str: &str,
    operations: usize,
) {
    println!("Running cache line contention test...");
    println!("Target: Verify false sharing prevention with cache-aligned records");
    println!("Thread counts: {}", thread_counts_str);
    println!("Record sizes: {}", record_sizes_str);
    println!("Operations per thread: {}", operations);

    if operations == 0 {
        eprintln!("Error: operations must be greater than 0");
        std::process::exit(1);
    }

    // Parse thread counts and record sizes
    let thread_counts = parse_comma_separated(thread_counts_str);
    let record_sizes = parse_comma_separated(record_sizes_str);

    if thread_counts.is_empty() || record_sizes.is_empty() {
        eprintln!("Error: No thread counts or record sizes specified");
        std::process::exit(1);
    }

    // Validate all thread counts and record sizes are positive
    for &count in &thread_counts {
        if count == 0 {
            eprintln!("Error: Thread count must be greater than 0");
            std::process::exit(1);
        }
    }
    for &size in &record_sizes {
        if size == 0 {
            eprintln!("Error: Record size must be greater than 0");
            std::process::exit(1);
        }
    }

    let mut all_results = Vec::new();
    let mut failed_tests = Vec::new();

    for &record_size in &record_sizes {
        for &thread_count in &thread_counts {
            println!(
                "\nTesting record_size={}, thread_count={}",
                record_size, thread_count
            );

            // Create database and table
            let db = Arc::new(Database::new());
            let table_name = format!("cache_test_{}_{}", record_size, thread_count);

            // Create table with specified record size
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

            let mut fields = Vec::new();
            for i in 0..record_size {
                fields.push(Field::new(
                    format!("field_{}", i),
                    "u8".to_string(),
                    u8_layout.clone(),
                    i,
                ));
            }

            db.create_table(
                table_name.clone(),
                fields,
                Some(thread_count * 2),
                usize::MAX,
            )
            .expect("Failed to create table");

            let table = db.get_table(&table_name).expect("Failed to get table");
            let actual_record_size = table.record_size;

            // Pre-populate with records for each thread
            println!("  Pre-populating {} records...", thread_count * 2);
            for i in 0..(thread_count * 2) {
                let mut data = vec![0u8; actual_record_size];
                for (j, item) in data.iter_mut().enumerate().take(actual_record_size) {
                    *item = ((i + j) % 256) as u8;
                }
                table.create_record(&data).expect("Failed to create record");
            }

            // Run benchmark
            println!("  Starting benchmark with {} threads...", thread_count);
            let start = Instant::now();

            let mut handles = Vec::new();
            let operations_per_thread = operations / thread_count.max(1);

            for thread_id in 0..thread_count {
                let db_clone = db.clone();
                let table_name_clone = table_name.clone();
                let handle = thread::spawn(move || {
                    let table = db_clone
                        .get_table(&table_name_clone)
                        .expect("Failed to get table in thread");
                    let record_size = table.record_size;

                    for i in 0..operations_per_thread {
                        // Each thread writes to its own record
                        let mut data = vec![0u8; record_size];
                        for (j, item) in data.iter_mut().enumerate().take(record_size) {
                            *item = ((thread_id * operations_per_thread + i + j) % 256) as u8;
                        }
                        table
                            .update_record(thread_id, &data)
                            .expect("Failed to update record");
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().expect("Thread panicked");
            }

            let elapsed = start.elapsed();
            let total_operations = operations_per_thread * thread_count;
            let ops_per_sec = total_operations as f64 / elapsed.as_secs_f64();

            println!("  Results:");
            println!("    Total time: {:?}", elapsed);
            println!("    Total operations: {}", total_operations);
            println!("    Operations per second: {:.2}", ops_per_sec);

            // Store results for analysis
            all_results.push((record_size, thread_count, ops_per_sec, elapsed));

            // For 64-byte records (cache-aligned), we expect good scaling
            // For 63-byte records (misaligned), we might see some degradation
            // We'll analyze after all tests
        }
    }

    // Analyze results for cache line contention
    println!("\n{}", "=".repeat(60));
    println!("Cache Contention Analysis:");
    println!("{}", "-".repeat(60));

    // Group results by thread count
    let mut results_by_threads = std::collections::HashMap::new();
    for (record_size, thread_count, ops_per_sec, elapsed) in &all_results {
        results_by_threads
            .entry(*thread_count)
            .or_insert_with(Vec::new)
            .push((*record_size, *ops_per_sec, *elapsed));
    }

    // Check for false sharing patterns
    let mut has_false_sharing = false;

    for (&thread_count, results) in &results_by_threads {
        println!("\nThread count: {}", thread_count);

        // Find 64-byte and 63-byte results for comparison
        let result_64 = results.iter().find(|&&(size, _, _)| size == 64);
        let result_63 = results.iter().find(|&&(size, _, _)| size == 63);

        if let (Some((_, ops_64, _)), Some((_, ops_63, _))) = (result_64, result_63) {
            let performance_diff = ((ops_64 - ops_63) / ops_64 * 100.0).abs();
            println!("  64-byte (aligned): {:.2} ops/sec", ops_64);
            println!("  63-byte (misaligned): {:.2} ops/sec", ops_63);
            println!("  Performance difference: {:.2}%", performance_diff);

            // If performance degradation > 20%, it suggests false sharing
            if performance_diff > 20.0 && thread_count > 1 {
                println!("  ⚠️  WARNING: Possible false sharing detected!");
                has_false_sharing = true;
                failed_tests.push((64, 63, thread_count, performance_diff));
            } else {
                println!("  ✅ OK: Minimal performance difference");
            }
        }
    }

    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("Cache Contention Test Summary:");
    println!("{}", "-".repeat(60));

    if !has_false_sharing {
        println!("✅ PASS: No significant false sharing detected");
        println!("Cache line contention prevention appears effective.");
    } else {
        println!("❌ FAIL: Possible false sharing detected in some configurations");
        println!("The system may benefit from cache line padding or alignment improvements.");

        for (size1, size2, threads, diff) in failed_tests {
            println!(
                "  Threads={}: {}-byte vs {}-byte diff={:.2}%",
                threads, size1, size2, diff
            );
        }

        std::process::exit(1);
    }
}
