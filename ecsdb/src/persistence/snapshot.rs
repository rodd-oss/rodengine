//! Snapshot format for persistent storage of database state.

use crate::entity::{ArchetypeRegistry, EntityRegistry};
use crate::error::Result;
use crate::schema::DatabaseSchema;
use bincode;
use crc32fast;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::spawn_blocking;

/// Magic number for snapshot files: "ECSSNAP" in ASCII
const SNAPSHOT_MAGIC: [u8; 8] = *b"ECSSNAP\x00";
/// Current snapshot format version
const SNAPSHOT_VERSION: u32 = 1;
/// Flags bit 0: compressed with zstd
const FLAG_COMPRESSED: u32 = 1 << 0;

/// Header at the start of a snapshot file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotHeader {
    magic: [u8; 8],
    version: u32,
    flags: u32,
    checksum: u32, // CRC32 of the data after header (compressed if flags indicate)
    reserved: [u8; 8],
}

impl SnapshotHeader {
    fn new(flags: u32, checksum: u32) -> Self {
        Self {
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            flags,
            checksum,
            reserved: [0; 8],
        }
    }

    /// Validates the header's magic and version.
    fn validate(&self) -> Result<()> {
        if self.magic != SNAPSHOT_MAGIC {
            return Err(crate::error::EcsDbError::SnapshotError(
                "Invalid snapshot magic".into(),
            ));
        }
        if self.version != SNAPSHOT_VERSION {
            return Err(crate::error::EcsDbError::SnapshotError(format!(
                "Unsupported snapshot version {}",
                self.version
            )));
        }
        Ok(())
    }
}

/// Snapshot of a single component table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSnapshot {
    /// Table ID (unique per component type)
    pub table_id: u16,
    /// Table name (must match schema)
    pub table_name: String,
    /// Size of each record in bytes
    pub record_size: usize,
    /// Raw buffer data (read buffer snapshot)
    pub buffer_data: Vec<u8>,
    /// Mapping from entity ID to byte offset within buffer_data
    pub entity_mapping: Vec<(u64, usize)>,
    /// List of free slot offsets (in bytes) in the buffer
    pub free_slots: Vec<usize>,
    /// Number of active records (should equal entity_mapping.len())
    pub active_count: usize,
}

/// Complete database snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSnapshot {
    /// Database schema (defines tables, fields, types)
    pub schema: DatabaseSchema,
    /// Entity registry state
    pub entity_registry: EntityRegistry,
    /// Archetype registry state
    pub archetype_registry: ArchetypeRegistry,
    /// Snapshots of all component tables
    pub tables: Vec<TableSnapshot>,
}

impl DatabaseSnapshot {
    /// Creates a snapshot from the current database state.
    /// This should be called after a commit to ensure consistency.
    pub fn from_database(db: &crate::db::Database) -> Result<Self> {
        db.create_snapshot()
    }

    /// Writes the snapshot to a file, optionally compressing with zstd.
    pub fn write_to_file(&self, path: &Path, compress: bool) -> Result<()> {
        // Serialize snapshot to bytes
        let snapshot_bytes = bincode::serialize(self)?;
        let (flags, data) = if compress {
            // Compress with zstd level 3 (good balance)
            let compressed = zstd::encode_all(snapshot_bytes.as_slice(), 3)
                .map_err(|e| crate::error::EcsDbError::CompressionError(e.to_string()))?;
            (FLAG_COMPRESSED, compressed)
        } else {
            (0, snapshot_bytes)
        };
        // Compute checksum of data (compressed or not)
        let checksum = compute_checksum(&data);
        let header = SnapshotHeader::new(flags, checksum);
        let header_bytes = bincode::serialize(&header)?;
        // Write header + data to file
        let mut file = File::create(path)?;
        file.write_all(&header_bytes)?;
        file.write_all(&data)?;
        Ok(())
    }

    /// Async version of `write_to_file`.
    pub async fn write_to_file_async(&self, path: &Path, compress: bool) -> Result<()> {
        // Serialize snapshot to bytes (CPU-bound, can stay synchronous)
        let snapshot_bytes = bincode::serialize(self)?;
        let (flags, data) = if compress {
            // Compress with zstd level 3 (CPU-bound, run in blocking task)
            let compressed = spawn_blocking(move || {
                zstd::encode_all(snapshot_bytes.as_slice(), 3)
                    .map_err(|e| crate::error::EcsDbError::CompressionError(e.to_string()))
            }).await??;
            (FLAG_COMPRESSED, compressed)
        } else {
            (0, snapshot_bytes)
        };
        // Compute checksum of data (fast)
        let checksum = compute_checksum(&data);
        let header = SnapshotHeader::new(flags, checksum);
        let header_bytes = bincode::serialize(&header)?;
        // Write header + data to file asynchronously
        let mut file = TokioFile::create(path).await?;
        file.write_all(&header_bytes).await?;
        file.write_all(&data).await?;
        Ok(())
    }

    /// Loads a snapshot from a file, decompressing if necessary.
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        // Read header (fixed size: 8+4+4+4+8 = 28 bytes)
        let mut header_buf = [0u8; 28];
        file.read_exact(&mut header_buf)?;
        let header: SnapshotHeader = bincode::deserialize(&header_buf)?;
        header.validate()?;
        // Read the rest of the file
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        // Verify checksum
        let computed = compute_checksum(&data);
        if computed != header.checksum {
            return Err(crate::error::EcsDbError::SnapshotError(
                "Checksum mismatch".into(),
            ));
        }
        // Decompress if needed
        let snapshot_bytes = if header.flags & FLAG_COMPRESSED != 0 {
            zstd::decode_all(&data[..])
                .map_err(|e| crate::error::EcsDbError::CompressionError(e.to_string()))?
        } else {
            data
        };
        // Deserialize snapshot
        let snapshot: DatabaseSnapshot = bincode::deserialize(&snapshot_bytes)?;
        Ok(snapshot)
    }

    /// Async version of `from_file`.
    pub async fn from_file_async(path: &Path) -> Result<Self> {
        let mut file = TokioFile::open(path).await?;
        // Read header (fixed size: 8+4+4+4+8 = 28 bytes)
        let mut header_buf = [0u8; 28];
        file.read_exact(&mut header_buf).await?;
        let header: SnapshotHeader = bincode::deserialize(&header_buf)?;
        header.validate()?;
        // Read the rest of the file
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        // Verify checksum
        let computed = compute_checksum(&data);
        if computed != header.checksum {
            return Err(crate::error::EcsDbError::SnapshotError(
                "Checksum mismatch".into(),
            ));
        }
        // Decompress if needed (CPU-bound, run in blocking task)
        let snapshot_bytes = if header.flags & FLAG_COMPRESSED != 0 {
            spawn_blocking(move || {
                zstd::decode_all(&data[..])
                    .map_err(|e| crate::error::EcsDbError::CompressionError(e.to_string()))
            }).await??
        } else {
            data
        };
        // Deserialize snapshot (CPU-bound, but small)
        let snapshot: DatabaseSnapshot = bincode::deserialize(&snapshot_bytes)?;
        Ok(snapshot)
    }

    /// Restores the snapshot into a new Database instance.
    pub fn restore(self) -> Result<crate::db::Database> {
        crate::db::Database::from_snapshot(self)
    }
}

/// Computes CRC32 checksum of data.
fn compute_checksum(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}
