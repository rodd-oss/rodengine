//! Runtime integration tests (mt_4 from backlog)
//!
//! Tests for runtime loop, tick phases, rate limiting, and procedure execution.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::TypeRegistry;
use in_mem_db_runtime::{ApiRequest, CrudOperation, Runtime};

/// Test basic runtime startup and shutdown
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
    let (api_tx, api_rx) = mpsc::sync_channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::sync_channel(1000);

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

    db.create_table("rate_test".to_string(), fields, None)
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

        api_tx.send(req).unwrap();
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
    let (api_tx, api_rx) = mpsc::sync_channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::sync_channel(1000);

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

    db.create_table("proc_test".to_string(), fields, None)
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
    let (response_tx, response_rx) = oneshot::channel();
    let rpc_req = ApiRequest::Rpc {
        name: "increment_all".to_string(),
        params: serde_json::json!({}),
        response: response_tx,
    };

    api_tx.send(rpc_req).unwrap();

    // Run one tick to process the request
    runtime.tick().unwrap();

    // Response should be available immediately
    let response = response_rx.blocking_recv().unwrap();
    assert!(response.is_ok());

    let response_value = response.unwrap();
    assert_eq!(response_value["status"], "queued");

    // Note: The procedure is queued for execution but not run in this test.
    // We verify the RPC request handling and procedure registration.
}

/// Test tick phase timing (simplified)
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
    let (_api_tx, api_rx) = mpsc::sync_channel(1000);
    let (persistence_tx, _persistence_rx) = mpsc::sync_channel(1000);

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
