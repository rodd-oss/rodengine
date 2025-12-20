use crate::utils::parse_comma_separated;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeLayout;

/// Run memory overhead test
pub fn run_memory_overhead_test(record_sizes_str: &str, record_counts_str: &str) {
    println!("Running memory overhead test...");
    println!("Target: <5% overhead beyond raw data size");
    println!("Record sizes: {}", record_sizes_str);
    println!("Record counts: {}", record_counts_str);

    // Parse record sizes and counts
    let record_sizes = parse_comma_separated(record_sizes_str);
    let record_counts = parse_comma_separated(record_counts_str);

    if record_sizes.is_empty() || record_counts.is_empty() {
        eprintln!("Error: No record sizes or counts specified");
        std::process::exit(1);
    }

    // Validate all sizes and counts are positive
    for &size in &record_sizes {
        if size == 0 {
            eprintln!("Error: Record size must be greater than 0");
            std::process::exit(1);
        }
    }
    for &count in &record_counts {
        if count == 0 {
            eprintln!("Error: Record count must be greater than 0");
            std::process::exit(1);
        }
    }

    let mut all_results = Vec::new();
    let mut failed_tests = Vec::new();

    for &record_size in &record_sizes {
        for &record_count in &record_counts {
            println!(
                "\nTesting record_size={}, record_count={}",
                record_size, record_count
            );

            let db = Database::new();
            let table_name = format!("test_{}_{}", record_size, record_count);

            // Create table
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

            db.create_table(table_name.clone(), fields, Some(record_count), usize::MAX)
                .expect("Failed to create table");

            let table = db.get_table(&table_name).expect("Failed to get table");

            // Fill table with data
            println!("  Populating {} records...", record_count);
            let start_populate = std::time::Instant::now();

            let batch_size = 1000;
            let total_batches = record_count.div_ceil(batch_size);

            for batch in 0..total_batches {
                let start = batch * batch_size;
                let end = (start + batch_size).min(record_count);

                for i in start..end {
                    let mut data = vec![0u8; record_size];
                    for (j, item) in data.iter_mut().enumerate().take(record_size) {
                        *item = ((i + j) % 256) as u8;
                    }
                    table.create_record(&data).expect("Failed to create record");
                }

                if batch % 10 == 0 || batch == total_batches - 1 {
                    println!("    Progress: {}/{} records", end, record_count);
                }
            }

            let populate_time = start_populate.elapsed();
            println!("  Population complete in {:?}", populate_time);

            // Measure memory
            let raw_data_bytes = table.record_size * table.record_count();
            let total_memory_bytes = table.buffer.capacity();
            let overhead_bytes = total_memory_bytes.saturating_sub(raw_data_bytes);
            let overhead_percentage = if raw_data_bytes > 0 {
                (overhead_bytes as f64 / raw_data_bytes as f64) * 100.0
            } else {
                0.0
            };

            println!("  Results:");
            println!("    Raw data size: {} bytes", raw_data_bytes);
            println!("    Total memory: {} bytes", total_memory_bytes);
            println!(
                "    Overhead: {} bytes ({}%)",
                overhead_bytes, overhead_percentage
            );

            // Check if overhead is within target
            if overhead_percentage < 5.0 {
                println!("    ✅ PASS: Overhead <5%");
                all_results.push((record_size, record_count, overhead_percentage, true));
            } else {
                println!("    ❌ FAIL: Overhead ≥5% (target: <5%)");
                all_results.push((record_size, record_count, overhead_percentage, false));
                failed_tests.push((record_size, record_count, overhead_percentage));
            }
        }
    }

    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("Memory Overhead Test Summary:");
    println!("{}", "-".repeat(60));

    for (record_size, record_count, overhead, passed) in &all_results {
        let status = if *passed { "✅ PASS" } else { "❌ FAIL" };
        println!(
            "  {}: size={}, count={}, overhead={:.2}%",
            status, record_size, record_count, overhead
        );
    }

    if failed_tests.is_empty() {
        println!("\n✅ ALL TESTS PASSED: All memory overheads <5%");
    } else {
        println!("\n❌ SOME TESTS FAILED:");
        for (size, count, overhead) in failed_tests {
            println!(
                "  size={}, count={}, overhead={:.2}%",
                size, count, overhead
            );
        }
        std::process::exit(1);
    }
}
