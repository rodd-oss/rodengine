use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ecsdb::Database;

fn bench_single_insert(c: &mut Criterion) {
    c.bench_function("insert_single_record", |b| {
        b.iter(|| {
            // TODO: actual benchmark
            black_box(0);
        });
    });
}

criterion_group!(benches, bench_single_insert);
criterion_main!(benches);
