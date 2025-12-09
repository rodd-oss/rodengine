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

fn bench_batch_insert(c: &mut Criterion) {
    let schema = ecsdb::schema::DatabaseSchema {
        name: "bench".to_string(),
        version: "1.0".to_string(),
        tables: Vec::new(),
        enums: std::collections::HashMap::new(),
        custom_types: std::collections::HashMap::new(),
    };
    let db = Database::from_schema(schema).unwrap();
    db.register_component::<BenchComponent>().unwrap();

    c.bench_function("batch_insert_10", |b| {
        b.iter(|| {
            // Create 10 entities and insert components
            for i in 0..10 {
                let entity_id = db.create_entity().unwrap();
                let comp = BenchComponent {
                    x: i as f32,
                    y: i as f32 * 2.0,
                    id: i,
                };
                db.insert(entity_id.0, &comp).unwrap();
            }
            db.commit().unwrap();
        });
    });
}

criterion_group!(benches, bench_batch_insert);
criterion_main!(benches);
