use crate::error::Result;
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
    wal: Vec<TransactionOp>, // Write-ahead log
    version: u64,
}

impl TransactionEngine {
    pub fn new() -> Self {
        Self {
            wal: Vec::new(),
            version: 0,
        }
    }

    pub fn process_transaction(&mut self, txn: Transaction) -> Result<u64> {
        // Log operations
        for op in txn.operations {
            self.wal.push(op);
        }

        // Bump version
        self.version += 1;

        Ok(self.version)
    }
}

impl Default for TransactionEngine {
    fn default() -> Self {
        Self::new()
    }
}
