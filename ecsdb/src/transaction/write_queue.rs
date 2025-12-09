use crate::error::{EcsDbError, Result};
use crate::transaction::wal::{WalLogger, WalOp};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

/// Default timeout for write operations (5 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Operation that can be sent to the write thread.
#[derive(Debug)]
pub enum WriteOp {
    Insert {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
        response: Sender<Result<()>>,
    },
    Update {
        table_id: u16,
        entity_id: u64,
        data: Vec<u8>,
        response: Sender<Result<()>>,
    },
    Delete {
        table_id: u16,
        entity_id: u64,
        response: Sender<Result<()>>,
    },
    CommitBatch {
        transaction_id: u64,
        operations: Vec<WriteOpWithoutResponse>,
        response: Sender<Result<u64>>, // returns new version
    },
    Shutdown,
}

/// Write operation without embedded response channel (used inside CommitBatch).
#[derive(Debug, Clone)]
pub enum WriteOpWithoutResponse {
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
}

/// Write queue handle that can be shared across threads.
pub struct WriteQueue {
    tx: Sender<WriteOp>,
    // We hold the join handle to ensure the thread lives as long as the queue.
    _thread: thread::JoinHandle<()>,
    /// Timeout for waiting for write thread responses
    timeout: Duration,
}

impl WriteQueue {
    /// Spawns a new write thread and returns a handle to send operations.
    /// The caller must provide a `process` closure that will be called by the write thread
    /// to apply operations to the database state.
    pub fn spawn<F>(mut process: F) -> Self
    where
        F: FnMut(&WriteOpWithoutResponse) -> Result<()> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();

        let thread = thread::spawn(move || {
            let rx = rx;
            let mut wal = WalLogger::new();
            while let Ok(op) = rx.recv() {
                match op {
                    WriteOp::Insert {
                        table_id,
                        entity_id,
                        data,
                        response,
                    } => {
                        let txn_id = wal.begin_transaction();
                        if let Err(e) = wal.log_operation(txn_id, 0, WalOp::Insert {
                            table_id,
                            entity_id,
                            data: data.clone(),
                        }) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process(&WriteOpWithoutResponse::Insert {
                            table_id,
                            entity_id,
                            data,
                        });
                        if result.is_ok() {
                            let _ = wal.log_commit(txn_id);
                        } else {
                            let _ = wal.log_rollback(txn_id);
                        }
                        let _ = response.send(result);
                    }
                    WriteOp::Update {
                        table_id,
                        entity_id,
                        data,
                        response,
                    } => {
                        let txn_id = wal.begin_transaction();
                        if let Err(e) = wal.log_operation(txn_id, 0, WalOp::Update {
                            table_id,
                            entity_id,
                            data: data.clone(),
                        }) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process(&WriteOpWithoutResponse::Update {
                            table_id,
                            entity_id,
                            data,
                        });
                        if result.is_ok() {
                            let _ = wal.log_commit(txn_id);
                        } else {
                            let _ = wal.log_rollback(txn_id);
                        }
                        let _ = response.send(result);
                    }
                    WriteOp::Delete {
                        table_id,
                        entity_id,
                        response,
                    } => {
                        let txn_id = wal.begin_transaction();
                        if let Err(e) = wal.log_operation(txn_id, 0, WalOp::Delete {
                            table_id,
                            entity_id,
                        }) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process(&WriteOpWithoutResponse::Delete {
                            table_id,
                            entity_id,
                        });
                        if result.is_ok() {
                            let _ = wal.log_commit(txn_id);
                        } else {
                            let _ = wal.log_rollback(txn_id);
                        }
                        let _ = response.send(result);
                    }
                    WriteOp::CommitBatch {
                        transaction_id,
                        operations,
                        response,
                    } => {
                        let mut all_ok = true;
                        for (seq, op) in operations.into_iter().enumerate() {
                            // Log operation to WAL
                            let wal_op = match &op {
                                WriteOpWithoutResponse::Insert { table_id, entity_id, data } => {
                                    WalOp::Insert {
                                        table_id: *table_id,
                                        entity_id: *entity_id,
                                        data: data.clone(),
                                    }
                                }
                                WriteOpWithoutResponse::Update { table_id, entity_id, data } => {
                                    WalOp::Update {
                                        table_id: *table_id,
                                        entity_id: *entity_id,
                                        data: data.clone(),
                                    }
                                }
                                WriteOpWithoutResponse::Delete { table_id, entity_id } => {
                                    WalOp::Delete {
                                        table_id: *table_id,
                                        entity_id: *entity_id,
                                    }
                                }
                            };
                            if let Err(e) = wal.log_operation(transaction_id, seq as u32, wal_op) {
                                all_ok = false;
                                let _ = response.send(Err(e));
                                break;
                            }
                            if let Err(e) = process(&op) {
                                all_ok = false;
                                // Log rollback for the transaction
                                let _ = wal.log_rollback(transaction_id);
                                let _ = response.send(Err(e));
                                break;
                            }
                        }
                        if all_ok {
                            // Log commit marker
                            if let Err(e) = wal.log_commit(transaction_id) {
                                let _ = response.send(Err(e));
                            } else {
                                let _ = response.send(Ok(transaction_id));
                            }
                        }
                    }
                    WriteOp::Shutdown => break,
                }
            }
        });

        Self {
            tx,
            _thread: thread,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Sets the timeout for waiting for write thread responses.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Sends an insert operation and waits for the response.
    pub fn insert(&self, table_id: u16, entity_id: u64, data: Vec<u8>) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let op = WriteOp::Insert {
            table_id,
            entity_id,
            data,
            response: tx,
        };
        self.tx.send(op).map_err(|_| EcsDbError::ChannelClosed)?;
        rx.recv_timeout(self.timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => EcsDbError::Timeout,
            RecvTimeoutError::Disconnected => EcsDbError::ChannelClosed,
        })?
    }

    /// Sends an update operation and waits for the response.
    pub fn update(&self, table_id: u16, entity_id: u64, data: Vec<u8>) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let op = WriteOp::Update {
            table_id,
            entity_id,
            data,
            response: tx,
        };
        self.tx.send(op).map_err(|_| EcsDbError::ChannelClosed)?;
        rx.recv_timeout(self.timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => EcsDbError::Timeout,
            RecvTimeoutError::Disconnected => EcsDbError::ChannelClosed,
        })?
    }

    /// Sends a delete operation and waits for the response.
    pub fn delete(&self, table_id: u16, entity_id: u64) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let op = WriteOp::Delete {
            table_id,
            entity_id,
            response: tx,
        };
        self.tx.send(op).map_err(|_| EcsDbError::ChannelClosed)?;
        rx.recv_timeout(self.timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => EcsDbError::Timeout,
            RecvTimeoutError::Disconnected => EcsDbError::ChannelClosed,
        })?
    }

    /// Sends a batch of operations to be applied atomically.
    /// Returns the new database version after the commit.
    pub fn commit_batch(&self, transaction_id: u64, operations: Vec<WriteOpWithoutResponse>) -> Result<u64> {
        let (tx, rx) = mpsc::channel();
        let op = WriteOp::CommitBatch {
            transaction_id,
            operations,
            response: tx,
        };
        self.tx.send(op).map_err(|_| EcsDbError::ChannelClosed)?;
        rx.recv_timeout(self.timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => EcsDbError::Timeout,
            RecvTimeoutError::Disconnected => EcsDbError::ChannelClosed,
        })?
    }

    /// Shuts down the write thread (waits for it to finish).
    /// After calling this, no further operations can be sent.
    pub fn shutdown(self) -> thread::Result<()> {
        // Send shutdown signal
        let _ = self.tx.send(WriteOp::Shutdown);
        // Wait for thread to finish
        self._thread.join()
    }
}
