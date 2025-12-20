use in_mem_db_core::database::Database;
use in_mem_db_core::table::Field;
use in_mem_db_core::types::{TypeLayout, TypeRegistry};

/// Creates a simple benchmark table
pub fn create_benchmark_table(db: &Database, table_name: &str, initial_capacity: Option<usize>) {
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

/// Parse comma-separated string into vector of usize
pub fn parse_comma_separated(input: &str) -> Vec<usize> {
    input
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .expect("Invalid value: must be positive integer")
        })
        .collect()
}

/// Run all performance regression tests
pub fn run_all_benchmarks() {
    use crate::benchmarks;

    println!("Running all performance regression tests...");
    println!("{}", "=".repeat(60));

    // Baseline read test
    println!("\n1. Baseline Read Throughput Test");
    println!("{}", "-".repeat(40));
    benchmarks::baseline_read::run_baseline_read_test(100_000, 1_000_000);

    // Write throughput test
    println!("\n2. Write Throughput Test");
    println!("{}", "-".repeat(40));
    benchmarks::write_throughput::run_write_throughput_test(10_000);

    // Mixed workload test
    println!("\n3. Mixed Workload Test");
    println!("{}", "-".repeat(40));
    benchmarks::mixed_workload::run_mixed_workload_test(100_000, 80);

    // Procedure scaling test
    println!("\n4. Procedure Scaling Test");
    println!("{}", "-".repeat(40));
    benchmarks::procedure_scaling::run_procedure_scaling_test(1_000_000, "1,2,4,8");

    // Memory overhead test
    println!("\n5. Memory Overhead Test");
    println!("{}", "-".repeat(40));
    benchmarks::memory_overhead::run_memory_overhead_test("64,256,1024,4096", "1000,5000,10000");

    // Cache contention test
    println!("\n6. Cache Line Contention Test");
    println!("{}", "-".repeat(40));
    benchmarks::cache_contention::run_cache_contention_test("1,2,4", "63,64,128", 10_000);

    println!("\n{}", "=".repeat(60));
    println!("All tests completed.");
}
