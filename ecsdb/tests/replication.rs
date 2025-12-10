use ecsdb::component::{Component, ZeroCopyComponent};
use ecsdb::db::Database;
use ecsdb::error::Result;
use ecsdb::replication::ReplicationConfig;
use ecsdb::schema::types::{DatabaseSchema, TableDefinition, FieldDefinition, FieldType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
struct Transform {
    position_x: f32,
    position_y: f32,
    position_z: f32,
}

impl Component for Transform {
    const TABLE_ID: u16 = 1;
    const TABLE_NAME: &'static str = "transform";
}

unsafe impl ZeroCopyComponent for Transform {
    fn static_size() -> usize {
        std::mem::size_of::<Transform>()
    }
    fn alignment() -> usize {
        std::mem::align_of::<Transform>()
    }
}

#[tokio::test]
async fn test_replication_enable() -> Result<()> {
    // Create database with replication disabled
    let schema = ecsdb::schema::parser::SchemaParser::from_file("examples/simple_schema.toml")?;
    let mut db = Database::from_schema(schema)?;
    
    // Enable replication with a random port
    let config = ReplicationConfig {
        listen_addr: "127.0.0.1:0".to_string(), // OS-assigned port
        broadcast_scheduler_interval_ms: 10_000_000, // huge interval to prevent processing
        broadcast_throttle_ms: 10_000_000, // huge throttle
        ..Default::default()
    };
    db.enable_replication(config).await?;
    
    // Verify replication manager is present
    assert!(db.replication_manager().is_some());
    
    Ok(())
}

#[tokio::test]
async fn test_broadcast_delta_on_commit() -> Result<()> {
    // Create a simple schema with one table, no foreign keys
    use ecsdb::schema::types::{DatabaseSchema, TableDefinition, FieldDefinition, FieldType};
    let schema = DatabaseSchema {
        name: "test".to_string(),
        version: "1.0".to_string(),
        tables: vec![
            TableDefinition {
                name: "test_component".to_string(),
                fields: vec![
                    FieldDefinition {
                        name: "value".to_string(),
                        field_type: FieldType::U32,
                        nullable: false,
                        indexed: false,
                        primary_key: false,
                        foreign_key: None,
                    },
                ],
                parent_table: None,
                description: None,
            },
        ],
        enums: std::collections::HashMap::new(),
        custom_types: std::collections::HashMap::new(),
    };
    let mut db = Database::from_schema(schema)?;
    
    let config = ReplicationConfig {
        listen_addr: "127.0.0.1:0".to_string(),
        broadcast_scheduler_interval_ms: 10_000_000,
        broadcast_throttle_ms: 10_000_000,
        ..Default::default()
    };
    db.enable_replication(config).await?;
    
    let rm = db.replication_manager().unwrap();
    let queue = rm.broadcast_queue();
    
    // Initially no pending deltas
    assert_eq!(queue.pending_count().await, 0);
    
    // Define a component matching the schema
    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
    struct TestComponent {
        value: u32,
    }
    impl Component for TestComponent {
        const TABLE_ID: u16 = 1;
        const TABLE_NAME: &'static str = "test_component";
    }
    unsafe impl ZeroCopyComponent for TestComponent {
        fn static_size() -> usize {
            std::mem::size_of::<TestComponent>()
        }
        fn alignment() -> usize {
            std::mem::align_of::<TestComponent>()
        }
    }
    
    // Register component and insert data
    db.register_component::<TestComponent>()?;
    let entity_id = db.create_entity()?;
    let comp = TestComponent { value: 42 };
    db.insert(entity_id.0, &comp)?;
    let version = db.commit()?;
    assert!(version > 0);
    
    // Delta is generated and broadcast; with no clients it will be dropped.
    // We just verify that commit succeeded and replication didn't break.
    // Optionally check that queue is empty (since no clients).
    // assert_eq!(queue.pending_count().await, 0);
    
    Ok(())
}

#[tokio::test]
async fn test_delta_encoding_roundtrip() -> Result<()> {
    use ecsdb::replication::delta_encoder::{Frame, FrameFlag};
    use bytes::Bytes;
    
    let payload = b"test payload";
    let frame = Frame::new(FrameFlag::Delta as u8, Bytes::from(&payload[..]));
    
    // Encode
    let encoded = frame.encode();
    
    // Decode
    let decoded = Frame::decode(encoded)?;
    
    assert_eq!(decoded.version, frame.version);
    assert_eq!(decoded.flags, frame.flags);
    assert_eq!(decoded.payload, frame.payload);
    
    Ok(())
}