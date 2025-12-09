use ecsdb::entity::EntityRegistry;
use ecsdb::error::Result;
use ecsdb::schema::parser::SchemaParser;

#[test]
fn test_schema_loading() -> Result<()> {
    let schema = SchemaParser::from_file("examples/simple_schema.toml")?;
    assert_eq!(schema.name, "game_db");
    assert_eq!(schema.tables.len(), 3);
    Ok(())
}

#[test]
fn test_entity_registry() -> Result<()> {
    let mut registry = EntityRegistry::new();
    let entity_id = registry.create_entity(0)?;
    assert_eq!(entity_id.0, 1);

    let entity = registry.get_entity(entity_id)?;
    assert_eq!(entity.id.0, 1);
    assert_eq!(entity.version.0, 0);

    registry.delete_entity(entity_id)?;
    // Should be able to reuse ID later
    let entity_id2 = registry.create_entity(0)?;
    assert_eq!(entity_id2.0, 1); // Reused ID
    assert_eq!(registry.entity_count(), 2); // original deleted, new created

    Ok(())
}
