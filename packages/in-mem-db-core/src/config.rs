//! Database configuration.

use std::path::PathBuf;

/// Database configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Tick rate in Hz (15-120)
    pub tickrate: u32,
    /// Persistence interval in ticks
    pub persistence_interval_ticks: u32,
    /// Maximum API requests per tick
    pub max_api_requests_per_tick: u32,
    /// Initial table capacity in records
    pub initial_table_capacity: usize,
    /// Data directory for persistence
    pub data_dir: PathBuf,
    /// Procedure thread pool size (0 = num_cpus)
    pub procedure_thread_pool_size: usize,
    /// Maximum buffer size per table in bytes (default: unlimited)
    pub max_buffer_size: usize,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Response timeout in milliseconds
    pub response_timeout_ms: u64,
    /// Maximum retry attempts for transient I/O errors
    pub persistence_max_retries: u32,
    /// Delay between retry attempts in milliseconds
    pub persistence_retry_delay_ms: u64,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            tickrate: 60,
            persistence_interval_ticks: 10,
            max_api_requests_per_tick: 600,
            initial_table_capacity: 1024,
            data_dir: PathBuf::from("./data"),
            procedure_thread_pool_size: 0,
            max_buffer_size: usize::MAX,
            request_timeout_ms: 5000,        // 5 seconds default
            response_timeout_ms: 10000,      // 10 seconds default
            persistence_max_retries: 3,      // Default retry attempts
            persistence_retry_delay_ms: 100, // 100ms delay between retries
        }
    }
}
