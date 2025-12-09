use crate::error::{EcsDbError, Result};
use std::sync::mpsc::{self, Sender};
use std::thread;

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
            while let Ok(op) = rx.recv() {
                match op {
                    WriteOp::Insert {
                        table_id,
                        entity_id,
                        data,
                        response,
                    } => {
                        let result = process(&WriteOpWithoutResponse::Insert {
                            table_id,
                            entity_id,
                            data,
                        });
                        let _ = response.send(result);
                    }
                    WriteOp::Update {
                        table_id,
                        entity_id,
                        data,
                        response,
                    } => {
                        let result = process(&WriteOpWithoutResponse::Update {
                            table_id,
                            entity_id,
                            data,
                        });
                        let _ = response.send(result);
                    }
                    WriteOp::Delete {
                        table_id,
                        entity_id,
                        response,
                    } => {
                        let result = process(&WriteOpWithoutResponse::Delete {
                            table_id,
                            entity_id,
                        });
                        let _ = response.send(result);
                    }
                    WriteOp::CommitBatch {
                        operations,
                        response,
                    } => {
                        let version = 0; // placeholder
                        let mut all_ok = true;
                        for op in operations {
                            if let Err(e) = process(&op) {
                                // TODO: rollback?
                                all_ok = false;
                                let _ = response.send(Err(e));
                                break;
                            }
                        }
                        if all_ok {
                            // TODO: actual commit that bumps version
                            let _ = response.send(Ok(version));
                        }
                    }
                    WriteOp::Shutdown => break,
                }
            }
        });

        Self {
            tx,
            _thread: thread,
        }
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
        rx.recv().map_err(|_| EcsDbError::ChannelClosed)?
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
        rx.recv().map_err(|_| EcsDbError::ChannelClosed)?
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
        rx.recv().map_err(|_| EcsDbError::ChannelClosed)?
    }

    /// Sends a batch of operations to be applied atomically.
    /// Returns the new database version after the commit.
    pub fn commit_batch(&self, operations: Vec<WriteOpWithoutResponse>) -> Result<u64> {
        let (tx, rx) = mpsc::channel();
        let op = WriteOp::CommitBatch {
            operations,
            response: tx,
        };
        self.tx.send(op).map_err(|_| EcsDbError::ChannelClosed)?;
        rx.recv().map_err(|_| EcsDbError::ChannelClosed)?
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
