//! Persistence manager for schema and data files.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crc32fast::Hasher;

#[cfg(feature = "persist")]
use memmap2::Mmap;

use crate::config::DbConfig;
use crate::database::Database;
use crate::error::DbError;
use crate::table::{Field, Relation, Table};
use crate::types::TypeRegistry;

use super::io_utils::{classify_io_error, retry_io_operation};
use super::schema::{CustomTypeSchema, FieldSchema, RelationSchema, SchemaFile, TableSchema};
use super::schema_validation::validate_schema;

/// Persistence manager for schema and data files.
#[derive(Debug)]
pub struct PersistenceManager {
    /// Data directory path
    data_dir: PathBuf,
    /// Flush interval in ticks
    flush_interval_ticks: u32,
    /// Current tick count
    tick_count: AtomicU64,
    /// Maximum buffer size per table in bytes
    max_buffer_size: usize,
    /// Maximum retry attempts for transient I/O errors
    max_retries: u32,
    /// Delay between retry attempts in milliseconds
    retry_delay_ms: u64,
}

impl PersistenceManager {
    /// Creates a new persistence manager with the given configuration.
    pub fn new(config: &DbConfig) -> Self {
        Self {
            data_dir: config.data_dir.clone(),
            flush_interval_ticks: config.persistence_interval_ticks,
            tick_count: AtomicU64::new(0),
            max_buffer_size: config.max_buffer_size,
            max_retries: config.persistence_max_retries,
            retry_delay_ms: config.persistence_retry_delay_ms,
        }
    }

    /// Saves the database schema to disk.
    ///
    /// # Arguments
    /// * `db` - Database to save
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn save_schema(&self, db: &Database) -> Result<(), DbError> {
        retry_io_operation(
            || self.save_schema_internal(db),
            self.max_retries,
            self.retry_delay_ms,
            "save_schema",
        )
    }

    /// Internal implementation of save_schema with retry logic.
    fn save_schema_internal(&self, db: &Database) -> Result<(), DbError> {
        let schema = self.build_schema(db)?;
        let schema_json = serde_json::to_string_pretty(&schema)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        // Write to temporary file first
        let temp_path = self.data_dir.join("schema.json.tmp");
        let final_path = self.data_dir.join("schema.json");

        // Ensure data directory exists
        fs::create_dir_all(&self.data_dir)
            .map_err(|e| classify_io_error(e, "Failed to create data directory"))?;

        // Write to temporary file
        let mut file = File::create(&temp_path)
            .map_err(|e| classify_io_error(e, "Failed to create temp file"))?;
        file.write_all(schema_json.as_bytes())
            .map_err(|e| classify_io_error(e, "Failed to write schema"))?;
        file.sync_all()
            .map_err(|e| classify_io_error(e, "Failed to sync schema"))?;

        // Atomic rename
        fs::rename(&temp_path, &final_path)
            .map_err(|e| classify_io_error(e, "Failed to rename schema file"))?;

        Ok(())
    }

    /// Loads the database schema from disk.
    ///
    /// # Arguments
    /// * `type_registry` - Type registry to populate with custom types
    ///
    /// # Returns
    /// `Result<Database, DbError>` containing the loaded database.
    pub fn load_schema(&self, type_registry: Arc<TypeRegistry>) -> Result<Database, DbError> {
        let schema_path = self.data_dir.join("schema.json");

        if !schema_path.exists() {
            // No schema file, return empty database
            return Ok(Database::with_type_registry(type_registry));
        }

        // Read schema file
        let mut file = File::open(&schema_path)
            .map_err(|e| classify_io_error(e, "Failed to open schema file"))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| classify_io_error(e, "Failed to read schema file"))?;

        // Parse schema
        let schema: SchemaFile = serde_json::from_str(&contents)
            .map_err(|e| DbError::SerializationError(format!("Failed to parse schema: {}", e)))?;

        // Validate version
        if schema.version != 1 {
            return Err(DbError::SerializationError(format!(
                "Unsupported schema version: {}",
                schema.version
            )));
        }

        // Ensure custom types are registered
        for (type_id, type_schema) in &schema.custom_types {
            type_registry
                .ensure_type_registered(
                    type_id,
                    type_schema.size,
                    type_schema.align,
                    type_schema.pod,
                )
                .map_err(|e| {
                    DbError::SerializationError(format!(
                        "Failed to ensure type {} is registered: {}",
                        type_id, e
                    ))
                })?;
        }

        // Validate schema integrity
        validate_schema(&schema, &type_registry)?;

        // Create database
        let db = Database::with_type_registry(type_registry);

        // Create tables
        for (table_name, table_schema) in &schema.tables {
            let fields = self.build_fields(&db, table_schema)?;
            db.create_table(table_name.clone(), fields, None, self.max_buffer_size)?;

            // Add relations
            let mut table_ref = db.get_table_mut(table_name)?;
            for relation_schema in &table_schema.relations {
                let relation = Relation {
                    to_table: relation_schema.to_table.clone(),
                    from_field: relation_schema.from_field.clone(),
                    to_field: relation_schema.to_field.clone(),
                };
                table_ref.add_relation(relation);
            }
        }

        Ok(db)
    }

    /// Flushes table data to disk.
    ///
    /// # Arguments
    /// * `table` - Table to flush
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn flush_table_data(&self, table: &Table) -> Result<(), DbError> {
        retry_io_operation(
            || self.flush_table_data_internal(table),
            self.max_retries,
            self.retry_delay_ms,
            "flush_table_data",
        )
    }

    /// Internal implementation of flush_table_data with retry logic.
    fn flush_table_data_internal(&self, table: &Table) -> Result<(), DbError> {
        // Ensure data directory exists
        fs::create_dir_all(&self.data_dir)
            .map_err(|e| classify_io_error(e, "Failed to create data directory"))?;

        let data_dir = self.data_dir.join("data");
        fs::create_dir_all(&data_dir)
            .map_err(|e| classify_io_error(e, "Failed to create data directory"))?;

        let temp_path = data_dir.join(format!("{}.bin.tmp", table.name));
        let final_path = data_dir.join(format!("{}.bin", table.name));

        // Load current buffer
        let buffer = table.buffer.load();

        // Calculate checksum before writing
        let mut hasher = Hasher::new();
        hasher.update(buffer.as_slice());
        let checksum = hasher.finalize();

        // Write to temporary file
        let mut file = File::create(&temp_path)
            .map_err(|e| classify_io_error(e, "Failed to create temp file"))?;
        file.write_all(buffer.as_slice())
            .map_err(|e| classify_io_error(e, "Failed to write data"))?;
        file.sync_all()
            .map_err(|e| classify_io_error(e, "Failed to sync data"))?;

        // Atomic rename
        fs::rename(&temp_path, &final_path)
            .map_err(|e| classify_io_error(e, "Failed to rename data file"))?;

        // Update checksum in schema
        self.update_schema_checksum(&table.name, checksum)?;

        Ok(())
    }

    /// Handles file growth by remapping memory-mapped files.
    ///
    /// This method should be called when a table's buffer grows beyond
    /// the current memory-mapped file size.
    ///
    /// # Arguments
    /// * `table` - Table whose buffer has grown
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    #[cfg(feature = "persist")]
    pub fn handle_file_growth(&self, table: &Table) -> Result<(), DbError> {
        use std::io::Write;

        let data_path = self
            .data_dir
            .join("data")
            .join(format!("{}.bin", table.name));

        if !data_path.exists() {
            // No existing file, just flush normally
            return self.flush_table_data(table);
        }

        // Load current buffer
        let buffer = table.buffer.load();
        let current_size = buffer.len();

        // Check if file needs to be grown
        let metadata = fs::metadata(&data_path)
            .map_err(|e| classify_io_error(e, "Failed to get file metadata"))?;
        let file_size = metadata.len() as usize;

        if current_size <= file_size {
            // Buffer fits within existing file, no need to grow
            return Ok(());
        }

        // File needs to be grown
        // Open file in append mode
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&data_path)
            .map_err(|e| classify_io_error(e, "Failed to open file for appending"))?;

        // Calculate additional bytes needed
        let additional_bytes = current_size - file_size;

        // Write zeros to extend file (this will be overwritten by actual data on next flush)
        let zeros = vec![0u8; additional_bytes];
        file.write_all(&zeros)
            .map_err(|e| classify_io_error(e, "Failed to extend file"))?;
        file.sync_all()
            .map_err(|e| classify_io_error(e, "Failed to sync extended file"))?;

        // Now we need to reload the memory-mapped file
        // For simplicity, we'll just flush the entire buffer
        // In a more optimized implementation, we would remap the file
        self.flush_table_data(table)
    }

    /// Loads table data from disk.
    ///
    /// # Arguments
    /// * `table` - Table to load data into
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn load_table_data(&self, table: &Table) -> Result<(), DbError> {
        let data_path = self
            .data_dir
            .join("data")
            .join(format!("{}.bin", table.name));

        if !data_path.exists() {
            // No data file, table is empty
            return Ok(());
        }

        #[cfg(feature = "persist")]
        {
            self.load_table_data_mmap(table, &data_path)
        }

        #[cfg(not(feature = "persist"))]
        {
            self.load_table_data_read(table, &data_path)
        }
    }

    /// Loads table data using traditional file reading.
    #[cfg(not(feature = "persist"))]
    fn load_table_data_read(
        &self,
        table: &Table,
        data_path: &std::path::Path,
    ) -> Result<(), DbError> {
        // Read data file
        let mut file =
            File::open(data_path).map_err(|e| classify_io_error(e, "Failed to open data file"))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| classify_io_error(e, "Failed to read data file"))?;

        // Verify checksum
        self.verify_checksum(&table.name, &data)?;

        self.validate_and_store_data(table, &data)
    }

    /// Loads table data using memory-mapped files.
    #[cfg(feature = "persist")]
    fn load_table_data_mmap(
        &self,
        table: &Table,
        data_path: &std::path::Path,
    ) -> Result<(), DbError> {
        // Open file for reading
        let file =
            File::open(data_path).map_err(|e| classify_io_error(e, "Failed to open data file"))?;

        // Memory map the file
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| classify_io_error(e, "Failed to memory map file"))?
        };

        // Verify checksum
        self.verify_checksum(&table.name, &mmap)?;

        // Validate data size is multiple of record size
        if !mmap.len().is_multiple_of(table.record_size) {
            return Err(DbError::SerializationError(format!(
                "Data file size {} is not multiple of record size {}",
                mmap.len(),
                table.record_size
            )));
        }

        // Restore next_id from max ID in data
        let record_count = mmap.len() / table.record_size;
        let mut max_id = 0u64;

        // Find id field (assuming first field is always id)
        if let Some(id_field) = table.fields.first() {
            if id_field.type_id == "u64" && id_field.size == 8 {
                for i in 0..record_count {
                    let offset = i * table.record_size + id_field.offset;
                    if offset + 8 <= mmap.len() {
                        let id_bytes = &mmap[offset..offset + 8];
                        let id = u64::from_le_bytes(id_bytes.try_into().unwrap());
                        max_id = max_id.max(id);
                    }
                }
            }
        }

        // Store memory-mapped data and update next_id
        table.buffer.store_mmap(mmap);
        table
            .next_id
            .store(max_id + 1, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }

    /// Validates data and stores it in the table buffer.
    #[cfg(not(feature = "persist"))]
    fn validate_and_store_data(&self, table: &Table, data: &[u8]) -> Result<(), DbError> {
        // Validate data size is multiple of record size
        if !data.len().is_multiple_of(table.record_size) {
            return Err(DbError::SerializationError(format!(
                "Data file size {} is not multiple of record size {}",
                data.len(),
                table.record_size
            )));
        }

        // Restore next_id from max ID in data
        let record_count = data.len() / table.record_size;
        let mut max_id = 0u64;

        // Find id field (assuming first field is always id)
        if let Some(id_field) = table.fields.first() {
            if id_field.type_id == "u64" && id_field.size == 8 {
                for i in 0..record_count {
                    let offset = i * table.record_size + id_field.offset;
                    if offset + 8 <= data.len() {
                        let id_bytes = &data[offset..offset + 8];
                        let id = u64::from_le_bytes(id_bytes.try_into().unwrap());
                        max_id = max_id.max(id);
                    }
                }
            }
        }

        // Store data and update next_id
        table.buffer.store(data.to_vec()).map_err(|e| match e {
            DbError::MemoryLimitExceeded {
                requested, limit, ..
            } => DbError::MemoryLimitExceeded {
                requested,
                limit,
                table: table.name.clone(),
            },
            _ => e,
        })?;
        table
            .next_id
            .store(max_id + 1, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }

    /// Called on each tick to trigger periodic flushes.
    ///
    /// # Arguments
    /// * `db` - Database to flush
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn tick(&self, db: &Database) -> Result<(), DbError> {
        let tick = self
            .tick_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if tick.is_multiple_of(self.flush_interval_ticks as u64) {
            tracing::debug!(
                "Persistence tick {} triggering flush (interval: {} ticks)",
                tick,
                self.flush_interval_ticks
            );
            self.flush_all_tables(db)?;
            tracing::debug!("Persistence flush completed successfully");
        }

        Ok(())
    }

    /// Flushes all tables in the database.
    ///
    /// # Arguments
    /// * `db` - Database to flush
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn flush_all_tables(&self, db: &Database) -> Result<(), DbError> {
        let table_count = db.table_count();
        tracing::debug!("Flushing all {} tables", table_count);

        db.with_tables_map(|tables| {
            let mut flushed_count = 0;
            let mut error_count = 0;

            for table in tables.values() {
                if let Err(e) = self.flush_table_data(table) {
                    // Log error but continue with other tables
                    tracing::error!("Failed to flush table {}: {}", table.name, e);
                    error_count += 1;
                } else {
                    flushed_count += 1;
                }
            }

            tracing::debug!(
                "Flush completed: {}/{} tables flushed successfully, {} errors",
                flushed_count,
                table_count,
                error_count
            );
        })
    }

    /// Builds schema from database.
    fn build_schema(&self, db: &Database) -> Result<SchemaFile, DbError> {
        let mut tables = HashMap::new();
        let mut custom_types = HashMap::new();
        let mut checksums = HashMap::new();

        // Extract custom types from type registry
        let type_registry = db.type_registry();
        let all_type_ids = type_registry.type_ids();

        // Built-in type IDs that should not be included in custom_types
        let builtin_type_ids = [
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "string",
        ];

        for type_id in all_type_ids {
            // Skip built-in types
            if builtin_type_ids.contains(&type_id.as_str()) {
                continue;
            }

            // Get type layout for custom type
            if let Some(layout) = type_registry.get(&type_id) {
                let custom_type_schema = CustomTypeSchema {
                    size: layout.size,
                    align: layout.align,
                    pod: layout.pod,
                };
                custom_types.insert(type_id, custom_type_schema);
            }
        }

        db.with_tables_map(|db_tables| {
            for (table_name, table) in db_tables {
                let fields = table
                    .fields
                    .iter()
                    .map(|f| FieldSchema {
                        name: f.name.clone(),
                        r#type: f.type_id.clone(),
                        offset: f.offset,
                    })
                    .collect();

                let relations = table
                    .relations
                    .iter()
                    .map(|r| RelationSchema {
                        to_table: r.to_table.clone(),
                        from_field: r.from_field.clone(),
                        to_field: r.to_field.clone(),
                    })
                    .collect();

                tables.insert(
                    table_name.clone(),
                    TableSchema {
                        record_size: table.record_size,
                        fields,
                        relations,
                    },
                );

                // Calculate checksum for existing data file if it exists
                let data_path = self
                    .data_dir
                    .join("data")
                    .join(format!("{}.bin", table_name));
                if data_path.exists() {
                    if let Ok(checksum) = self.calculate_file_checksum(&data_path) {
                        checksums.insert(table_name.clone(), checksum);
                    }
                }
            }
        })?;

        Ok(SchemaFile {
            version: 1,
            tables,
            custom_types,
            checksums,
        })
    }

    /// Builds field definitions from schema.
    fn build_fields(
        &self,
        db: &Database,
        table_schema: &TableSchema,
    ) -> Result<Vec<Field>, DbError> {
        let type_registry = db.type_registry();
        let mut fields = Vec::with_capacity(table_schema.fields.len());

        for field_schema in &table_schema.fields {
            let layout =
                type_registry
                    .get(&field_schema.r#type)
                    .ok_or_else(|| DbError::TypeMismatch {
                        expected: field_schema.r#type.clone(),
                        got: "unknown type".to_string(),
                    })?;

            let field = Field::new(
                field_schema.name.clone(),
                field_schema.r#type.clone(),
                layout.clone(),
                field_schema.offset,
            );
            fields.push(field);
        }

        Ok(fields)
    }

    /// Calculates CRC32 checksum for a file.
    fn calculate_file_checksum(&self, path: &std::path::Path) -> Result<u32, DbError> {
        let mut file = File::open(path).map_err(|e| classify_io_error(e, "Failed to open file"))?;
        let mut hasher = Hasher::new();

        // Use a larger buffer for better performance (64KB)
        let mut buffer = vec![0u8; 65536];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| classify_io_error(e, "Failed to read file"))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hasher.finalize())
    }

    /// Updates checksum in schema file for a table.
    fn update_schema_checksum(&self, table_name: &str, checksum: u32) -> Result<(), DbError> {
        let schema_path = self.data_dir.join("schema.json");
        if !schema_path.exists() {
            // No schema file yet, nothing to update
            return Ok(());
        }

        // Read existing schema
        let mut file = File::open(&schema_path)
            .map_err(|e| classify_io_error(e, "Failed to open schema file"))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| classify_io_error(e, "Failed to read schema file"))?;

        // Parse schema
        let mut schema: SchemaFile = serde_json::from_str(&contents)
            .map_err(|e| DbError::SerializationError(format!("Failed to parse schema: {}", e)))?;

        // Update checksum
        schema.checksums.insert(table_name.to_string(), checksum);

        // Write updated schema
        let schema_json = serde_json::to_string_pretty(&schema)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        let temp_path = self.data_dir.join("schema.json.tmp");
        let mut temp_file = File::create(&temp_path)
            .map_err(|e| classify_io_error(e, "Failed to create temp schema file"))?;
        temp_file
            .write_all(schema_json.as_bytes())
            .map_err(|e| classify_io_error(e, "Failed to write schema"))?;
        temp_file
            .sync_all()
            .map_err(|e| classify_io_error(e, "Failed to sync schema"))?;

        // Atomic rename
        fs::rename(&temp_path, &schema_path)
            .map_err(|e| classify_io_error(e, "Failed to rename schema file"))?;

        Ok(())
    }

    /// Verifies checksum for a data file.
    fn verify_checksum(&self, table_name: &str, data: &[u8]) -> Result<(), DbError> {
        let schema_path = self.data_dir.join("schema.json");
        if !schema_path.exists() {
            // No schema file, skip checksum verification
            return Ok(());
        }

        // Read schema to get expected checksum
        let mut file = File::open(&schema_path)
            .map_err(|e| classify_io_error(e, "Failed to open schema file"))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| classify_io_error(e, "Failed to read schema file"))?;

        let schema: SchemaFile = serde_json::from_str(&contents)
            .map_err(|e| DbError::SerializationError(format!("Failed to parse schema: {}", e)))?;

        if let Some(expected_checksum) = schema.checksums.get(table_name) {
            // Calculate actual checksum
            let mut hasher = Hasher::new();
            hasher.update(data);
            let actual_checksum = hasher.finalize();

            if actual_checksum != *expected_checksum {
                return Err(DbError::DataCorruption(format!(
                    "Checksum mismatch for table '{}': expected {:08x}, got {:08x}",
                    table_name, expected_checksum, actual_checksum
                )));
            }
        }

        Ok(())
    }
}
