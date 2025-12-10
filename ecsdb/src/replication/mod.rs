//! Replication module for multi‑client synchronization.
//!
//! Provides delta‑based replication with client connection management,
//! delta serialization, network broadcast, conflict resolution, and
//! full/incremental sync protocols.

pub mod broadcast;
pub mod client;
pub mod conflict;
pub mod delta_encoder;
pub mod delta_log;
pub mod sync;

pub use broadcast::{BroadcastQueue, BroadcastScheduler};
pub use client::{ClientManager, ClientSession};
pub use conflict::{ConflictLog, ConflictResolver, ConflictStrategy};
pub use delta_encoder::{DeltaDecoder, DeltaEncoder, Frame, FrameFlag};
pub use delta_log::{DeltaLog, DeltaLogEntry};
pub use sync::{
    FullSyncMessage, FullSyncProtocol, IncrementalSyncMessage, IncrementalSyncProtocol,
};

use crate::error::{EcsDbError, Result};
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Replication configuration.
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// TCP listen address (e.g., "127.0.0.1:9000")
    pub listen_addr: String,
    /// WebSocket listen address (optional, e.g., "127.0.0.1:9001")
    pub websocket_addr: Option<String>,
    /// Maximum number of connected clients.
    pub max_clients: usize,
    /// Authentication token (optional).
    pub auth_token: Option<String>,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_secs: u64,
    /// Delta batch size (number of operations per network packet).
    pub delta_batch_size: usize,
    /// Enable compression (zstd).
    pub enable_compression: bool,
    /// Conflict resolution strategy.
    pub conflict_strategy: ConflictStrategy,
    /// Broadcast throttle interval in milliseconds.
    pub broadcast_throttle_ms: u64,
    /// Broadcast scheduler interval in milliseconds.
    pub broadcast_scheduler_interval_ms: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:9000".to_string(),
            websocket_addr: None,
            max_clients: 100,
            auth_token: None,
            heartbeat_interval_secs: 5,
            delta_batch_size: 100,
            enable_compression: false,
            conflict_strategy: ConflictStrategy::ServerAuthoritative,
            broadcast_throttle_ms: 10,
            broadcast_scheduler_interval_ms: 100,
        }
    }
}

/// Main replication manager that orchestrates client connections,
/// delta broadcasting, and sync protocols.
pub struct ReplicationManager {
    config: ReplicationConfig,
    client_manager: Arc<ClientManager>,
    broadcast_queue: Arc<BroadcastQueue>,
    conflict_resolver: conflict::ConflictResolver,
    _full_sync: FullSyncProtocol,
    _incremental_sync: IncrementalSyncProtocol,
    /// Shutdown signal sender.
    shutdown_tx: watch::Sender<bool>,
    /// Background tasks.
    tasks: Vec<JoinHandle<Result<()>>>,
}

impl ReplicationManager {
    /// Creates a new replication manager with the given configuration.
    pub fn new(config: ReplicationConfig) -> Self {
        let client_manager = Arc::new(ClientManager::new(config.max_clients));
        let broadcast_queue = Arc::new(BroadcastQueue::new(config.delta_batch_size));
        // Set client manager in broadcast queue.
        // We need mutable access; we'll store broadcast_queue as mutable later.
        // For now, we'll set after creation using a setter.
        let conflict_resolver = conflict::ConflictResolver::new(config.conflict_strategy);
        let _full_sync = FullSyncProtocol::default();
        let _incremental_sync = IncrementalSyncProtocol::default();
        let (shutdown_tx, _) = watch::channel(false);

        Self {
            config,
            client_manager,
            broadcast_queue,
            conflict_resolver,
            _full_sync,
            _incremental_sync,
            shutdown_tx,
            tasks: Vec::new(),
        }
    }

    /// Starts listening for client connections (TCP and optionally WebSocket).
    pub async fn start(&mut self) -> Result<()> {
        // Set client manager in broadcast queue (requires mutability)
        let queue = Arc::get_mut(&mut self.broadcast_queue).unwrap();
        queue.set_client_manager(self.client_manager.clone()).await;

        // Start TCP listener
        let listener_addr = self.config.listen_addr.clone();
        let client_manager = self.client_manager.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let listener_task = tokio::spawn(async move {
            Self::run_tcp_listener(&listener_addr, client_manager, shutdown_rx).await
        });
        self.tasks.push(listener_task);

        // Start broadcast scheduler
        let broadcast_queue = self.broadcast_queue.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let scheduler_interval =
            std::time::Duration::from_millis(self.config.broadcast_scheduler_interval_ms);
        let scheduler_task = tokio::spawn(async move {
            let mut scheduler =
                BroadcastScheduler::new(broadcast_queue, scheduler_interval, shutdown_rx);
            scheduler.run().await
        });
        self.tasks.push(scheduler_task);

        log::info!("Replication manager started on {}", self.config.listen_addr);
        Ok(())
    }

    /// Stops the replication manager and disconnects all clients.
    pub async fn stop(&mut self) -> Result<()> {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);
        // Wait for tasks to finish
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        // Disconnect all clients
        // TODO: implement disconnect all
        log::info!("Replication manager stopped");
        Ok(())
    }

    /// Broadcasts a delta to all connected clients.
    pub async fn broadcast_delta(&self, delta: crate::storage::delta::Delta) -> Result<()> {
        self.broadcast_queue.enqueue(delta).await
    }

    /// Returns the number of connected clients.
    pub async fn connected_clients(&self) -> usize {
        self.client_manager.connected_count().await
    }

    /// Returns serializable information for all connected clients.
    pub async fn get_clients(&self) -> Vec<self::client::ClientInfo> {
        self.client_manager.get_clients().await
    }

    /// Returns a reference to the client manager.
    pub fn client_manager(&self) -> &Arc<ClientManager> {
        &self.client_manager
    }

    /// Returns a reference to the broadcast queue.
    pub fn broadcast_queue(&self) -> &Arc<BroadcastQueue> {
        &self.broadcast_queue
    }

    /// Returns the number of pending delta batches in the broadcast queue.
    pub async fn pending_delta_count(&self) -> usize {
        self.broadcast_queue.pending_count().await
    }

    /// Returns recent delta log entries for monitoring.
    pub async fn delta_log_entries(&self) -> Vec<DeltaLogEntry> {
        self.broadcast_queue.delta_log_entries().await
    }

    /// Returns a reference to the conflict resolver.
    pub fn conflict_resolver(&self) -> &conflict::ConflictResolver {
        &self.conflict_resolver
    }

    /// Returns a mutable reference to the conflict resolver.
    pub fn conflict_resolver_mut(&mut self) -> &mut conflict::ConflictResolver {
        &mut self.conflict_resolver
    }

    /// Runs the TCP listener loop.
    async fn run_tcp_listener(
        addr: &str,
        client_manager: Arc<ClientManager>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            EcsDbError::IoError(std::io::Error::other(format!(
                "Failed to bind to {}: {}",
                addr, e
            )))
        })?;

        log::info!("Replication TCP listener started on {}", addr);

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let manager = client_manager.clone();
                            tokio::spawn(async move {
                                log::debug!("New client connection from {}", peer_addr);
                                match manager.add_client(peer_addr, stream).await {
                                    Ok(client_id) => {
                                        log::info!("Client {} connected", client_id.0);
                                        // TODO: start client handler task
                                    }
                                    Err(e) => log::warn!("Failed to add client: {}", e),
                                }
                            });
                        }
                        Err(e) => log::error!("Accept error: {}", e),
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        log::info!("TCP listener shutting down");
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for ReplicationManager {
    fn drop(&mut self) {
        // Attempt to stop if still running
        if !self.tasks.is_empty() {
            let _ = self.shutdown_tx.send(true);
        }
    }
}
