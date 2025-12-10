//! Network synchronization client.

use crate::error::{ClientError, Result};
use tokio::net::TcpStream;

/// Network client that maintains a connection to the server.
pub struct SyncClient {
    #[allow(dead_code)]
    stream: TcpStream,
}

impl SyncClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await.map_err(|e| {
            ClientError::NetworkError(format!("Failed to connect to {}: {}", addr, e))
        })?;
        Ok(Self { stream })
    }
}
