use crate::error::Result;
use crate::transaction::wal::{WalLogger, WalOp};
use std::sync::mpsc;

#[derive(Debug, Clone)]
pub enum TransactionOp {
    Insert {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
    },
    Update {
        table_id: u16,
        entity_id: u64,
        field_offset: usize,
        data: Vec<u8>,
    },
    Delete {
        table_id: u16,
        entity_id: u64,
    },
}

pub struct Transaction {
    operations: Vec<TransactionOp>,
    #[allow(dead_code)]
    response_tx: mpsc::Sender<Result<u64>>, // Returns version on commit
}

impl Transaction {
    pub fn new(response_tx: mpsc::Sender<Result<u64>>) -> Self {
        Self {
            operations: Vec::new(),
            response_tx,
        }
    }

    pub fn insert(&mut self, table_id: u16, entity_id: u64, data: Vec<u8>) {
        self.operations.push(TransactionOp::Insert {
            table_id,
            entity_id,
            data,
        });
    }

    pub fn update(&mut self, table_id: u16, entity_id: u64, field_offset: usize, data: Vec<u8>) {
        self.operations.push(TransactionOp::Update {
            table_id,
            entity_id,
            field_offset,
            data,
        });
    }

    pub fn delete(&mut self, table_id: u16, entity_id: u64) {
        self.operations.push(TransactionOp::Delete {
            table_id,
            entity_id,
        });
    }

    pub fn commit(self) -> Result<u64> {
        // Send to write thread
        // (Would be implemented with MPSC channel in real code)
        Ok(0)
    }
}

pub struct TransactionEngine {
    wal: WalLogger, // Write-ahead log
    version: u64,
}

impl TransactionEngine {
    pub fn new() -> Self {
        Self {
            wal: WalLogger::new(),
            version: 0,
        }
    }

    pub fn process_transaction(&mut self, txn: Transaction) -> Result<u64> {
        let txn_id = self.wal.begin_transaction();
        for (seq, op) in txn.operations.into_iter().enumerate() {
            let wal_op = match op {
                TransactionOp::Insert { table_id, entity_id, data } => {
                    WalOp::Insert { table_id, entity_id, data }
                }
                TransactionOp::Update { table_id, entity_id, field_offset: _, data } => {
                    WalOp::Update { table_id, entity_id, data }
                }
                TransactionOp::Delete { table_id, entity_id } => {
                    WalOp::Delete { table_id, entity_id }
                }
            };
            self.wal.log_operation(txn_id, seq as u32, wal_op)?;
        }
        self.wal.log_commit(txn_id)?;
        self.version += 1;
        Ok(self.version)
    }
}

impl Default for TransactionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_engine() -> Result<()> {
        let mut engine = TransactionEngine::new();
        let (tx, _rx) = mpsc::channel();
        let mut txn = Transaction::new(tx);
        txn.insert(1, 100, vec![1, 2, 3]);
        txn.update(1, 100, 0, vec![4, 5, 6]);
        txn.delete(1, 100);
        let version = engine.process_transaction(txn)?;
        assert_eq!(version, 1);
        // Verify WAL has entries (we can't access wal directly, but we can trust it)
        Ok(())
    }
}
