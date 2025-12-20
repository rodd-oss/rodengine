//! Performance benchmarks for in-memory database.
//!
//! CLI tool for running performance regression tests:
//! - Baseline throughput: >10M reads/sec/core
//! - Write throughput: >1M writes/sec/core
//! - CRUD operation latency
//! - Concurrent read/write throughput
//! - Memory allocation profiles

mod benchmarks;
mod cli;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};

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
            benchmarks::baseline_read::run_baseline_read_test(iterations, record_count);
        }
        Commands::WriteThroughput { iterations } => {
            benchmarks::write_throughput::run_write_throughput_test(iterations);
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
            benchmarks::mixed_workload::run_mixed_workload_test(operations, read_percent);
        }
        Commands::ProcedureScaling {
            record_count,
            cores,
        } => {
            if record_count == 0 {
                eprintln!("Error: record_count must be greater than 0");
                std::process::exit(1);
            }
            benchmarks::procedure_scaling::run_procedure_scaling_test(record_count, &cores);
        }
        Commands::MemoryOverhead {
            record_sizes,
            record_counts,
        } => {
            benchmarks::memory_overhead::run_memory_overhead_test(&record_sizes, &record_counts);
        }
        Commands::CacheContention {
            thread_counts,
            record_sizes,
            operations,
        } => {
            benchmarks::cache_contention::run_cache_contention_test(
                &thread_counts,
                &record_sizes,
                operations,
            );
        }
        Commands::All => {
            utils::run_all_benchmarks();
        }
    }
}
