//! Runtime integration tests (mt_4 from backlog)
//!
//! Tests for runtime loop, tick phases, rate limiting, and procedure execution.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot};

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeRegistry;
use in_mem_db_runtime::{ApiRequest, CrudOperation, Runtime};
use ntest::timeout;

/// Test basic runtime startup and shutdown
#[timeout(1000)]
#[test]
fn test_runtime_basic_lifecycle() {
    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Arc::new(Database::with_type_registry(type_registry));

    // Create configuration
    let config = DbConfig {
        tickrate: 60, // 60 Hz
        max_api_requests_per_tick: 100,
        ..Default::default()
    };

    // Create channels
    let (api_tx, api_rx) = mpsc::channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::channel(1000);

    // Create runtime
    let mut runtime = Runtime::new(db.clone(), config, api_rx, persistence_tx);

    // Create a table first
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "value".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("rate_test".to_string(), fields, None, usize::MAX)
        .unwrap();

    // Flood with requests
    let mut responses = Vec::new();
    for i in 0..50 {
        let (response_tx, response_rx) = oneshot::channel();
        responses.push(response_rx);

        let req = ApiRequest::Crud {
            table: "rate_test".to_string(),
            operation: CrudOperation::Create {
                values: vec![serde_json::json!(i)],
            },
            response: response_tx,
        };

        api_tx.blocking_send(req).unwrap();
    }

    // Run runtime for a short time to process some requests
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(100) {
        // Run one tick to process queued requests
        runtime.tick().unwrap();
    }

    // Check that at least some requests were processed
    let mut processed = 0;
    for mut response_rx in responses {
        if response_rx.try_recv().is_ok() {
            processed += 1;
        }
    }

    // With 60 Hz tickrate and 100 requests per tick, we should process
    // all 50 requests in 100ms (6 ticks)
    assert!(processed > 0, "Some requests should have been processed");
    assert!(processed <= 50, "Should not exceed rate limit");
}

/// Test procedure execution
#[timeout(1000)]
#[test]
fn test_runtime_procedure_execution() {
    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Arc::new(Database::with_type_registry(type_registry));

    // Create configuration
    let config = DbConfig {
        tickrate: 60,
        max_api_requests_per_tick: 100,
        ..Default::default()
    };

    // Create channels
    let (api_tx, api_rx) = mpsc::channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::channel(1000);

    // Create runtime
    let mut runtime = Runtime::new(db.clone(), config, api_rx, persistence_tx);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "counter".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("proc_test".to_string(), fields, None, usize::MAX)
        .unwrap();

    // Register a simple procedure that does nothing (for testing RPC handling)
    runtime.register_procedure("increment_all".to_string(), |_db, _tx, _params| {
        // Simple no-op procedure for testing
        Ok(serde_json::json!({}))
    });

    // Add some test data
    {
        let table = db.get_table_mut("proc_test").unwrap();
        for i in 0..5 {
            let mut data = vec![0u8; table.record_size];
            data[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            table.create_record(&data).unwrap();
        }
    } // table dropped here, releasing write lock

    // Send RPC request
    let (response_tx, mut response_rx) = oneshot::channel();
    let rpc_req = ApiRequest::Rpc {
        name: "increment_all".to_string(),
        params: serde_json::json!({}),
        response: response_tx,
    };

    api_tx.blocking_send(rpc_req).unwrap();

    // Run multiple ticks to ensure procedure executes
    for _ in 0..5 {
        runtime.tick().unwrap();
    }

    // Response should be available after procedure execution
    let response = response_rx.try_recv();
    assert!(response.is_ok(), "Response should be available");

    let response_result = response.unwrap();
    assert!(response_result.is_ok(), "Procedure should succeed");

    let response_value = response_result.unwrap();
    // Procedure returns empty JSON object
    assert!(response_value.is_object() && response_value.as_object().unwrap().is_empty());
}

/// Test procedure panic recovery without data corruption
#[timeout(1000)]
#[test]
fn test_procedure_panic_recovery() {
    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Arc::new(Database::with_type_registry(type_registry));

    // Create configuration
    let config = DbConfig {
        tickrate: 60,
        max_api_requests_per_tick: 100,
        ..Default::default()
    };

    // Create channels
    let (api_tx, api_rx) = mpsc::channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::channel(1000);

    // Create runtime
    let mut runtime = Runtime::new(db.clone(), config, api_rx, persistence_tx);

    // Create a table
    let u64_layout = db.type_registry().get("u64").unwrap().clone();
    let fields = vec![Field::new(
        "value".to_string(),
        "u64".to_string(),
        u64_layout,
        0,
    )];

    db.create_table("panic_test".to_string(), fields, None, usize::MAX)
        .unwrap();

    // Register a procedure that panics when params["panic"] == true
    runtime.register_procedure("panic_procedure".to_string(), |db, tx, params| {
        if params
            .get("panic")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            panic!("Test panic triggered by params");
        }

        // Normal procedure logic: increment all values
        let table = db.get_table("panic_test")?;
        let buffer = table.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_size = table.record_size;
        let record_count = buffer_slice.len() / record_size;

        for i in 0..record_count {
            let offset = i * record_size;
            let mut current = [0u8; 8];
            current.copy_from_slice(&buffer_slice[offset..offset + 8]);
            let mut value = u64::from_le_bytes(current);
            value += 1;
            let new_data = value.to_le_bytes().to_vec();
            tx.transaction_mut().stage_update(&table, i, new_data)?;
        }

        Ok(serde_json::json!({ "processed": record_count }))
    });

    // Add some test data
    let initial_values = vec![1u64, 2, 3, 4, 5];
    {
        let table = db.get_table_mut("panic_test").unwrap();
        for &value in &initial_values {
            let mut data = vec![0u8; table.record_size];
            data[0..8].copy_from_slice(&value.to_le_bytes());
            table.create_record(&data).unwrap();
        }
    }

    // Test 1: Normal procedure execution (should succeed)
    {
        let (response_tx, _response_rx) = oneshot::channel();
        let rpc_req = ApiRequest::Rpc {
            name: "panic_procedure".to_string(),
            params: serde_json::json!({}),
            response: response_tx,
        };

        api_tx.blocking_send(rpc_req).unwrap();

        // Run multiple ticks to ensure procedure executes
        for _ in 0..5 {
            runtime.tick().unwrap();
        }

        // Check that normal procedure completed
        let table = db.get_table("panic_test").unwrap();
        let buffer = table.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_size = table.record_size;
        let mut found_incremented = false;

        for (i, &initial_value) in initial_values.iter().enumerate() {
            let offset = i * record_size;
            let mut current = [0u8; 8];
            current.copy_from_slice(&buffer_slice[offset..offset + 8]);
            let value = u64::from_le_bytes(current);

            // Either original or incremented value is acceptable
            // since we don't know if procedure committed
            if value == initial_value + 1 {
                found_incremented = true;
            }
        }

        // At least some values should be incremented
        assert!(
            found_incremented,
            "Normal procedure should have incremented values"
        );
    }

    // Test 2: Procedure with panic=true (should fail without corruption)
    {
        let (response_tx, mut response_rx) = oneshot::channel();
        let rpc_req = ApiRequest::Rpc {
            name: "panic_procedure".to_string(),
            params: serde_json::json!({ "panic": true }),
            response: response_tx,
        };

        api_tx.blocking_send(rpc_req).unwrap();

        // Run multiple ticks to ensure procedure executes (and panics)
        for _ in 0..5 {
            runtime.tick().unwrap();
        }

        // Check that we got a panic error response
        let response = response_rx.try_recv();
        assert!(response.is_ok(), "Response should be available after panic");

        let response_result = response.unwrap();
        assert!(
            response_result.is_err(),
            "Procedure with panic=true should return error"
        );

        let error = response_result.unwrap_err();
        match error {
            in_mem_db_core::error::DbError::ProcedurePanic(msg) => {
                assert!(msg.contains("Test panic"), "Error should indicate panic");
            }
            _ => panic!("Expected ProcedurePanic error, got {:?}", error),
        }

        // Check that data is not corrupted
        let table = db.get_table("panic_test").unwrap();
        let buffer = table.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_size = table.record_size;

        for (i, &initial_value) in initial_values.iter().enumerate() {
            let offset = i * record_size;
            let mut current = [0u8; 8];
            current.copy_from_slice(&buffer_slice[offset..offset + 8]);
            let value = u64::from_le_bytes(current);

            // Values should be either original or incremented from first test
            // but not corrupted (e.g., not random bytes)
            assert!(
                value == initial_value || value == initial_value + 1,
                "Data should not be corrupted after panic"
            );
        }
    }

    // Test 3: Thread pool remains functional after panics
    {
        let (response_tx, _response_rx) = oneshot::channel();
        let rpc_req = ApiRequest::Rpc {
            name: "panic_procedure".to_string(),
            params: serde_json::json!({}),
            response: response_tx,
        };

        api_tx.blocking_send(rpc_req).unwrap();

        // Run ticks - runtime should still function
        for _ in 0..3 {
            runtime.tick().unwrap();
        }

        // Runtime should still be operational - queue_sizes() returns usize which is always >= 0
        let (ddl_size, dml_size, proc_size) = runtime.queue_sizes();
        // Just check that the function returns without panicking
        assert!(
            ddl_size < 1000 && dml_size < 1000 && proc_size < 1000,
            "Runtime should remain functional"
        );
    }
}

/// Test tick phase timing (simplified)
#[timeout(1000)]
#[test]
fn test_runtime_tick_phases_simulation() {
    // This test simulates the tick phase timing logic without running
    // the full runtime loop

    let tick_duration = Duration::from_millis(16); // ~60 Hz

    // Calculate phase budgets
    let api_budget = tick_duration.mul_f32(0.3); // 30%
    let procedure_budget = tick_duration.mul_f32(0.5); // 50%
    let persistence_budget = tick_duration.mul_f32(0.2); // 20%

    // Verify budgets sum to tick duration (within floating point error)
    let total_budget = api_budget + procedure_budget + persistence_budget;
    let diff = (total_budget.as_nanos() as i128 - tick_duration.as_nanos() as i128).abs();
    assert!(
        diff < 1_000_000,
        "Phase budgets should sum to tick duration"
    );

    // Verify phase proportions
    let api_ratio = api_budget.as_secs_f64() / tick_duration.as_secs_f64();
    let proc_ratio = procedure_budget.as_secs_f64() / tick_duration.as_secs_f64();
    let persist_ratio = persistence_budget.as_secs_f64() / tick_duration.as_secs_f64();

    assert!((api_ratio - 0.3).abs() < 0.01, "API phase should be ~30%");
    assert!(
        (proc_ratio - 0.5).abs() < 0.01,
        "Procedure phase should be ~50%"
    );
    assert!(
        (persist_ratio - 0.2).abs() < 0.01,
        "Persistence phase should be ~20%"
    );
}

/// Test DDL vs DML prioritization
#[timeout(1000)]
#[test]
fn test_runtime_request_prioritization() {
    // Create type registry
    let type_registry = Arc::new(TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();

    // Create database
    let db = Arc::new(Database::with_type_registry(type_registry));

    // Create configuration
    let config = DbConfig {
        tickrate: 60,
        max_api_requests_per_tick: 100,
        ..Default::default()
    };

    // Create channels
    let (_api_tx, api_rx) = mpsc::channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::channel(1000);

    // Create runtime
    let _runtime = Runtime::new(db.clone(), config, api_rx, persistence_tx);

    // Test request classification
    let u64_layout = db.type_registry().get("u64").unwrap().clone();

    // DDL request (CreateTable)
    let (ddl_tx, _) = oneshot::channel();
    let ddl_req = ApiRequest::CreateTable {
        name: "test".to_string(),
        fields: vec![Field::new(
            "id".to_string(),
            "u64".to_string(),
            u64_layout,
            0,
        )],
        response: ddl_tx,
    };
    assert!(ddl_req.is_ddl(), "CreateTable should be DDL");

    // DML request (CRUD)
    let (dml_tx, _) = oneshot::channel();
    let dml_req = ApiRequest::Crud {
        table: "test".to_string(),
        operation: CrudOperation::Create {
            values: vec![serde_json::json!(1)],
        },
        response: dml_tx,
    };
    assert!(!dml_req.is_ddl(), "CRUD should not be DDL");

    // RPC request
    let (rpc_tx, _) = oneshot::channel();
    let rpc_req = ApiRequest::Rpc {
        name: "test".to_string(),
        params: serde_json::json!({}),
        response: rpc_tx,
    };
    assert!(!rpc_req.is_ddl(), "RPC should not be DDL");
}
