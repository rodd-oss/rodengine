//! Full‑sync and incremental sync protocols.
//!
//! Handles initial synchronization of the entire database state,
//! chunked transfer for large snapshots, and incremental delta catch‑up.

use crate::error::{EcsDbError, Result};
use crate::replication::client::{ClientId, ClientManager, ClientMessage, ClientState};
use crate::storage::delta::{Delta, DeltaOp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Full‑sync message containing schema, snapshot, and current version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSyncMessage {
    /// Schema TOML (as string).
    pub schema_toml: String,
    /// Snapshot data (compressed).
    pub snapshot_data: Vec<u8>,
    /// Current database version.
    pub version: u64,
    /// Total number of chunks.
    pub total_chunks: usize,
    /// Current chunk index (0‑based).
    pub chunk_index: usize,
    /// Checksum of the whole snapshot (optional).
    pub checksum: Option<u32>,
}

/// Incremental sync message containing a batch of deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalSyncMessage {
    /// Starting version (inclusive).
    pub from_version: u64,
    /// Ending version (inclusive).
    pub to_version: u64,
    /// Deltas for each version in range.
    pub deltas: Vec<Delta>,
    /// Whether this is a catch‑up batch (client was behind).
    pub catch_up: bool,
}

/// Progress update during full sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub client_id: ClientId,
    pub total_chunks: usize,
    pub completed_chunks: usize,
    pub estimated_remaining_secs: f64,
}

/// Full‑sync protocol implementation.
pub struct FullSyncProtocol {
    /// Maximum chunk size in bytes.
    max_chunk_size: usize,
    /// Compression level for snapshot (0 = none).
    compression_level: i32,
}

impl Default for FullSyncProtocol {
    fn default() -> Self {
        Self {
            max_chunk_size: 1024 * 1024, // 1 MB
            compression_level: 3,
        }
    }
}

impl FullSyncProtocol {
    /// Creates a full‑sync message from a database snapshot.
    pub fn create_full_sync(
        &self,
        schema_toml: String,
        snapshot_data: Vec<u8>,
        version: u64,
    ) -> Vec<FullSyncMessage> {
        let chunks = self.chunk_data(snapshot_data);
        let total_chunks = chunks.len();
        chunks
            .into_iter()
            .enumerate()
            .map(|(index, chunk)| FullSyncMessage {
                schema_toml: if index == 0 {
                    schema_toml.clone()
                } else {
                    String::new() // Only first chunk carries schema
                },
                snapshot_data: chunk,
                version,
                total_chunks,
                chunk_index: index,
                checksum: None, // TODO: compute
            })
            .collect()
    }

    /// Splits data into chunks of at most `max_chunk_size`.
    fn chunk_data(&self, data: Vec<u8>) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < data.len() {
            let end = std::cmp::min(start + self.max_chunk_size, data.len());
            chunks.push(data[start..end].to_vec());
            start = end;
        }
        chunks
    }

    /// Sends full sync to a client via the client manager.
    pub async fn send_to_client(
        &self,
        client_manager: Arc<ClientManager>,
        client_id: ClientId,
        schema_toml: String,
        snapshot_data: Vec<u8>,
        version: u64,
    ) -> Result<()> {
        let messages = self.create_full_sync(schema_toml, snapshot_data, version);
        for msg in messages {
            let bytes = bincode::serialize(&msg)?;
            if let Some(client) = client_manager.get_client(client_id).await {
                client.send(ClientMessage::Snapshot(bytes))?;
            } else {
                return Err(EcsDbError::ReplicationError("Client not found".to_string()));
            }
        }
        Ok(())
    }

    /// Updates client state after receiving a chunk acknowledgement.
    pub async fn handle_chunk_ack(
        &self,
        client_manager: Arc<ClientManager>,
        client_id: ClientId,
        chunk_index: usize,
        total_chunks: usize,
    ) -> Result<()> {
        // If all chunks received, transition client to ready state.
        if chunk_index == total_chunks - 1 {
            client_manager
                .update_client_state(client_id, ClientState::Ready)
                .await?;
            log::info!("Client {} completed full sync", client_id.0);
        }
        Ok(())
    }
}

/// Incremental sync protocol implementation.
pub struct IncrementalSyncProtocol {
    /// Maximum number of deltas per batch.
    max_deltas_per_batch: usize,
    /// Delta archive for catch‑up (stores recent deltas).
    delta_archive: Mutex<VecDeque<Delta>>,
    /// Maximum archive size (number of deltas).
    max_archive_size: usize,
}

impl Default for IncrementalSyncProtocol {
    fn default() -> Self {
        Self {
            max_deltas_per_batch: 100,
            delta_archive: Mutex::new(VecDeque::with_capacity(1000)),
            max_archive_size: 1000,
        }
    }
}

impl IncrementalSyncProtocol {
    /// Archives a delta for future catch‑up.
    pub async fn archive_delta(&self, delta: Delta) {
        let mut archive = self.delta_archive.lock().await;
        archive.push_back(delta);
        if archive.len() > self.max_archive_size {
            archive.pop_front();
        }
    }

    /// Creates an incremental sync message from a version range.
    pub async fn create_incremental_sync(
        &self,
        from_version: u64,
        to_version: u64,
    ) -> Option<IncrementalSyncMessage> {
        let archive = self.delta_archive.lock().await;
        // Collect deltas within the range (inclusive).
        let mut deltas = Vec::new();
        for delta in archive.iter() {
            if delta.version >= from_version && delta.version <= to_version {
                deltas.push(delta.clone());
            }
        }
        if deltas.is_empty() {
            return None;
        }
        Some(IncrementalSyncMessage {
            from_version,
            to_version,
            deltas,
            catch_up: true,
        })
    }

    /// Sends incremental sync to a client.
    pub async fn send_to_client(
        &self,
        client_manager: Arc<ClientManager>,
        client_id: ClientId,
        from_version: u64,
        to_version: u64,
    ) -> Result<()> {
        if let Some(msg) = self.create_incremental_sync(from_version, to_version).await {
            let bytes = bincode::serialize(&msg)?;
            if let Some(client) = client_manager.get_client(client_id).await {
                client.send(ClientMessage::Delta(Delta {
                    ops: vec![], // placeholder, we'll send raw bytes
                    version: to_version,
                    timestamp: 0,
                }))?;
                // TODO: need a dedicated message type for incremental sync.
                // For now, we'll reuse Delta message with special encoding.
            }
        }
        Ok(())
    }

    /// Handles a client's request for deltas from a specific version.
    pub async fn handle_client_request(
        &self,
        client_manager: Arc<ClientManager>,
        client_id: ClientId,
        requested_version: u64,
    ) -> Result<()> {
        // Get client's current version.
        let client = client_manager
            .get_client(client_id)
            .await
            .ok_or_else(|| EcsDbError::ReplicationError("Client not found".to_string()))?;
        let client_version = client.client_version;
        if requested_version > client_version {
            // Client is behind, send catch‑up deltas.
            self.send_to_client(
                client_manager,
                client_id,
                client_version + 1,
                requested_version,
            )
            .await?;
        } else {
            // Client already up‑to‑date.
            log::debug!(
                "Client {} already at version {}",
                client_id.0,
                client_version
            );
        }
        Ok(())
    }
}

/// Heartbeat and keepalive mechanism.
pub struct HeartbeatManager {
    interval_secs: u64,
    timeout_secs: u64,
}

impl HeartbeatManager {
    pub fn new(interval_secs: u64, timeout_secs: u64) -> Self {
        Self {
            interval_secs,
            timeout_secs,
        }
    }

    /// Starts heartbeat task for a client.
    pub async fn start_for_client(
        &self,
        client_manager: Arc<ClientManager>,
        client_id: ClientId,
    ) -> Result<()> {
        // TODO: spawn task that periodically sends ping and checks for timeout.
        Ok(())
    }
}
