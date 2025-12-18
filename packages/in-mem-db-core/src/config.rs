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
        }
    }
}
