//! Database container managing tables, types, and procedures.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use crate::error::DbError;
use crate::table::Table;
use crate::transaction::TransactionHandle;
use crate::types::TypeRegistry;

/// Database container holding all tables, types, and runtime state.
#[derive(Debug)]
pub struct Database {
    /// Map of table name to table instance
    tables: RwLock<HashMap<String, Table>>,
    /// Type registry for custom types
    type_registry: Arc<TypeRegistry>,
}

impl Database {
    /// Creates a new empty database.
    pub fn new() -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
            type_registry: Arc::new(TypeRegistry::new()),
        }
    }

    /// Creates a new database with existing type registry.
    pub fn with_type_registry(type_registry: Arc<TypeRegistry>) -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
            type_registry,
        }
    }

    /// Returns a reference to the type registry.
    pub fn type_registry(&self) -> &Arc<TypeRegistry> {
        &self.type_registry
    }

    /// Creates a new table with the given name and field definitions.
    ///
    /// # Arguments
    /// * `name` - Table name
    /// * `fields` - Field definitions
    /// * `initial_capacity` - Initial buffer capacity in records (default: 1024)
    /// * `max_buffer_size` - Maximum buffer size in bytes (default: unlimited)
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn create_table(
        &self,
        name: String,
        fields: Vec<crate::table::Field>,
        initial_capacity: Option<usize>,
        max_buffer_size: usize,
    ) -> Result<(), DbError> {
        let mut tables = self.tables.write().map_err(|_| DbError::LockPoisoned)?;
        if tables.contains_key(&name) {
            return Err(DbError::TableAlreadyExists(name));
        }
        let table = Table::create(name.clone(), fields, initial_capacity, max_buffer_size)?;
        tables.insert(name, table);
        Ok(())
    }

    /// Deletes a table by name.
    ///
    /// # Arguments
    /// * `name` - Table name
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn delete_table(&self, name: &str) -> Result<(), DbError> {
        let mut tables = self.tables.write().map_err(|_| DbError::LockPoisoned)?;
        tables.remove(name).ok_or_else(|| DbError::TableNotFound {
            table: name.to_string(),
        })?;
        Ok(())
    }

    /// Gets a reference to a table by name.
    ///
    /// # Arguments
    /// * `name` - Table name
    ///
    /// # Returns
    /// `Result<TableRef, DbError>` containing a read-only table reference.
    pub fn get_table(&self, name: &str) -> Result<TableRef<'_>, DbError> {
        let tables = self.tables.read().map_err(|_| DbError::LockPoisoned)?;
        // Verify table exists
        if !tables.contains_key(name) {
            return Err(DbError::TableNotFound {
                table: name.to_string(),
            });
        }
        Ok(TableRef {
            inner: tables,
            key: name.to_string(),
        })
    }

    /// Gets a mutable reference to a table by name.
    ///
    /// # Arguments
    /// * `name` - Table name
    ///
    /// # Returns
    /// `Result<TableRefMut, DbError>` containing a mutable table reference.
    pub fn get_table_mut(&self, name: &str) -> Result<TableRefMut<'_>, DbError> {
        let tables = self.tables.write().map_err(|_| DbError::LockPoisoned)?;
        // Verify table exists (need to check before we can store key)
        if !tables.contains_key(name) {
            return Err(DbError::TableNotFound {
                table: name.to_string(),
            });
        }
        Ok(TableRefMut {
            inner: tables,
            key: name.to_string(),
        })
    }

    /// Returns an iterator over all table names.
    pub fn table_names(&self) -> Vec<String> {
        let tables = self.tables.read().unwrap();
        tables.keys().cloned().collect()
    }

    /// Returns the number of tables in the database.
    pub fn table_count(&self) -> usize {
        let tables = self.tables.read().unwrap();
        tables.len()
    }

    /// Executes a closure with a map of table references.
    ///
    /// This method locks the tables for reading and provides a HashMap
    /// of table references to the closure. The lock is held for the
    /// duration of the closure execution.
    ///
    /// # Arguments
    /// * `f` - Closure to execute with the table map
    ///
    /// # Returns
    /// `Result<R, DbError>` containing the result of the closure.
    pub fn with_tables_map<F, R>(&self, f: F) -> Result<R, DbError>
    where
        F: FnOnce(&HashMap<String, &Table>) -> R,
    {
        let tables = self.tables.read().map_err(|_| DbError::LockPoisoned)?;
        let mut map = HashMap::with_capacity(tables.len());
        for (name, table) in tables.iter() {
            map.insert(name.clone(), table as &Table);
        }
        Ok(f(&map))
    }

    /// Commits a transaction using this database's tables.
    ///
    /// # Arguments
    /// * `tx` - Transaction handle to commit
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit_transaction(&self, tx: &mut TransactionHandle) -> Result<(), DbError> {
        self.with_tables_map(|tables| tx.commit_with_tables(tables))?
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only reference to a table within the database.
pub struct TableRef<'a> {
    inner: std::sync::RwLockReadGuard<'a, HashMap<String, Table>>,
    key: String,
}

impl<'a> std::ops::Deref for TableRef<'a> {
    type Target = Table;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The table exists because we just looked it up and the lock guard ensures
        // the HashMap is not mutated. The key is the same as used in lookup.
        self.inner
            .get(&self.key)
            .expect("Table should exist in HashMap")
    }
}

/// Mutable reference to a table within the database.
pub struct TableRefMut<'a> {
    inner: std::sync::RwLockWriteGuard<'a, HashMap<String, Table>>,
    key: String,
}

impl<'a> std::ops::Deref for TableRefMut<'a> {
    type Target = Table;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The table exists because we just looked it up and the lock guard ensures
        // the HashMap is not mutated. The key is the same as used in lookup.
        self.inner
            .get(&self.key)
            .expect("Table should exist in HashMap")
    }
}

impl<'a> std::ops::DerefMut for TableRefMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The table exists because we just looked it up and the lock guard ensures
        // the HashMap is not mutated. The key is the same as used in lookup.
        self.inner
            .get_mut(&self.key)
            .expect("Table should exist in HashMap")
    }
}

/// Procedure function signature.
///
/// Procedures are Rust functions that can be registered and invoked via RPC.
/// They receive:
/// - A reference to the database
/// - A mutable transaction handle for isolated changes
/// - JSON parameters for configuration
/// - Return JSON result or error
pub type ProcedureFn = fn(
    db: &Database,
    tx: &mut TransactionHandle,
    params: serde_json::Value,
) -> Result<serde_json::Value, DbError>;
