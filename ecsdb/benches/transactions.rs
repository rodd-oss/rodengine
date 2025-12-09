use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_transaction(c: &mut Criterion) {
    c.bench_function("transaction_commit", |b| {
        b.iter(|| {
            black_box(0);
        });
    });
}

criterion_group!(benches, bench_transaction);
criterion_main!(benches);
