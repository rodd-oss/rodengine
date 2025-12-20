use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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
