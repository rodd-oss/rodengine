use std::collections::HashMap;

use crate::error::DbError;
use crate::table::Table;

use super::transaction::Transaction;

/// RAII guard for transaction handling with auto-abort on drop.
///
/// If the transaction is not explicitly committed, it will be
/// automatically aborted when the handle is dropped.
#[derive(Debug)]
pub struct TransactionHandle {
    /// The transaction being managed
    pub(crate) transaction: Transaction,
    /// Whether to auto-abort on drop
    pub(crate) auto_abort: bool,
}

impl TransactionHandle {
    /// Creates a new transaction handle.
    pub fn new() -> Self {
        Self {
            transaction: Transaction::new(),
            auto_abort: true,
        }
    }

    /// Gets a mutable reference to the underlying transaction.
    pub fn transaction_mut(&mut self) -> &mut Transaction {
        &mut self.transaction
    }

    /// Commits the transaction.
    ///
    /// # Arguments
    /// * `tables` - Map of table name to Table reference
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit(mut self, tables: &HashMap<String, &Table>) -> Result<(), DbError> {
        self.auto_abort = false;
        self.transaction.commit(tables)
    }

    /// Commits the transaction without consuming the handle.
    ///
    /// # Arguments
    /// * `tables` - Map of table name to Table reference
    ///
    /// # Returns
    /// `Result<(), DbError>` indicating success or failure.
    pub fn commit_with_tables(&mut self, tables: &HashMap<String, &Table>) -> Result<(), DbError> {
        self.auto_abort = false;
        self.transaction.commit(tables)
    }

    /// Aborts the transaction.
    pub fn abort(mut self) {
        self.auto_abort = false;
        self.transaction.abort();
    }

    /// Returns whether the transaction has been committed.
    pub fn is_committed(&self) -> bool {
        self.transaction.is_committed()
    }

    /// Returns whether the transaction has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.transaction.is_aborted()
    }

    /// Returns whether the transaction is still active.
    pub fn is_active(&self) -> bool {
        self.transaction.is_active()
    }
}

impl Default for TransactionHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TransactionHandle {
    fn drop(&mut self) {
        if self.auto_abort && self.transaction.is_active() {
            self.transaction.abort();
        }
    }
}
