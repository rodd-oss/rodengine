//! Performance benchmarks for in-memory database.
//!
//! CLI tool for running performance regression tests:
//! - Baseline throughput: >10M reads/sec/core
//! - Write throughput: >1M writes/sec/core
//! - CRUD operation latency
//! - Concurrent read/write throughput
//! - Memory allocation profiles

use clap::{Parser, Subcommand};
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};
use std::time::Instant;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run baseline read throughput test (>10M reads/sec/core)
    BaselineRead {
        /// Number of iterations to run
        #[arg(short, long, default_value_t = 100_000)]
        iterations: usize,

        /// Number of pre-populated records
        #[arg(short, long, default_value_t = 1_000_000)]
        record_count: usize,
    },

    /// Run write throughput test (>1M writes/sec/core)
    WriteThroughput {
        /// Number of iterations to run
        #[arg(short, long, default_value_t = 10_000)]
        iterations: usize,
    },

    /// Run mixed read/write workload test
    MixedWorkload {
        /// Total operations to perform
        #[arg(short, long, default_value_t = 100_000)]
        operations: usize,

        /// Read percentage (0-100)
        #[arg(short, long, default_value_t = 80)]
        read_percent: u8,
    },

    /// Run procedure scaling test (>90% efficiency with core count)
    ProcedureScaling {
        /// Number of records to create
        #[arg(short, long, default_value_t = 1_000_000)]
        record_count: usize,

        /// Core counts to test (comma-separated)
        #[arg(short, long, default_value = "1,2,4,8")]
        cores: String,
    },

    /// Run memory overhead test (<5% beyond raw data size)
    MemoryOverhead {
        /// Record sizes to test (comma-separated)
        #[arg(long, default_value = "64,256,1024,4096")]
        record_sizes: String,

        /// Record counts to test (comma-separated)
        #[arg(long, default_value = "1000,5000,10000")]
        record_counts: String,
    },

    /// Run cache line contention test (verify false sharing prevention)
    CacheContention {
        /// Number of writer threads to test (comma-separated)
        #[arg(long, default_value = "1,2,4")]
        thread_counts: String,

        /// Record sizes to test (comma-separated)
        #[arg(long, default_value = "63,64,128")]
        record_sizes: String,

        /// Operations per thread
        #[arg(long, default_value_t = 10_000)]
        operations: usize,
    },

    /// Run all performance regression tests
    All,
}

/// Creates a simple benchmark table
fn create_benchmark_table(db: &Database, table_name: &str, initial_capacity: Option<usize>) {
    let _registry = TypeRegistry::new();

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

    let fields = vec![Field::new(
        "value".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table(table_name.to_string(), fields, initial_capacity, usize::MAX)
        .expect("Failed to create benchmark table");
}

/// Run baseline read throughput test
fn run_baseline_read_test(iterations: usize, record_count: usize) {
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

/// Run write throughput test
fn run_write_throughput_test(iterations: usize) {
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

/// Run mixed workload test
fn run_mixed_workload_test(operations: usize, read_percent: u8) {
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

/// Run procedure scaling test
fn run_procedure_scaling_test(record_count: usize, cores_str: &str) {
    use in_mem_db_core::config::DbConfig;
    use in_mem_db_core::error::DbError;
    use in_mem_db_core::transaction::TransactionHandle;
    use in_mem_db_runtime::Runtime;
    use std::time::{Duration, Instant};
    use tokio::sync::{mpsc, oneshot};

    println!("Running procedure scaling test...");
    println!("Record count: {}, Core counts: {}", record_count, cores_str);

    // Parse core counts
    let mut core_counts: Vec<usize> = cores_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid core count value: must be positive integer")
        })
        .collect();

    if core_counts.is_empty() {
        eprintln!("Error: No core counts specified");
        std::process::exit(1);
    }

    // Validate core counts
    for &cores in &core_counts {
        if cores == 0 {
            eprintln!("Error: Core count must be greater than 0");
            std::process::exit(1);
        }
    }

    if !core_counts.contains(&1) {
        eprintln!("Error: Core counts must include 1 for baseline measurement");
        eprintln!("Please include '1' in the core counts list (e.g., '1,2,4')");
        std::process::exit(1);
    }

    // Sort core counts to ensure baseline measurement first
    core_counts.sort();

    // Create database
    let db = in_mem_db_core::database::Database::new();

    // Create u64 type layout
    let u64_layout = unsafe {
        in_mem_db_core::types::TypeLayout::new(
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
        in_mem_db_core::table::Field::new(
            "id".to_string(),
            "u64".to_string(),
            u64_layout.clone(),
            0,
        ),
        in_mem_db_core::table::Field::new("value".to_string(), "u64".to_string(), u64_layout, 8),
    ];

    db.create_table(
        "scaling_test".to_string(),
        fields,
        Some(record_count),
        usize::MAX,
    )
    .expect("Failed to create test table");

    let table = db
        .get_table("scaling_test")
        .expect("Failed to get table 'scaling_test'");
    let record_size = table.record_size;

    // Pre-populate records
    println!("Pre-populating {} records...", record_count);
    let start_populate = Instant::now();

    let batch_size = 100_000;
    let total_batches = record_count.div_ceil(batch_size);

    for batch in 0..total_batches {
        let start = batch * batch_size;
        let end = (start + batch_size).min(record_count);

        for i in start..end {
            let mut data = vec![0u8; record_size];
            data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            data[8..16].copy_from_slice(&((i * 2) as u64).to_le_bytes());
            table
                .create_record(&data)
                .expect("Failed to create record in scaling_test table");
        }

        if batch % 10 == 0 || batch == total_batches - 1 {
            println!("  Progress: {}/{} records", end, record_count);
        }
    }

    let populate_time = start_populate.elapsed();
    println!("Pre-population complete in {:?}", populate_time);

    // Define simple sum procedure
    fn sum_values_procedure(
        db: &in_mem_db_core::database::Database,
        _tx: &mut TransactionHandle,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, DbError> {
        let table = db
            .get_table("scaling_test")
            .map_err(|e| DbError::SerializationError(e.to_string()))?;
        let record_count = table.record_count();
        let mut sum = 0u64;

        for i in 0..record_count {
            let (record_bytes, _arc) = table
                .read_record(i)
                .map_err(|e| DbError::SerializationError(e.to_string()))?;
            let value_bytes = &record_bytes[8..16];
            let value = u64::from_le_bytes(
                value_bytes
                    .try_into()
                    .expect("Failed to convert bytes to u64: slice length not 8"),
            );
            sum = sum.wrapping_add(value);
        }

        Ok(serde_json::json!({ "sum": sum }))
    }

    // Test each core count
    let mut results = Vec::new();
    let mut base_time = Duration::default();

    for &cores in &core_counts {
        println!("\nTesting with {} cores:", cores);

        let mut total_time = Duration::default();
        let runs = 3; // Run multiple times for averaging

        for run in 0..runs {
            // Create a new database for each run
            let db = in_mem_db_core::database::Database::new();

            // Recreate the table
            let u64_layout = unsafe {
                in_mem_db_core::types::TypeLayout::new(
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
                in_mem_db_core::table::Field::new(
                    "id".to_string(),
                    "u64".to_string(),
                    u64_layout.clone(),
                    0,
                ),
                in_mem_db_core::table::Field::new(
                    "value".to_string(),
                    "u64".to_string(),
                    u64_layout,
                    8,
                ),
            ];

            db.create_table(
                "scaling_test".to_string(),
                fields,
                Some(record_count),
                usize::MAX,
            )
            .expect("Failed to create test table");

            {
                let table = db
                    .get_table("scaling_test")
                    .expect("Failed to get table 'scaling_test'");
                let record_size = table.record_size;

                // Pre-populate records for this run
                let batch_size = 100_000;
                let total_batches = record_count.div_ceil(batch_size);

                for batch in 0..total_batches {
                    let start = batch * batch_size;
                    let end = (start + batch_size).min(record_count);

                    for i in start..end {
                        let mut data = vec![0u8; record_size];
                        data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
                        data[8..16].copy_from_slice(&((i * 2) as u64).to_le_bytes());
                        table
                            .create_record(&data)
                            .expect("Failed to create record in scaling_test table");
                    }
                }
            }

            let db_arc = std::sync::Arc::new(db);
            let (api_tx, api_rx) = mpsc::channel(1000);
            let (persistence_tx, _persistence_rx) = mpsc::channel(100);

            let config = DbConfig {
                tickrate: 1000,
                persistence_interval_ticks: 10,
                max_api_requests_per_tick: 1000,
                initial_table_capacity: 1024,
                data_dir: std::env::temp_dir(),
                procedure_thread_pool_size: 0,
                max_buffer_size: usize::MAX,
                request_timeout_ms: 5000,
                response_timeout_ms: 10000,
                persistence_max_retries: 3,
                persistence_retry_delay_ms: 100,
            };

            let mut runtime = Runtime::new(db_arc, config, api_rx, persistence_tx);
            runtime.register_procedure("sum_values".to_string(), sum_values_procedure);

            // Create multiple concurrent requests for parallel testing
            let mut result_channels = Vec::new();
            for _ in 0..cores {
                let (result_tx, result_rx) = oneshot::channel();
                let request = in_mem_db_runtime::ApiRequest::Rpc {
                    name: "sum_values".to_string(),
                    params: serde_json::json!({}),
                    response: result_tx,
                };
                api_tx
                    .blocking_send(request)
                    .expect("Failed to send API request");
                result_channels.push(result_rx);
            }

            let start = Instant::now();
            runtime.tick().expect("Runtime tick failed");
            let elapsed = start.elapsed();

            // Collect results
            for rx in result_channels {
                let _ = rx.blocking_recv().expect("Failed to receive result");
            }

            total_time += elapsed;
            println!("  Run {}: {:?}", run + 1, elapsed);
        }

        let avg_time = total_time / runs as u32;

        if cores == 1 {
            base_time = avg_time;
        }

        let expected_time = if cores == 1 {
            avg_time
        } else {
            base_time / cores as u32
        };

        let efficiency = if cores == 1 {
            100.0
        } else {
            let avg_secs = avg_time.as_secs_f64();
            let expected_secs = expected_time.as_secs_f64();
            if avg_secs == 0.0 || expected_secs == 0.0 {
                0.0
            } else {
                (expected_secs / avg_secs) * 100.0
            }
        };

        results.push((cores, avg_time, efficiency));
        println!(
            "  Average time: {:?}, Efficiency: {:.1}%",
            avg_time, efficiency
        );
    }

    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("Procedure Scaling Test Results:");
    println!("{}", "-".repeat(60));

    for (cores, time, efficiency) in &results {
        println!(
            "  {} cores: {:?} (efficiency: {:.1}%)",
            cores, time, efficiency
        );
    }

    // Check if all efficiencies are >90%
    let all_pass = results
        .iter()
        .skip(1) // Skip 1-core baseline
        .all(|(_, _, efficiency)| *efficiency > 90.0);

    if all_pass {
        println!("\n✅ PASS: All scaling efficiencies >90%");
    } else {
        println!("\n❌ FAIL: Some scaling efficiencies ≤90%");
        for (cores, _, efficiency) in &results {
            if *cores > 1 && *efficiency <= 90.0 {
                println!(
                    "  {} cores efficiency: {:.1}% (expected >90%)",
                    cores, efficiency
                );
            }
        }
    }
}

/// Run memory overhead test
fn run_memory_overhead_test(record_sizes_str: &str, record_counts_str: &str) {
    println!("Running memory overhead test...");
    println!("Target: <5% overhead beyond raw data size");
    println!("Record sizes: {}", record_sizes_str);
    println!("Record counts: {}", record_counts_str);

    // Parse record sizes and counts
    let record_sizes: Vec<usize> = record_sizes_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid record size: must be positive integer")
        })
        .collect();

    let record_counts: Vec<usize> = record_counts_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid record count: must be positive integer")
        })
        .collect();

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

/// Run cache line contention test
fn run_cache_contention_test(thread_counts_str: &str, record_sizes_str: &str, operations: usize) {
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

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
    let thread_counts: Vec<usize> = thread_counts_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid thread count: must be positive integer")
        })
        .collect();

    let record_sizes: Vec<usize> = record_sizes_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid record size: must be positive integer")
        })
        .collect();

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

/// Run all performance regression tests
fn run_all_tests() {
    println!("Running all performance regression tests...");
    println!("{}", "=".repeat(60));

    // Baseline read test
    println!("\n1. Baseline Read Throughput Test");
    println!("{}", "-".repeat(40));
    run_baseline_read_test(100_000, 1_000_000);

    // Write throughput test
    println!("\n2. Write Throughput Test");
    println!("{}", "-".repeat(40));
    run_write_throughput_test(10_000);

    // Mixed workload test
    println!("\n3. Mixed Workload Test");
    println!("{}", "-".repeat(40));
    run_mixed_workload_test(100_000, 80);

    // Procedure scaling test
    println!("\n4. Procedure Scaling Test");
    println!("{}", "-".repeat(40));
    run_procedure_scaling_test(1_000_000, "1,2,4,8");

    // Memory overhead test
    println!("\n5. Memory Overhead Test");
    println!("{}", "-".repeat(40));
    run_memory_overhead_test("64,256,1024,4096", "1000,5000,10000");

    // Cache contention test
    println!("\n6. Cache Line Contention Test");
    println!("{}", "-".repeat(40));
    run_cache_contention_test("1,2,4", "63,64,128", 10_000);

    println!("\n{}", "=".repeat(60));
    println!("All tests completed.");
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::BaselineRead {
            iterations,
            record_count,
        } => {
            if record_count == 0 {
                eprintln!("Error: record_count must be greater than 0");
                std::process::exit(1);
            }
            run_baseline_read_test(iterations, record_count);
        }
        Commands::WriteThroughput { iterations } => {
            run_write_throughput_test(iterations);
        }
        Commands::MixedWorkload {
            operations,
            read_percent,
        } => {
            if read_percent > 100 {
                eprintln!("Error: read_percent must be between 0 and 100");
                std::process::exit(1);
            }
            if operations == 0 {
                eprintln!("Error: operations must be greater than 0");
                std::process::exit(1);
            }
            run_mixed_workload_test(operations, read_percent);
        }
        Commands::ProcedureScaling {
            record_count,
            cores,
        } => {
            if record_count == 0 {
                eprintln!("Error: record_count must be greater than 0");
                std::process::exit(1);
            }
            run_procedure_scaling_test(record_count, &cores);
        }
        Commands::MemoryOverhead {
            record_sizes,
            record_counts,
        } => {
            run_memory_overhead_test(&record_sizes, &record_counts);
        }
        Commands::CacheContention {
            thread_counts,
            record_sizes,
            operations,
        } => {
            run_cache_contention_test(&thread_counts, &record_sizes, operations);
        }
        Commands::All => {
            run_all_tests();
        }
    }
}
