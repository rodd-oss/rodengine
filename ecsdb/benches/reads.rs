use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_single_read(c: &mut Criterion) {
    c.bench_function("read_single_record", |b| {
        b.iter(|| {
            black_box(0);
        });
    });
}

criterion_group!(benches, bench_single_read);
criterion_main!(benches);
