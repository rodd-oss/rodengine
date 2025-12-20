use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::error::DbError;
use in_mem_db_core::table::Field;
use in_mem_db_core::transaction::TransactionHandle;
use in_mem_db_core::types::TypeLayout;
use in_mem_db_runtime::Runtime;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// Run procedure scaling test
pub fn run_procedure_scaling_test(record_count: usize, cores_str: &str) {
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
    let db = Database::new();

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
        db: &Database,
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
            let db = Database::new();

            // Recreate the table
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
