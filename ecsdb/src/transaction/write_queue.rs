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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Insert {
                                table_id,
                                entity_id,
                                data: data.clone(),
                            },
                        ) {
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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Update {
                                table_id,
                                entity_id,
                                data: data.clone(),
                            },
                        ) {
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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Delete {
                                table_id,
                                entity_id,
                            },
                        ) {
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
                                WriteOpWithoutResponse::Insert {
                                    table_id,
                                    entity_id,
                                    data,
                                } => WalOp::Insert {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                    data: data.clone(),
                                },
                                WriteOpWithoutResponse::Update {
                                    table_id,
                                    entity_id,
                                    data,
                                } => WalOp::Update {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                    data: data.clone(),
                                },
                                WriteOpWithoutResponse::Delete {
                                    table_id,
                                    entity_id,
                                } => WalOp::Delete {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                },
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

    /// Spawns a new write thread with separate batch processor for atomic batch operations.
    /// The `process_single` closure is called for individual insert/update/delete operations.
    /// The `process_batch` closure is called for CommitBatch with all operations; it must
    /// apply them atomically (all-or-nothing). If `process_batch` returns an error,
    /// the entire batch is rolled back (via WAL rollback log).
    pub fn spawn_with_batch<F, G>(mut process_single: F, mut process_batch: G) -> Self
    where
        F: FnMut(&WriteOpWithoutResponse) -> Result<()> + Send + 'static,
        G: FnMut(&[WriteOpWithoutResponse]) -> Result<()> + Send + 'static,
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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Insert {
                                table_id,
                                entity_id,
                                data: data.clone(),
                            },
                        ) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process_single(&WriteOpWithoutResponse::Insert {
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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Update {
                                table_id,
                                entity_id,
                                data: data.clone(),
                            },
                        ) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process_single(&WriteOpWithoutResponse::Update {
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
                        if let Err(e) = wal.log_operation(
                            txn_id,
                            0,
                            WalOp::Delete {
                                table_id,
                                entity_id,
                            },
                        ) {
                            let _ = response.send(Err(e));
                            continue;
                        }
                        let result = process_single(&WriteOpWithoutResponse::Delete {
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
                        // Log all operations first
                        for (seq, op) in operations.iter().enumerate() {
                            let wal_op = match op {
                                WriteOpWithoutResponse::Insert {
                                    table_id,
                                    entity_id,
                                    data,
                                } => WalOp::Insert {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                    data: data.clone(),
                                },
                                WriteOpWithoutResponse::Update {
                                    table_id,
                                    entity_id,
                                    data,
                                } => WalOp::Update {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                    data: data.clone(),
                                },
                                WriteOpWithoutResponse::Delete {
                                    table_id,
                                    entity_id,
                                } => WalOp::Delete {
                                    table_id: *table_id,
                                    entity_id: *entity_id,
                                },
                            };
                            if let Err(e) = wal.log_operation(transaction_id, seq as u32, wal_op) {
                                let _ = response.send(Err(e));
                                // Already logged previous operations; need to rollback transaction
                                let _ = wal.log_rollback(transaction_id);
                                continue;
                            }
                        }
                        // Now process batch atomically
                        match process_batch(&operations) {
                            Ok(()) => {
                                if let Err(e) = wal.log_commit(transaction_id) {
                                    let _ = response.send(Err(e));
                                } else {
                                    let _ = response.send(Ok(transaction_id));
                                }
                            }
                            Err(e) => {
                                let _ = wal.log_rollback(transaction_id);
                                let _ = response.send(Err(e));
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
    pub fn commit_batch(
        &self,
        transaction_id: u64,
        operations: Vec<WriteOpWithoutResponse>,
    ) -> Result<u64> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct MockOp {
        op: WriteOpWithoutResponse,
    }

    #[derive(Default, Clone)]
    struct MockProcessor {
        recorded: Arc<Mutex<Vec<MockOp>>>,
        // If true, all operations return ChannelClosed error
        should_error: Arc<Mutex<bool>>,
    }

    impl MockProcessor {
        fn new() -> Self {
            Self {
                recorded: Arc::new(Mutex::new(Vec::new())),
                should_error: Arc::new(Mutex::new(false)),
            }
        }

        fn process(&self, op: &WriteOpWithoutResponse) -> Result<()> {
            let mut recorded = self.recorded.lock().unwrap();
            let result = if *self.should_error.lock().unwrap() {
                Err(EcsDbError::ChannelClosed)
            } else {
                Ok(())
            };
            recorded.push(MockOp { op: op.clone() });
            result
        }

        fn process_batch(&self, ops: &[WriteOpWithoutResponse]) -> Result<()> {
            let mut recorded = self.recorded.lock().unwrap();
            let result = if *self.should_error.lock().unwrap() {
                Err(EcsDbError::ChannelClosed)
            } else {
                Ok(())
            };
            for op in ops {
                recorded.push(MockOp { op: op.clone() });
            }
            result
        }

        fn recorded_ops(&self) -> Vec<WriteOpWithoutResponse> {
            self.recorded
                .lock()
                .unwrap()
                .iter()
                .map(|m| m.op.clone())
                .collect()
        }

        fn set_force_error(&self, _err: EcsDbError) {
            *self.should_error.lock().unwrap() = true;
        }
    }

    #[test]
    fn test_write_queue_spawn_and_shutdown() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| p.process(op)
        });
        // Shutdown should succeed
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_insert_operation() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| p.process(op)
        });
        let result = queue.insert(1, 100, vec![1, 2, 3]);
        assert!(result.is_ok());
        let ops = processor.recorded_ops();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WriteOpWithoutResponse::Insert {
                table_id,
                entity_id,
                data,
            } => {
                assert_eq!(*table_id, 1);
                assert_eq!(*entity_id, 100);
                assert_eq!(data, &vec![1, 2, 3]);
            }
            _ => panic!("Unexpected op"),
        }
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_update_operation() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| p.process(op)
        });
        let result = queue.update(2, 200, vec![4, 5, 6]);
        assert!(result.is_ok());
        let ops = processor.recorded_ops();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WriteOpWithoutResponse::Update {
                table_id,
                entity_id,
                data,
            } => {
                assert_eq!(*table_id, 2);
                assert_eq!(*entity_id, 200);
                assert_eq!(data, &vec![4, 5, 6]);
            }
            _ => panic!("Unexpected op"),
        }
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_delete_operation() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| p.process(op)
        });
        let result = queue.delete(3, 300);
        assert!(result.is_ok());
        let ops = processor.recorded_ops();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            WriteOpWithoutResponse::Delete {
                table_id,
                entity_id,
            } => {
                assert_eq!(*table_id, 3);
                assert_eq!(*entity_id, 300);
            }
            _ => panic!("Unexpected op"),
        }
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_commit_batch_atomic() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn_with_batch(
            {
                let p = processor.clone();
                move |op| p.process(op)
            },
            {
                let p = processor.clone();
                move |ops| p.process_batch(ops)
            },
        );
        let ops = vec![
            WriteOpWithoutResponse::Insert {
                table_id: 1,
                entity_id: 100,
                data: vec![1],
            },
            WriteOpWithoutResponse::Update {
                table_id: 2,
                entity_id: 200,
                data: vec![2],
            },
        ];
        let result = queue.commit_batch(123, ops);
        assert!(result.is_ok());
        let recorded = processor.recorded_ops();
        // The batch processor records each operation individually
        assert_eq!(recorded.len(), 2);
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_batch_rollback_on_error() {
        let processor = MockProcessor::new();
        processor.set_force_error(EcsDbError::ChannelClosed);
        let queue = WriteQueue::spawn_with_batch(
            {
                let p = processor.clone();
                move |op| p.process(op)
            },
            {
                let p = processor.clone();
                move |ops| p.process_batch(ops)
            },
        );
        let ops = vec![WriteOpWithoutResponse::Insert {
            table_id: 1,
            entity_id: 100,
            data: vec![1],
        }];
        let result = queue.commit_batch(124, ops);
        assert!(result.is_err());
        // Even though error, the operations were recorded (since we record before processing)
        let recorded = processor.recorded_ops();
        assert_eq!(recorded.len(), 1);
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_timeout() {
        let processor = MockProcessor::new();
        let mut queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| {
                // Simulate long processing
                std::thread::sleep(Duration::from_millis(100));
                p.process(op)
            }
        });
        // Set timeout shorter than processing time
        queue.set_timeout(Duration::from_millis(10));
        let result = queue.insert(1, 100, vec![]);
        assert!(result.is_err());
        match result.unwrap_err() {
            EcsDbError::Timeout => (),
            _ => panic!("Expected timeout error"),
        }
        queue.shutdown().unwrap();
    }

    #[test]
    fn test_channel_closed_after_shutdown() {
        let processor = MockProcessor::new();
        let queue = WriteQueue::spawn({
            let p = processor.clone();
            move |op| p.process(op)
        });
        queue.shutdown().unwrap();
        // Sending after shutdown should fail
        // But we cannot call queue.insert because queue is moved. So we can't test.
        // Instead we can test that shutdown consumes queue.
    }
}
