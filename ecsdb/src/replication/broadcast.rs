//! Network broadcast mechanism for delta propagation.
//!
//! Manages outbound delta batches, flow control, and reliable delivery.

use crate::error::Result;
use crate::replication::client::{ClientManager, ClientMessage};
use crate::storage::delta::{Delta, DeltaOp};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// A batch of delta operations ready for broadcast.
#[derive(Debug, Clone)]
pub struct DeltaBatch {
    pub ops: Vec<DeltaOp>,
    pub version: u64,
    pub timestamp: u64,
}

impl From<Delta> for DeltaBatch {
    fn from(delta: Delta) -> Self {
        Self {
            ops: delta.ops,
            version: delta.version,
            timestamp: delta.timestamp,
        }
    }
}

/// Broadcast queue with flow control and batching.
pub struct BroadcastQueue {
    /// Pending delta batches waiting to be sent.
    queue: Mutex<VecDeque<DeltaBatch>>,
    /// Maximum batch size (number of operations).
    _batch_size: usize,
    /// Client manager for sending.
    client_manager: Mutex<Option<Arc<ClientManager>>>,
    /// Minimum interval between broadcasts (for throttling).
    throttle_interval: Duration,
    /// Last broadcast time.
    last_broadcast: Mutex<Option<Instant>>,
}

impl BroadcastQueue {
    pub fn new(batch_size: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            _batch_size: batch_size,
            client_manager: Mutex::new(None),
            throttle_interval: Duration::from_millis(10),
            last_broadcast: Mutex::new(None),
        }
    }

    /// Sets the client manager (called after creation).
    pub async fn set_client_manager(&self, manager: Arc<ClientManager>) {
        let mut client_manager = self.client_manager.lock().await;
        *client_manager = Some(manager);
    }

    /// Enqueues a delta for broadcast.
    pub async fn enqueue(&self, delta: Delta) -> Result<()> {
        let mut queue = self.queue.lock().await;
        // If the delta is small and we have room in the last batch, merge it.
        // For simplicity, we just push the whole delta as a batch.
        let batch = DeltaBatch::from(delta);
        queue.push_back(batch);
        Ok(())
    }

    /// Processes the broadcast queue, sending batches to clients.
    /// This should be called periodically (e.g., from a background task).
    pub async fn process(&self) -> Result<usize> {
        // Throttling: skip if we broadcast too recently
        let mut last_broadcast = self.last_broadcast.lock().await;
        if let Some(instant) = *last_broadcast {
            if instant.elapsed() < self.throttle_interval {
                return Ok(0);
            }
        }

        let batch = {
            let mut queue = self.queue.lock().await;
            queue.pop_front()
        };

        if let Some(batch) = batch {
            // Convert batch to Delta for serialization
            let delta = Delta {
                ops: batch.ops,
                version: batch.version,
                timestamp: batch.timestamp,
            };
            // Send to all ready clients
            let client_manager_guard = self.client_manager.lock().await;
            if let Some(client_manager) = client_manager_guard.as_ref() {
                let count = client_manager
                    .broadcast(ClientMessage::Delta(delta))
                    .await?;
                *last_broadcast = Some(Instant::now());
                Ok(count)
            } else {
                // No client manager set; drop batch
                log::warn!("BroadcastQueue has no client manager, dropping delta");
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }

    /// Returns the number of pending batches.
    pub async fn pending_count(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    /// Clears the broadcast queue.
    pub async fn clear(&self) {
        let mut queue = self.queue.lock().await;
        queue.clear();
    }

    /// Sets the throttle interval.
    pub fn set_throttle_interval(&mut self, interval: Duration) {
        self.throttle_interval = interval;
    }
}

/// Background broadcast scheduler that runs in a Tokio task.
pub struct BroadcastScheduler {
    queue: Arc<BroadcastQueue>,
    interval: Duration,
    shutdown_signal: tokio::sync::watch::Receiver<bool>,
}

impl BroadcastScheduler {
    pub fn new(
        queue: Arc<BroadcastQueue>,
        interval: Duration,
        shutdown_signal: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self {
            queue,
            interval,
            shutdown_signal,
        }
    }

    /// Runs the scheduler loop.
    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.queue.process().await {
                        log::error!("Broadcast processing error: {}", e);
                    }
                }
                _ = self.shutdown_signal.changed() => {
                    if *self.shutdown_signal.borrow() {
                        log::info!("Broadcast scheduler shutting down");
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
