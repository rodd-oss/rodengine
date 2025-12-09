use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ecsdb::component::{Component, ZeroCopyComponent};
use ecsdb::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
struct BenchComponent {
    x: f32,
    y: f32,
    id: u32,
}

impl Component for BenchComponent {
    const TABLE_ID: u16 = 999;
    const TABLE_NAME: &'static str = "bench_component";
}

unsafe impl ZeroCopyComponent for BenchComponent {
    fn static_size() -> usize {
        std::mem::size_of::<BenchComponent>()
    }
    fn alignment() -> usize {
        std::mem::align_of::<BenchComponent>()
    }
}

fn bench_single_read(c: &mut Criterion) {
    // Setup database with one entity
    let schema = ecsdb::schema::DatabaseSchema {
        name: "bench".to_string(),
        version: "1.0".to_string(),
        tables: Vec::new(),
        enums: std::collections::HashMap::new(),
        custom_types: std::collections::HashMap::new(),
    };
    let db = Database::from_schema(schema).unwrap();
    db.register_component::<BenchComponent>().unwrap();
    let entity_id = db.create_entity().unwrap();
    let comp = BenchComponent {
        x: 1.0,
        y: 2.0,
        id: 42,
    };
    db.insert(entity_id.0, &comp).unwrap();
    db.commit().unwrap();

    c.bench_function("read_single_record", |b| {
        b.iter(|| {
            let retrieved = db.get::<BenchComponent>(entity_id.0).unwrap();
            black_box(retrieved);
        });
    });
}

criterion_group!(benches, bench_single_read);
criterion_main!(benches);
