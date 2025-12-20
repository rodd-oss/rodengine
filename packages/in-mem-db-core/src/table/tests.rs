use super::*;
use crate::atomic_buffer::AtomicBuffer;
use crate::error::DbError;
use crate::types::{TypeLayout, TypeRegistry};
use ntest::timeout;

fn create_test_fields() -> Vec<Field> {
    // Create a mock type registry with test layouts
    let _registry = TypeRegistry::new();

    // Create mock layouts for testing
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

    let string_layout = unsafe {
        TypeLayout::new(
            "string".to_string(),
            260, // Fixed size: 4 bytes length + 256 bytes data
            1,
            false,
            |src, dst| {
                let string_ptr = src as *const String;
                let string = &*string_ptr;
                let bytes = string.as_bytes();
                let len = bytes.len().min(256);

                // Write length as u32 (4 bytes)
                dst.extend_from_slice(&(len as u32).to_ne_bytes());

                // Write string bytes
                dst.extend_from_slice(&bytes[..len]);

                // Pad with zeros
                let padding = 256 - len;
                if padding > 0 {
                    dst.extend(std::iter::repeat_n(0u8, padding));
                }

                260
            },
            |src, dst| {
                if src.len() < 260 {
                    return 0;
                }
                // Read length (first 4 bytes)
                let mut len_bytes = [0u8; 4];
                len_bytes.copy_from_slice(&src[..4]);
                let len = u32::from_ne_bytes(len_bytes) as usize;

                let actual_len = len.min(256);
                let dst_ptr = dst as *mut String;
                let bytes = &src[4..4 + actual_len];
                *dst_ptr = String::from_utf8_lossy(bytes).to_string();

                260
            },
            None,
        )
    };

    let bool_layout = unsafe {
        TypeLayout::new(
            "bool".to_string(),
            1,
            1,
            true,
            |src, dst| {
                let bool_ptr = src as *const bool;
                dst.push(if *bool_ptr { 1 } else { 0 });
                1
            },
            |src, dst| {
                if src.is_empty() {
                    return 0;
                }
                let dst_ptr = dst as *mut bool;
                *dst_ptr = src[0] != 0;
                1
            },
            Some(std::any::TypeId::of::<bool>()),
        )
    };

    vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 0),
        Field::new("name".to_string(), "string".to_string(), string_layout, 8),
        Field::new("active".to_string(), "bool".to_string(), bool_layout, 268), // After string field (8 + 260)
    ]
}


#[timeout(1000)]
#[test]
fn test_table_create() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    assert_eq!(table.name, "test_table");
    assert_eq!(table.record_size, 269); // 0-7: id, 8-267: name (260 bytes), 268: active
    assert_eq!(table.fields.len(), 3);
    assert_eq!(table.relations.len(), 0);
    assert_eq!(table.current_next_id(), 1);
    assert_eq!(table.record_count(), 0);
    // Buffer capacity should be 100 records * 269 bytes = 26900 bytes
    assert_eq!(table.buffer.capacity(), 26900);
}

#[timeout(1000)]
#[test]
fn test_field_offset() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    assert_eq!(table.field_offset("id").unwrap(), 0);
    assert_eq!(table.field_offset("name").unwrap(), 8);
    assert_eq!(table.field_offset("active").unwrap(), 268);

    assert!(table.field_offset("nonexistent").is_err());
}

#[timeout(1000)]
#[test]
fn test_next_id() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    assert_eq!(table.next_id(), 1);
    assert_eq!(table.next_id(), 2);
    assert_eq!(table.next_id(), 3);
    assert_eq!(table.current_next_id(), 4);
}

#[timeout(1000)]
#[test]
fn test_duplicate_field_names() {
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

    let fields = vec![
        Field::new("id".to_string(), "u64".to_string(), u64_layout.clone(), 0),
        Field::new("id".to_string(), "u64".to_string(), u64_layout, 8), // Duplicate!
    ];

    let result = Table::create("test_table".to_string(), fields, Some(100), usize::MAX);
    assert!(result.is_err());
    match result {
        Err(DbError::FieldAlreadyExists { table, field }) => {
            assert_eq!(table, "test_table");
            assert_eq!(field, "id");
        }
        _ => panic!("Expected FieldAlreadyExists error"),
    }
}

#[timeout(1000)]
#[test]
fn test_relations() {
    let fields = create_test_fields();
    let mut table =
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    let relation = Relation {
        to_table: "other_table".to_string(),
        from_field: "id".to_string(),
        to_field: "foreign_id".to_string(),
    };

    table.add_relation(relation);
    assert_eq!(table.relations.len(), 1);
    assert_eq!(table.relations[0].to_table, "other_table");

    assert!(table.remove_relation("other_table"));
    assert_eq!(table.relations.len(), 0);
    assert!(!table.remove_relation("nonexistent"));
}

#[timeout(1000)]
#[test]
fn test_get_field() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    assert!(table.get_field("id").is_some());
    assert!(table.get_field("name").is_some());
    assert!(table.get_field("active").is_some());
    assert!(table.get_field("nonexistent").is_none());
}

#[timeout(1000)]
#[test]
fn test_create_record() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create test record data
    let mut data = vec![0u8; table.record_size];
    // Set id field (u64 at offset 0)
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    // Set name field (string at offset 8) - 260 bytes total
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix (4 bytes)
    data[12..17].copy_from_slice(b"hello");
    // Set active field (bool at offset 268)
    data[268] = 1; // true

    // Create record
    let id = table.create_record(&data).unwrap();
    assert_eq!(id, 1);

    // Verify record was added
    assert_eq!(table.record_count(), 1);
    assert_eq!(table.current_next_id(), 2);

    // Test invalid data size
    let result = table.create_record(&data[..10]);
    assert!(result.is_err());
}

#[timeout(1000)]
#[test]
fn test_query_records() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create 5 test records
    for i in 0..5 {
        let mut data = vec![0u8; table.record_size];
        // Set id field (u64 at offset 0)
        data[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
        // Set name field (string at offset 8) - 260 bytes total
        let name = if i % 2 == 0 { "even" } else { "odd" };
        let name_bytes = name.as_bytes();
        data[8..12].copy_from_slice(&(name_bytes.len() as u32).to_ne_bytes()); // Length prefix
        data[12..12 + name_bytes.len()].copy_from_slice(name_bytes);
        // Set active field (bool at offset 268)
        data[268] = if i < 3 { 1 } else { 0 }; // First 3 active, last 2 inactive
        table.create_record(&data).unwrap();
    }

    assert_eq!(table.record_count(), 5);

    // Test query with no filters (should return all records)
    let filters = std::collections::HashMap::new();
    let result = table.query_records(&filters, None, None).unwrap();
    assert_eq!(result.len(), 5);
    assert_eq!(result, vec![0, 1, 2, 3, 4]);

    // Test query with field filter (active = true)
    let mut filters = std::collections::HashMap::new();
    filters.insert("active".to_string(), vec![1]); // true
    let result = table.query_records(&filters, None, None).unwrap();
    assert_eq!(result.len(), 3); // First 3 records are active
    assert_eq!(result, vec![0, 1, 2]);
}

#[timeout(1000)]
#[test]
fn test_record_offset() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    assert_eq!(table.record_offset(0), 0);
    assert_eq!(table.record_offset(1), 269);
    assert_eq!(table.record_offset(10), 2690);
}

#[timeout(1000)]
#[test]
fn test_field_exceeds_record_size() {
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

    // Create a field with offset 96 (aligned to 8), which would require record size 104
    // This should work since record size is calculated from max(field.offset + field.size)
    let fields = vec![Field::new(
        "data".to_string(),
        "u64".to_string(),
        u64_layout,
        96,
    )];

    let result = Table::create("test_table".to_string(), fields, Some(100), usize::MAX);
    assert!(result.is_ok());
    let table = result.unwrap();
    assert_eq!(table.record_size, 104); // 96 + 8
}

#[timeout(1000)]
#[test]
fn test_add_field() {
    let fields = create_test_fields();
    let mut table =
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create a new field layout
    let i32_layout = unsafe {
        TypeLayout::new(
            "i32".to_string(),
            4,
            4,
            true,
            |src, dst| {
                dst.extend_from_slice(std::slice::from_raw_parts(src, 4));
                4
            },
            |src, dst| {
                if src.len() >= 4 {
                    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 4);
                    4
                } else {
                    0
                }
            },
            Some(std::any::TypeId::of::<i32>()),
        )
    };

    // Add a new field
    let offset = table
        .add_field("score".to_string(), "i32".to_string(), i32_layout.clone())
        .unwrap();

    // Check that field was added with correct offset
    // Existing fields: id(0-7), name(8-267), active(268)
    // New field should be at aligned offset after active (268 + 1 = 269, aligned to 4 = 272)
    assert_eq!(offset, 272);
    assert_eq!(table.record_size, 276); // 272 + 4 = 276

    // Check field exists
    assert!(table.get_field("score").is_some());
    let field = table.get_field("score").unwrap();
    assert_eq!(field.name, "score");
    assert_eq!(field.type_id, "i32");
    assert_eq!(field.offset, 272);
    assert_eq!(field.size, 4);
    assert_eq!(field.align, 4);

    // Try to add duplicate field - need to clone the layout since add_field takes ownership
    let i32_layout_clone = i32_layout.clone();
    let result = table.add_field("score".to_string(), "i32".to_string(), i32_layout_clone);
    assert!(result.is_err());
    match result {
        Err(DbError::FieldAlreadyExists { table: t, field }) => {
            assert_eq!(t, "test_table");
            assert_eq!(field, "score");
        }
        _ => panic!("Expected FieldAlreadyExists error"),
    }
}

#[timeout(1000)]
#[test]
fn test_remove_field() {
    let fields = create_test_fields();
    let mut table =
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Remove the "name" field
    assert!(table.remove_field("name").is_ok());

    // Check field was removed
    assert!(table.get_field("name").is_none());
    assert!(table.get_field("id").is_some());
    assert!(table.get_field("active").is_some());

    // Check record size was recalculated
    // After removing name (32 bytes), we have id(8) + active(1) = 9 bytes
    // But active needs to be aligned after id: id ends at 8, active align=1 so offset=8
    // So record size = 8 + 1 = 9
    assert_eq!(table.record_size, 9);

    // Check field offsets were rebuilt
    let id_field = table.get_field("id").unwrap();
    assert_eq!(id_field.offset, 0);

    let active_field = table.get_field("active").unwrap();
    assert_eq!(active_field.offset, 8); // After id (8 bytes)

    // Try to remove non-existent field
    let result = table.remove_field("nonexistent");
    assert!(result.is_err());
    match result {
        Err(DbError::FieldNotFound { table: t, field }) => {
            assert_eq!(t, "test_table");
            assert_eq!(field, "nonexistent");
        }
        _ => panic!("Expected FieldNotFound error"),
    }
}

#[timeout(1000)]
#[test]
fn test_add_field_with_alignment() {
    let _registry = TypeRegistry::new();

    // Create a table with a u8 field (align=1) and a u64 field (align=8)
    let u8_layout = unsafe {
        TypeLayout::new(
            "u8".to_string(),
            1,
            1,
            true,
            |src, dst| {
                dst.push(*src);
                1
            },
            |src, dst| {
                if src.is_empty() {
                    return 0;
                }
                *dst = src[0];
                1
            },
            Some(std::any::TypeId::of::<u8>()),
        )
    };

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
        "flag".to_string(),
        "u8".to_string(),
        u8_layout,
        0,
    )];

    let mut table =
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Add u64 field - should be aligned to 8 bytes
    let offset = table
        .add_field("id".to_string(), "u64".to_string(), u64_layout)
        .unwrap();

    // u8 ends at offset 1, next aligned offset for align=8 is 8
    assert_eq!(offset, 8);
    assert_eq!(table.record_size, 16); // 8 + 8 = 16
}

#[timeout(1000)]
#[test]
fn test_field_type_validation() {
    let fields = create_test_fields();
    let mut table =
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create layout with type_id "i32"
    let i32_layout = unsafe {
        TypeLayout::new(
            "i32".to_string(),
            4,
            4,
            true,
            |src, dst| {
                dst.extend_from_slice(std::slice::from_raw_parts(src, 4));
                4
            },
            |src, dst| {
                if src.len() >= 4 {
                    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, 4);
                    4
                } else {
                    0
                }
            },
            Some(std::any::TypeId::of::<i32>()),
        )
    };

    // Try to add field with mismatched type_id
    let result = table.add_field("score".to_string(), "u32".to_string(), i32_layout);
    assert!(result.is_err());
    match result {
        Err(DbError::TypeMismatch { expected, got }) => {
            assert_eq!(expected, "i32");
            assert_eq!(got, "u32");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[timeout(1000)]
#[test]
fn test_read_record_raw() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Buffer is empty initially
    assert!(table.read_record_raw(0).is_err());

    // We would need to add data to test this properly
    // This test verifies the method exists and returns error for empty buffer
}

#[timeout(1000)]
#[test]
fn test_read_field_raw() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Buffer is empty, but we can still test field lookup
    assert!(table.read_field_raw(0, "id").is_err());
    assert!(table.read_field_raw(0, "nonexistent").is_err());
}

#[timeout(1000)]
#[test]
fn test_concurrent_read_access() {
    use std::sync::Arc;
    use std::thread;

    let fields = create_test_fields();
    let table = Arc::new(
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap(),
    );

    // Spawn multiple threads to test concurrent access
    let mut handles = vec![];
    for _ in 0..10 {
        let table_clone = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            // Each thread loads the buffer
            let buffer = table_clone.buffer.load();
            // Verify we can access it
            assert_eq!(buffer.len(), 0);
            // Thread holds Arc, preventing buffer deallocation
            std::thread::sleep(std::time::Duration::from_micros(100));
            // Arc drops here, allowing buffer deallocation if no other references
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // All threads completed without panics
    // ArcSwap epoch tracking ensures old buffers dropped after last reader
}

#[timeout(1000)]
#[test]
fn test_multiple_readers_same_buffer() {
    use std::sync::Arc;
    use std::thread;

    let fields = create_test_fields();
    let table = Arc::new(
        Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap(),
    );

    // Store some data
    {
        let mut data = vec![0u8; table.record_size * 3];
        // Fill with test data
        for (i, item) in data.iter_mut().enumerate() {
            *item = (i % 256) as u8;
        }
        table.buffer.store(data).unwrap();
    }

    let mut handles = vec![];
    for _ in 0..5 {
        let table_clone = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            // Each thread reads the same buffer concurrently
            let buffer = table_clone.buffer.load();

            // Verify all threads see the same data
            assert_eq!(buffer.len(), table_clone.record_size * 3);

            // Read some data from the buffer
            for record_idx in 0..3 {
                let offset = record_idx * table_clone.record_size;
                if offset < buffer.len() {
                    let slice = buffer.as_slice();
                    let _ = &slice
                        [offset..offset + table_clone.record_size.min(slice.len() - offset)];
                }
            }

            // Simulate some work
            for _ in 0..1000 {
                let slice = buffer.as_slice();
                let _sum: usize = slice.iter().map(|&b| b as usize).sum();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All threads successfully accessed the same buffer concurrently
}

#[timeout(1000)]
#[test]
fn test_update_record() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create initial record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
    data[12..17].copy_from_slice(b"hello");
    data[268] = 1;

    let id = table.create_record(&data).unwrap();
    assert_eq!(id, 1);

    // Update record
    let mut updated_data = vec![0u8; table.record_size];
    updated_data[0..8].copy_from_slice(&2u64.to_le_bytes());
    updated_data[8..12].copy_from_slice(&6u32.to_ne_bytes()); // Length prefix
    updated_data[12..18].copy_from_slice(b"world!");
    updated_data[268] = 0;

    table.update_record(0, &updated_data).unwrap();

    // Verify update
    let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    assert_eq!(slice, &updated_data[..]);

    // Test invalid record index
    let result = table.update_record(1, &updated_data);
    assert!(result.is_err());

    // Test invalid data size
    let result = table.update_record(0, &updated_data[..10]);
    assert!(result.is_err());
}

#[timeout(1000)]
#[test]
fn test_partial_update() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create initial record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
    data[12..17].copy_from_slice(b"hello");
    data[268] = 1;

    table.create_record(&data).unwrap();

    // Partially update only the name field
    let mut name_data = vec![0u8; 260]; // string field size (260 bytes)
    name_data[0..4].copy_from_slice(&6u32.to_ne_bytes()); // Length prefix (4 bytes)
    name_data[4..10].copy_from_slice(b"world!");

    table.partial_update(0, &[("name", &name_data)]).unwrap();

    // Verify only name was updated
    let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

    // id should still be 1
    assert_eq!(&slice[0..8], &1u64.to_le_bytes());
    // name should be updated
    assert_eq!(&slice[8..12], &6u32.to_ne_bytes()); // Length prefix (4 bytes)
    assert_eq!(&slice[12..18], b"world!");
    // active should still be 1
    assert_eq!(slice[268], 1);

    // Test invalid field name
    let result = table.partial_update(0, &[("nonexistent", &name_data)]);
    assert!(result.is_err());

    // Test invalid field data size
    let result = table.partial_update(0, &[("name", &name_data[..10])]);
    assert!(result.is_err());
}

#[timeout(1000)]
#[test]
fn test_delete_and_compact_records() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create 3 records
    for i in 0..3 {
        let mut data = vec![0u8; table.record_size];
        data[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
        data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
        data[12..17].copy_from_slice(b"hello");
        data[268] = 0; // active = false initially
        table.create_record(&data).unwrap();
    }

    assert_eq!(table.record_count(), 3);

    // Delete record 1 (index 1)
    table.delete_record(1, "active").unwrap();

    // Verify record 1 is marked as deleted
    let (ptr, len, _arc) = table.read_record_raw(1).unwrap();
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    assert_eq!(slice[268], 1); // active = true (deleted)

    // Compact table
    let removed = table.compact_table("active").unwrap();
    assert_eq!(removed, 1);
    assert_eq!(table.record_count(), 2);

    // Verify remaining records are 0 and 2 (original indices)
    let (ptr0, _, _) = table.read_record_raw(0).unwrap();
    let slice0 = unsafe { std::slice::from_raw_parts(ptr0, table.record_size) };
    assert_eq!(&slice0[0..8], &1u64.to_le_bytes()); // id = 1

    let (ptr1, _, _) = table.read_record_raw(1).unwrap();
    let slice1 = unsafe { std::slice::from_raw_parts(ptr1, table.record_size) };
    assert_eq!(&slice1[0..8], &3u64.to_le_bytes()); // id = 3

    // Test invalid deletion field
    let result = table.delete_record(0, "nonexistent");
    assert!(result.is_err());

    // Test compact with non-boolean field
    let result = table.compact_table("id");
    assert!(result.is_err());
}

#[timeout(1000)]
#[test]
fn test_concurrent_writers_last_writer_wins() {
    use std::sync::Arc;
    use std::thread;

    let fields = create_test_fields();
    let table = Arc::new(
        Table::create("test_table".to_string(), fields, Some(1000), usize::MAX).unwrap(),
    );

    // Create initial record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
    data[12..17].copy_from_slice(b"hello");
    data[268] = 0;

    table.create_record(&data).unwrap();

    // Spawn multiple writers that update the same record
    let mut handles = vec![];
    for i in 0..10 {
        let table_clone = Arc::clone(&table);
        let mut update_data = vec![0u8; table.record_size];
        update_data[0..8].copy_from_slice(&(i as u64 + 100).to_le_bytes());
        update_data[8..12].copy_from_slice(&1u32.to_ne_bytes()); // Length prefix
        update_data[12] = b'A' + i as u8;
        update_data[268] = (i % 2) as u8;

        handles.push(thread::spawn(move || {
            // Each writer clones buffer and updates
            table_clone.update_record(0, &update_data).unwrap();
        }));
    }

    // Wait for all writers
    for handle in handles {
        handle.join().unwrap();
    }

    // Only the last writer's changes should be visible
    // (last-writer-wins semantics)
    let (ptr, len, _arc) = table.read_record_raw(0).unwrap();
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

    // The final state should be from one of the writers
    // We can't predict which one due to race conditions,
    // but the record should be valid
    let id = u64::from_le_bytes(slice[0..8].try_into().unwrap());
    assert!((100..=109).contains(&id));
    assert_eq!(&slice[8..12], &1u32.to_ne_bytes()); // Length prefix (4 bytes)
    assert!((b'A'..=b'J').contains(&slice[12]));
    assert!(slice[268] == 0 || slice[268] == 1);
}

#[timeout(1000)]
#[test]
fn test_failed_write_discards_buffer() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create initial record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
    data[12..17].copy_from_slice(b"hello");
    data[268] = 0;

    table.create_record(&data).unwrap();

    // Save initial state
    let initial_len = table.buffer.len();

    // Attempt invalid update (out of bounds)
    let result = table.update_record(5, &data);
    assert!(result.is_err());

    // Buffer should be unchanged
    assert_eq!(table.buffer.len(), initial_len);

    // Attempt another invalid operation
    let result = table.partial_update(0, &[("nonexistent", &data[..10])]);
    assert!(result.is_err());

    // Buffer should still be unchanged
    assert_eq!(table.buffer.len(), initial_len);

    // Valid operation should work
    data[268] = 1;
    table.update_record(0, &data).unwrap();
    assert_eq!(table.buffer.len(), initial_len); // Same length, different content
}

#[timeout(1000)]
#[test]
fn test_load_full_clones_buffer() {
    let buffer = AtomicBuffer::new(1024, 64, usize::MAX);

    // Store initial data
    let initial_data = vec![1u8, 2, 3, 4, 5];
    buffer.store(initial_data.clone()).unwrap();

    // Load for reading
    let read_arc = buffer.load();
    assert_eq!(read_arc.as_slice(), &initial_data);

    // Load for modification (clones)
    let mut cloned = buffer.load_full();
    assert_eq!(&cloned, &initial_data);

    // Modify clone
    cloned.push(6);
    cloned.push(7);

    // Original buffer unchanged
    let read_arc2 = buffer.load();
    assert_eq!(read_arc2.as_slice(), &initial_data);

    // Store modified clone
    buffer.store(cloned).unwrap();

    // Now buffer is updated
    let read_arc3 = buffer.load();
    assert_eq!(read_arc3.as_slice(), &[1u8, 2, 3, 4, 5, 6, 7]);
}

#[timeout(1000)]
#[test]
fn test_create_record_from_values() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create field values
    let id_bytes = 1u64.to_le_bytes();
    let mut name_bytes = vec![0u8; 260]; // string field size (260 bytes)
    name_bytes[0..4].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix (4 bytes)
    name_bytes[4..9].copy_from_slice(b"hello");
    let active_bytes = [1u8]; // bool = true

    let field_values = vec![&id_bytes[..], &name_bytes[..], &active_bytes[..]];

    // Create record from values
    let id = table.create_record_from_values(&field_values).unwrap();
    assert_eq!(id, 1);
    assert_eq!(table.record_count(), 1);

    // Read back and verify
    let (slice, _arc) = table.read_record(0).unwrap();
    assert_eq!(slice.len(), table.record_size);

    // Verify id field
    assert_eq!(&slice[0..8], &id_bytes);

    // Verify name field
    assert_eq!(&slice[8..12], &5u32.to_ne_bytes()); // Length prefix (4 bytes)
    assert_eq!(&slice[12..17], b"hello");

    // Verify active field
    assert_eq!(slice[268], 1);

    // Test field count mismatch
    let result = table.create_record_from_values(&[&id_bytes[..]]);
    assert!(result.is_err());

    // Test field size mismatch
    let wrong_size_bytes = [0u8; 4]; // Should be 8 bytes for u64
    let result = table.create_record_from_values(&[
        &wrong_size_bytes[..],
        &name_bytes[..],
        &active_bytes[..],
    ]);
    assert!(result.is_err());
}

#[timeout(1000)]
#[test]
fn test_read_record() {
    let fields = create_test_fields();
    let table = Table::create("test_table".to_string(), fields, Some(100), usize::MAX).unwrap();

    // Create a record
    let mut data = vec![0u8; table.record_size];
    data[0..8].copy_from_slice(&1u64.to_le_bytes());
    data[8..12].copy_from_slice(&5u32.to_ne_bytes()); // Length prefix
    data[12..17].copy_from_slice(b"hello");
    data[268] = 1;

    table.create_record(&data).unwrap();

    // Read record as slice
    let (slice, arc) = table.read_record(0).unwrap();
    assert_eq!(slice.len(), table.record_size);
    assert_eq!(slice, &data[..]);

    // Arc should hold the buffer
    assert_eq!(arc.len(), table.record_size);

    // Test out of bounds
    assert!(table.read_record(1).is_err());
}

