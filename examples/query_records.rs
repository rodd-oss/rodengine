//! Example demonstrating the GET /tables/{name}/records endpoint with query parameters

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};
use in_mem_db_runtime::{ApiRequest, QueryParams, Runtime};
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create database
    let config = DbConfig::default();
    let db = Arc::new(Database::new(config.clone()));
    
    // Create type registry with basic types
    let type_registry = db.type_registry();
    
    // Register u64 type
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
    type_registry.register_type("u64".to_string(), u64_layout)?;
    
    // Register string type (simplified for example)
    let string_layout = unsafe {
        TypeLayout::new(
            "string".to_string(),
            260, // Fixed size: 4 bytes length + 256 bytes data
            1,
            false,
            |src, dst| {
                let string_ptr = src as *const String;
                let string = &*string_ptr;
                let bytes = string.as_bytes();
                let len = bytes.len().min(256);
                
                // Write length as u32 (4 bytes)
                dst.extend_from_slice(&(len as u32).to_ne_bytes());
                
                // Write string bytes
                dst.extend_from_slice(&bytes[..len]);
                
                // Pad with zeros
                let padding = 256 - len;
                if padding > 0 {
                    dst.extend(std::iter::repeat_n(0u8, padding));
                }
                
                260
            },
            |src, dst| {
                if src.len() < 260 {
                    return 0;
                }
                // Read length (first 4 bytes)
                let mut len_bytes = [0u8; 4];
                len_bytes.copy_from_slice(&src[..4]);
                let len = u32::from_ne_bytes(len_bytes) as usize;
                
                let actual_len = len.min(256);
                let dst_ptr = dst as *mut String;
                let bytes = &src[4..4 + actual_len];
                *dst_ptr = String::from_utf8_lossy(bytes).to_string();
                
                260
            },
            None,
        )
    };
    type_registry.register_type("string".to_string(), string_layout)?;
    
    // Register bool type
    let bool_layout = unsafe {
        TypeLayout::new(
            "bool".to_string(),
            1,
            1,
            true,
            |src, dst| {
                let bool_ptr = src as *const bool;
                dst.push(if *bool_ptr { 1 } else { 0 });
                1
            },
            |src, dst| {
                if src.is_empty() {
                    return 0;
                }
                let dst_ptr = dst as *mut bool;
                *dst_ptr = src[0] != 0;
                1
            },
            Some(std::any::TypeId::of::<bool>()),
        )
    };
    type_registry.register_type("bool".to_string(), bool_layout)?;
    
    // Create table with fields
    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), type_registry.get("u64").unwrap().clone(), 0),
        Field::new("name".to_string(), "string".to_string(), type_registry.get("string").unwrap().clone(), 8),
        Field::new("active".to_string(), "bool".to_string(), type_registry.get("bool").unwrap().clone(), 268),
    ];
    
    // Create runtime
    let (api_tx, api_rx) = mpsc::channel();
    let (persistence_tx, _persistence_rx) = mpsc::sync_channel(100);
    let mut runtime = Runtime::new(db.clone(), config, api_rx, persistence_tx);
    
    // Start runtime in background thread
    let runtime_handle = thread::spawn(move || {
        runtime.run().unwrap();
    });
    
    // Create table via API
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
    let create_table_request = ApiRequest::CreateTable {
        name: "users".to_string(),
        fields,
        response: response_tx,
    };
    
    api_tx.blocking_send(create_table_request).unwrap();
    let table_response = response_rx.blocking_recv().unwrap().unwrap();
    println!("Created table: {:?}", table_response);
    
    // Create some records
    for i in 0..10 {
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
        let values = vec![
            serde_json::Value::Number((i as u64 + 1).into()),
            serde_json::Value::String(format!("User{}", i + 1)),
            serde_json::Value::Bool(i % 2 == 0), // Even IDs are active
        ];
        
        let create_record_request = ApiRequest::Crud {
            table: "users".to_string(),
            operation: in_mem_db_runtime::CrudOperation::Create { values },
            response: response_tx,
        };
        
        api_tx.blocking_send(create_record_request).unwrap();
        let record_response = response_rx.blocking_recv().unwrap().unwrap();
        println!("Created record {}: {:?}", i + 1, record_response);
    }
    
    // Query records with limit
    println!("\n--- Query with limit=3 ---");
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
    let query_params = QueryParams {
        limit: Some(3),
        offset: None,
        filters: std::collections::HashMap::new(),
    };
    
    let query_request = ApiRequest::QueryRecords {
        table: "users".to_string(),
        query: query_params,
        response: response_tx,
    };
    
    api_tx.blocking_send(query_request).unwrap();
    let query_response = response_rx.blocking_recv().unwrap().unwrap();
    println!("Query response: {}", serde_json::to_string_pretty(&query_response).unwrap());
    
    // Query records with offset
    println!("\n--- Query with offset=5 ---");
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
    let query_params = QueryParams {
        limit: None,
        offset: Some(5),
        filters: std::collections::HashMap::new(),
    };
    
    let query_request = ApiRequest::QueryRecords {
        table: "users".to_string(),
        query: query_params,
        response: response_tx,
    };
    
    api_tx.blocking_send(query_request).unwrap();
    let query_response = response_rx.blocking_recv().unwrap().unwrap();
    println!("Query response: {}", serde_json::to_string_pretty(&query_response).unwrap());
    
    // Query records with filter (active=true)
    println!("\n--- Query with filter active=true ---");
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
    let mut filters = std::collections::HashMap::new();
    filters.insert("active".to_string(), serde_json::Value::Bool(true));
    
    let query_params = QueryParams {
        limit: None,
        offset: None,
        filters,
    };
    
    let query_request = ApiRequest::QueryRecords {
        table: "users".to_string(),
        query: query_params,
        response: response_tx,
    };
    
    api_tx.blocking_send(query_request).unwrap();
    let query_response = response_rx.blocking_recv().unwrap().unwrap();
    println!("Query response: {}", serde_json::to_string_pretty(&query_response).unwrap());
    
    // Query records with filter (name contains "User1")
    println!("\n--- Query with filter name=User1 ---");
    let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
    let mut filters = std::collections::HashMap::new();
    filters.insert("name".to_string(), serde_json::Value::String("User1".to_string()));
    
    let query_params = QueryParams {
        limit: None,
        offset: None,
        filters,
    };
    
    let query_request = ApiRequest::QueryRecords {
        table: "users".to_string(),
        query: query_params,
        response: response_tx,
    };
    
    api_tx.blocking_send(query_request).unwrap();
    let query_response = response_rx.blocking_recv().unwrap().unwrap();
    println!("Query response: {}", serde_json::to_string_pretty(&query_response).unwrap());
    
    // Clean up
    drop(api_tx); // This will cause the runtime to exit
    runtime_handle.join().unwrap();
    
    Ok(())
}