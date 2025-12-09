//! Basic usage example for ECSDb.
//! Demonstrates loading a schema, parsing it, and printing table info.

use ecsdb::schema::parser::SchemaParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load schema from file
    let schema = SchemaParser::from_file("examples/simple_schema.toml")?;

    println!("Database: {} v{}", schema.name, schema.version);
    println!("Tables:");

    for table in &schema.tables {
        println!("  - {}", table.name);
        if let Some(parent) = &table.parent_table {
            println!("    parent: {}", parent);
        }
        for field in &table.fields {
            println!("    {}: {:?}", field.name, field.field_type);
        }
    }

    Ok(())
}
