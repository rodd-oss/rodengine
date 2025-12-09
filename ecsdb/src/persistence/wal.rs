//! Write-ahead logging for durability.

use crate::error::Result;
use crate::transaction::wal::{WalEntry, WalOp, WalLogger};
use async_trait::async_trait;

/// Trait for write-ahead log implementations.
#[async_trait]
pub trait Wal: Send + Sync {
    /// Starts a new transaction and returns its ID.
    fn begin_transaction(&mut self) -> u64;

    /// Logs an operation as part of a transaction.
    async fn log_operation(&mut self, transaction_id: u64, sequence: u32, operation: WalOp) -> Result<()>;

    /// Logs a commit marker for a transaction.
    async fn log_commit(&mut self, transaction_id: u64) -> Result<()>;

    /// Logs a rollback marker for a transaction.
    async fn log_rollback(&mut self, transaction_id: u64) -> Result<()>;

    /// Returns all entries for a given transaction (for replay/testing).
    fn entries_for_transaction(&self, transaction_id: u64) -> Vec<&WalEntry>;

    /// Returns the number of entries in the WAL.
    fn len(&self) -> usize;

    /// Returns true if the WAL is empty.
    fn is_empty(&self) -> bool;

    /// Clears the WAL (only safe after a checkpoint).
    fn clear(&mut self);

    /// Forces any buffered writes to disk (if applicable).
    async fn sync(&self) -> Result<()>;
}

/// In-memory WAL implementation (for testing).
pub struct InMemoryWal {
    entries: Vec<WalEntry>,
    next_transaction_id: u64,
}

impl InMemoryWal {
    /// Creates a new empty in-memory WAL.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_transaction_id: 1,
        }
    }
}

#[async_trait]
impl Wal for InMemoryWal {
    fn begin_transaction(&mut self) -> u64 {
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        id
    }

    async fn log_operation(&mut self, transaction_id: u64, sequence: u32, operation: WalOp) -> Result<()> {
        let entry = WalEntry::new(transaction_id, sequence, operation);
        self.entries.push(entry);
        Ok(())
    }

    async fn log_commit(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self.next_sequence_for_transaction(transaction_id);
        let entry = WalEntry::new(transaction_id, seq, WalOp::Commit { transaction_id });
        self.entries.push(entry);
        Ok(())
    }

    async fn log_rollback(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self.next_sequence_for_transaction(transaction_id);
        let entry = WalEntry::new(transaction_id, seq, WalOp::Rollback { transaction_id });
        self.entries.push(entry);
        Ok(())
    }

    fn entries_for_transaction(&self, transaction_id: u64) -> Vec<&WalEntry> {
        self.entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .collect()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    async fn sync(&self) -> Result<()> {
        Ok(())
    }
}

impl InMemoryWal {
    fn next_sequence_for_transaction(&self, transaction_id: u64) -> u32 {
        self.entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .count() as u32
    }
}

impl Default for InMemoryWal {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Wal for WalLogger {
    fn begin_transaction(&mut self) -> u64 {
        WalLogger::begin_transaction(self)
    }

    async fn log_operation(&mut self, transaction_id: u64, sequence: u32, operation: WalOp) -> Result<()> {
        WalLogger::log_operation(self, transaction_id, sequence, operation)
    }

    async fn log_commit(&mut self, transaction_id: u64) -> Result<()> {
        WalLogger::log_commit(self, transaction_id)
    }

    async fn log_rollback(&mut self, transaction_id: u64) -> Result<()> {
        WalLogger::log_rollback(self, transaction_id)
    }

    fn entries_for_transaction(&self, transaction_id: u64) -> Vec<&WalEntry> {
        WalLogger::entries_for_transaction(self, transaction_id)
    }

    fn len(&self) -> usize {
        WalLogger::len(self)
    }

    fn is_empty(&self) -> bool {
        WalLogger::is_empty(self)
    }

    fn clear(&mut self) {
        WalLogger::clear(self)
    }

    async fn sync(&self) -> Result<()> {
        Ok(())
    }
}