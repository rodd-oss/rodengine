//! Client connection management for replication.
//!
//! Handles TCP (and optionally WebSocket) client connections,
//! authentication, session state, and lifecycle.

use crate::error::{EcsDbError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Unique client identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(pub Uuid);

impl ClientId {
    pub fn new() -> Self {
        ClientId(Uuid::new_v4())
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::new()
    }
}

/// Client session state.
#[derive(Debug, Clone)]
pub enum ClientState {
    /// Just connected, not authenticated.
    PendingAuth,
    /// Authenticated, waiting for initial sync.
    AwaitingSync,
    /// Synchronized, ready for incremental updates.
    Ready,
    /// Synchronizing (full or incremental).
    Syncing,
    /// Disconnected (zombie session, can be resumed).
    Disconnected,
}

/// Per‑client session data.
#[derive(Clone)]
pub struct ClientSession {
    pub id: ClientId,
    pub addr: SocketAddr,
    pub state: ClientState,
    /// Last known database version the client has acknowledged.
    pub client_version: u64,
    /// Subscribed tables (empty means all).
    pub subscribed_tables: Vec<u16>,
    /// Network socket (TCP or WebSocket).
    pub socket: Option<Arc<RwLock<TcpStream>>>,
    /// Channel for sending messages to the client's writer task.
    pub sender: mpsc::UnboundedSender<ClientMessage>,
}

/// Messages that can be sent to a client.
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Delta batch to apply.
    Delta(crate::storage::delta::Delta),
    /// Full snapshot data.
    Snapshot(Vec<u8>),
    /// Ping heartbeat.
    Ping,
    /// Disconnect request.
    Disconnect,
}

impl ClientSession {
    pub fn new(addr: SocketAddr, stream: TcpStream) -> Self {
        let (sender, _receiver) = mpsc::unbounded_channel();
        Self {
            id: ClientId::new(),
            addr,
            state: ClientState::PendingAuth,
            client_version: 0,
            subscribed_tables: Vec::new(),
            socket: Some(Arc::new(RwLock::new(stream))),
            sender,
        }
    }

    /// Sends a message to the client (non‑blocking).
    pub fn send(&self, msg: ClientMessage) -> Result<()> {
        self.sender.send(msg).map_err(|_| EcsDbError::ChannelClosed)
    }

    /// Closes the network socket.
    pub async fn close(&mut self) {
        if let Some(socket) = self.socket.take() {
            let mut guard = socket.write().await;
            let _ = (&mut *guard).shutdown().await;
        }
    }
}

/// Manages all connected client sessions.
pub struct ClientManager {
    /// Active sessions keyed by client ID.
    sessions: Arc<RwLock<HashMap<ClientId, ClientSession>>>,
    /// Maximum number of concurrent clients.
    max_clients: usize,
}

impl ClientManager {
    pub fn new(max_clients: usize) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            max_clients,
        }
    }

    /// Adds a new client session.
    pub async fn add_client(&self, addr: SocketAddr, stream: TcpStream) -> Result<ClientId> {
        let mut sessions = self.sessions.write().await;
        if sessions.len() >= self.max_clients {
            return Err(EcsDbError::ReplicationError(
                "Maximum client count reached".to_string(),
            ));
        }
        let session = ClientSession::new(addr, stream);
        let id = session.id;
        sessions.insert(id, session);
        Ok(id)
    }

    /// Removes a client session.
    pub async fn remove_client(&self, id: ClientId) -> Option<ClientSession> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&id)
    }

    /// Retrieves a client session (read‑only).
    pub async fn get_client(&self, id: ClientId) -> Option<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions.get(&id).cloned()
    }

    /// Returns the number of connected clients.
    pub async fn connected_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Broadcasts a message to all clients in the given state.
    pub async fn broadcast_to_state(
        &self,
        state: ClientState,
        msg: ClientMessage,
    ) -> Result<usize> {
        let sessions = self.sessions.read().await;
        let mut count = 0;
        for session in sessions.values() {
            if std::mem::discriminant(&session.state) == std::mem::discriminant(&state) {
                session.send(msg.clone())?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Broadcasts a message to all clients.
    pub async fn broadcast(&self, msg: ClientMessage) -> Result<usize> {
        let sessions = self.sessions.read().await;
        let mut count = 0;
        for session in sessions.values() {
            session.send(msg.clone())?;
            count += 1;
        }
        Ok(count)
    }

    /// Updates a client's version.
    pub async fn update_client_version(&self, id: ClientId, version: u64) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&id) {
            session.client_version = version;
        }
        Ok(())
    }

    /// Updates a client's state.
    pub async fn update_client_state(&self, id: ClientId, state: ClientState) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&id) {
            session.state = state;
        }
        Ok(())
    }
}

/// Starts a TCP listener that accepts new clients and adds them to the manager.
pub async fn start_tcp_listener(addr: &str, client_manager: Arc<ClientManager>) -> Result<()> {
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        EcsDbError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to bind to {}: {}", addr, e),
        ))
    })?;

    log::info!("Replication TCP listener started on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let manager = client_manager.clone();
                tokio::spawn(async move {
                    log::debug!("New client connection from {}", addr);
                    match manager.add_client(addr, stream).await {
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
}
