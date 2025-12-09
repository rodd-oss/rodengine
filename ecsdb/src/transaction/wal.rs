use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Operation type for write-ahead log entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalOp {
    Insert {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
    },
    Update {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
    },
    Delete {
        table_id: u16,
        entity_id: u64,
    },
    Commit {
        transaction_id: u64,
    },
    Rollback {
        transaction_id: u64,
    },
}

/// A single entry in the write-ahead log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Monotonic timestamp (microseconds since epoch)
    pub timestamp: u64,
    /// Unique transaction identifier
    pub transaction_id: u64,
    /// Sequence number within transaction
    pub sequence: u32,
    /// The operation to log
    pub operation: WalOp,
    /// Checksum of the entry (CRC32)
    pub checksum: u32,
}

impl WalEntry {
    /// Creates a new WAL entry with a computed checksum.
    pub fn new(transaction_id: u64, sequence: u32, operation: WalOp) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // Create entry without checksum first
        let mut entry = Self {
            timestamp,
            transaction_id,
            sequence,
            operation,
            checksum: 0,
        };

        // Compute and set checksum
        entry.checksum = entry.compute_checksum();
        entry
    }

    /// Computes a CRC32 checksum of the entry (excluding the checksum field).
    fn compute_checksum(&self) -> u32 {
        // For simplicity, we'll use a simple hash; in production use CRC32.
        // Using crc32fast crate would be better, but we avoid extra dependencies.
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.transaction_id.to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        // Serialize operation? For now, we'll hash the debug representation.
        // Instead, we'll serialize with bincode and hash bytes.
        let op_bytes = bincode::serialize(&self.operation).unwrap();
        hasher.update(&op_bytes);
        hasher.finalize()
    }

    /// Validates the entry's checksum.
    pub fn validate_checksum(&self) -> bool {
        let mut clone = self.clone();
        clone.checksum = 0;
        clone.compute_checksum() == self.checksum
    }
}

/// In-memory write-ahead log (for Phase 1).
/// In later phases, this will be backed by disk storage.
pub struct WalLogger {
    entries: Vec<WalEntry>,
    next_transaction_id: u64,
}

impl WalLogger {
    /// Creates a new empty WAL.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_transaction_id: 1,
        }
    }

    /// Starts a new transaction and returns its ID.
    pub fn begin_transaction(&mut self) -> u64 {
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        id
    }

    /// Logs an operation as part of a transaction.
    pub fn log_operation(&mut self, transaction_id: u64, sequence: u32, operation: WalOp) -> Result<()> {
        let entry = WalEntry::new(transaction_id, sequence, operation);
        self.entries.push(entry);
        Ok(())
    }

    /// Logs a commit marker for a transaction.
    pub fn log_commit(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self.next_sequence_for_transaction(transaction_id);
        let entry = WalEntry::new(transaction_id, seq, WalOp::Commit { transaction_id });
        self.entries.push(entry);
        Ok(())
    }

    /// Logs a rollback marker for a transaction.
    pub fn log_rollback(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self.next_sequence_for_transaction(transaction_id);
        let entry = WalEntry::new(transaction_id, seq, WalOp::Rollback { transaction_id });
        self.entries.push(entry);
        Ok(())
    }

    /// Returns all entries for a given transaction.
    pub fn entries_for_transaction(&self, transaction_id: u64) -> Vec<&WalEntry> {
        self.entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .collect()
    }

    /// Returns the next sequence number for a transaction.
    fn next_sequence_for_transaction(&self, transaction_id: u64) -> u32 {
        self.entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .count() as u32
    }

    /// Returns the number of entries in the WAL.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the WAL is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears the WAL (only safe after a checkpoint).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for WalLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_entry_checksum() {
        let entry = WalEntry::new(1, 0, WalOp::Insert {
            table_id: 1,
            entity_id: 100,
            data: vec![1, 2, 3],
        });
        assert!(entry.validate_checksum());
        // Tamper with data
        let mut tampered = entry.clone();
        tampered.checksum = 0;
        assert!(!tampered.validate_checksum());
    }

    #[test]
    fn test_wal_logger() {
        let mut wal = WalLogger::new();
        let txn_id = wal.begin_transaction();
        wal.log_operation(txn_id, 0, WalOp::Insert {
            table_id: 1,
            entity_id: 100,
            data: vec![],
        }).unwrap();
        wal.log_commit(txn_id).unwrap();
        assert_eq!(wal.len(), 2);
        let entries = wal.entries_for_transaction(txn_id);
        assert_eq!(entries.len(), 2);
        match &entries[0].operation {
            WalOp::Insert { table_id, entity_id, .. } => {
                assert_eq!(*table_id, 1);
                assert_eq!(*entity_id, 100);
            }
            _ => panic!("expected Insert"),
        }
        match &entries[1].operation {
            WalOp::Commit { transaction_id } => {
                assert_eq!(*transaction_id, txn_id);
            }
            _ => panic!("expected Commit"),
        }
    }
}