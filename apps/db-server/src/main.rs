//! Main REST API server for in-memory database.
//!
//! Integrates core storage engine, runtime loop, and REST API
//! with configuration parsing and graceful shutdown.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

use clap::Parser;
use in_mem_db_api::{router::Router, server::Server};
use in_mem_db_core::{config::DbConfig, persistence::PersistenceManager};
use in_mem_db_runtime::{FlushCommand, Runtime};
use tokio::signal;

/// Command-line arguments for the database server.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Tick rate for runtime loop (Hz)
    #[arg(long, default_value_t = 60)]
    tickrate: u32,

    /// Data directory for persistence
    #[arg(long, default_value = "./data")]
    data_dir: String,

    /// Request timeout in milliseconds
    #[arg(long, default_value_t = 5000)]
    request_timeout_ms: u64,

    /// Response timeout in milliseconds
    #[arg(long, default_value_t = 10000)]
    response_timeout_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt::init();

    // Create configuration
    let config = Arc::new(DbConfig {
        tickrate: args.tickrate,
        persistence_interval_ticks: 10, // Default value
        max_api_requests_per_tick: 600, // Default value
        initial_table_capacity: 1024,   // Default value
        data_dir: PathBuf::from(&args.data_dir),
        procedure_thread_pool_size: 0, // Default value (num_cpus)
        max_buffer_size: usize::MAX,   // Default value (unlimited)
        request_timeout_ms: args.request_timeout_ms,
        response_timeout_ms: args.response_timeout_ms,
        persistence_max_retries: 3,      // Default value
        persistence_retry_delay_ms: 100, // Default value
    });

    // Create persistence manager
    let persistence = PersistenceManager::new(&config);

    // Load or create database
    let type_registry = Arc::new(in_mem_db_core::types::TypeRegistry::new());
    in_mem_db_core::types::register_builtin_types(&type_registry).unwrap();
    let db = match persistence.load_schema(type_registry) {
        Ok(db) => db,
        Err(in_mem_db_core::error::DbError::DataCorruption(msg)) => {
            tracing::error!("Schema corruption detected: {}", msg);
            tracing::error!("Database cannot start. Please restore from backup or repair schema.");
            std::process::exit(1);
        }
        Err(e) => return Err(format!("Failed to load database: {}", e).into()),
    };

    // Load table data
    let table_names = db.table_names();
    for table_name in table_names {
        let table = db
            .get_table(&table_name)
            .map_err(|e| format!("Failed to get table {}: {}", table_name, e))?;
        persistence
            .load_table_data(&table)
            .map_err(|e| format!("Failed to load data for table {}: {}", table_name, e))?;
    }

    let db = Arc::new(db);

    // Create channels for runtime communication
    let (api_tx, api_rx) = mpsc::channel(1000);
    let (persistence_tx, mut persistence_rx) = mpsc::channel(1000);

    // Spawn persistence thread
    let persistence_config = config.clone();
    let persistence_db = db.clone();
    thread::spawn(move || {
        let persistence = PersistenceManager::new(&persistence_config);

        // Create a simple tick loop for the persistence thread
        let tick_duration =
            std::time::Duration::from_secs_f64(1.0 / persistence_config.tickrate as f64);

        // Metrics for monitoring
        let mut total_flush_commands = 0u64;
        let mut successful_flushes = 0u64;
        let mut failed_flushes = 0u64;
        let mut last_metrics_log = std::time::Instant::now();

        loop {
            let tick_start = std::time::Instant::now();

            // Process any pending flush commands
            while let Ok(cmd) = persistence_rx.try_recv() {
                total_flush_commands += 1;
                match cmd {
                    FlushCommand::FlushAll => {
                        tracing::debug!("Received FlushAll command");
                        if let Err(e) = persistence.flush_all_tables(&persistence_db) {
                            tracing::error!("Failed to flush all tables: {}", e);
                            failed_flushes += 1;
                        } else {
                            successful_flushes += 1;
                            tracing::debug!("Successfully flushed all tables");
                        }
                    }
                    FlushCommand::FlushTable(table_name) => {
                        tracing::debug!("Received FlushTable command for {}", table_name);
                        match persistence_db.get_table(&table_name) {
                            Ok(table) => {
                                if let Err(e) = persistence.flush_table_data(&table) {
                                    tracing::error!("Failed to flush table {}: {}", table_name, e);
                                    failed_flushes += 1;
                                } else {
                                    successful_flushes += 1;
                                    tracing::debug!("Successfully flushed table {}", table_name);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Table {} not found for flush: {}", table_name, e);
                                failed_flushes += 1;
                            }
                        }
                    }
                }
            }

            // Call PersistenceManager.tick() to handle periodic flushing
            if let Err(e) = persistence.tick(&persistence_db) {
                tracing::error!("Persistence tick failed: {}", e);
                failed_flushes += 1;
            } else {
                // tick() returns Ok(()) even if no flush occurred
                // We only count successful flushes when they actually happen
            }

            // Log metrics periodically (every 60 seconds)
            if last_metrics_log.elapsed() > std::time::Duration::from_secs(60) {
                tracing::info!(
                    "Persistence metrics: total_commands={}, successful_flushes={}, failed_flushes={}",
                    total_flush_commands,
                    successful_flushes,
                    failed_flushes
                );
                last_metrics_log = std::time::Instant::now();
            }

            // Sleep for remainder of tick
            if let Some(remaining) = tick_duration.checked_sub(tick_start.elapsed()) {
                std::thread::sleep(remaining);
            }
        }
    });

    // Create runtime
    let mut runtime = Runtime::new(db.clone(), (*config).clone(), api_rx, persistence_tx);
    // Spawn runtime thread
    thread::spawn(move || {
        if let Err(e) = runtime.run() {
            tracing::error!("Runtime loop fatal error: {}", e);
            std::process::exit(1);
        }
    });

    // Create router with API sender
    let router = Router::new(db, config, api_tx);

    // Create server
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let server = Server::new(addr, router);

    println!("Starting in-memory database server...");
    println!("  Host: {}", args.host);
    println!("  Port: {}", args.port);
    println!("  Tickrate: {} Hz", args.tickrate);
    println!("  Data directory: {}", args.data_dir);
    println!("  Request timeout: {} ms", args.request_timeout_ms);
    println!("  Response timeout: {} ms", args.response_timeout_ms);

    // Start server with graceful shutdown
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.serve().await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
    println!("\nShutting down server...");
    server_handle.abort();

    Ok(())
}
