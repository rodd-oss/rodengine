//! Disk-backed write-ahead log with file rotation.

use crate::error::{EcsDbError, Result};
use crate::transaction::wal::{WalEntry, WalOp};
use async_trait::async_trait;
use bincode;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Magic number for WAL files: "ECSWAL\x00\x00" in ASCII
const WAL_MAGIC: [u8; 8] = *b"ECSWAL\x00\x00";
/// Current WAL format version
const WAL_VERSION: u32 = 1;
/// Maximum size of a single WAL file before rotation (default: 64 MiB)
const DEFAULT_MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
/// Size of the header at the start of each WAL file
const HEADER_SIZE: usize = 32;

/// Header at the start of a WAL file.
#[derive(Debug, Clone)]
struct WalFileHeader {
    magic: [u8; 8],
    version: u32,
    flags: u32,
    reserved: [u8; 16],
}

impl WalFileHeader {
    fn new() -> Self {
        Self {
            magic: WAL_MAGIC,
            version: WAL_VERSION,
            flags: 0,
            reserved: [0; 16],
        }
    }

    /// Writes the header to a writer.
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&self.flags.to_le_bytes())?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    /// Reads a header from a reader.
    fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        let mut flags_bytes = [0u8; 4];
        reader.read_exact(&mut flags_bytes)?;
        let flags = u32::from_le_bytes(flags_bytes);
        let mut reserved = [0u8; 16];
        reader.read_exact(&mut reserved)?;

        if magic != WAL_MAGIC {
            return Err(EcsDbError::WalError("Invalid WAL magic".into()));
        }
        if version != WAL_VERSION {
            return Err(EcsDbError::WalError(format!(
                "Unsupported WAL version {}",
                version
            )));
        }

        Ok(Self {
            magic,
            version,
            flags,
            reserved,
        })
    }
}

/// Disk‑backed WAL with file rotation.
pub struct FileWal {
    /// Directory containing WAL files
    dir: PathBuf,
    /// Maximum size per file before rotation
    max_file_size: u64,
    /// Current file writer
    current_file: Option<BufWriter<File>>,
    /// ID (index) of the current file
    current_file_id: u64,
    /// Size of the current file (bytes written)
    current_file_size: u64,
    /// Synchronize every write? (for durability)
    sync_on_write: bool,
    /// Next transaction ID to assign
    next_transaction_id: u64,
    /// In‑memory index of entries for fast lookup (optional, for now empty)
    entries: Vec<WalEntry>,
}

impl FileWal {
    /// Opens or creates a WAL in the given directory.
    pub fn open(dir: impl AsRef<Path>, max_file_size: Option<u64>) -> Result<Self> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;

        // Find the latest WAL file to continue from, or start a new one.
        let (current_file_id, existing_files) = Self::scan_existing_files(dir)?;

        // Load all existing entries from WAL files
        let entries = Self::read_entries_from_files(&existing_files)?;
        let next_transaction_id = entries
            .iter()
            .map(|e| e.transaction_id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        let (current_file, current_file_size) = if let Some(latest_path) = existing_files.last() {
            // Open the latest file for appending
            let file = OpenOptions::new().append(true).open(latest_path)?;
            let size = file.metadata()?.len();
            (Some(BufWriter::new(file)), size)
        } else {
            // No existing files, start new file with id 1
            (None, 0)
        };

        Ok(Self {
            dir: dir.to_path_buf(),
            max_file_size: max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE),
            current_file,
            current_file_id,
            current_file_size,
            sync_on_write: true,
            next_transaction_id,
            entries,
        })
    }

    /// Scans the directory for existing WAL files and returns the next file ID
    /// and sorted list of file paths.
    fn scan_existing_files(dir: &Path) -> Result<(u64, Vec<PathBuf>)> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "wal" {
                        files.push(path);
                    }
                }
            }
        }

        // Sort by file ID (extracted from filename "wal_0001.wal")
        files.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("wal_"))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        });

        let next_id = files
            .last()
            .and_then(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("wal_"))
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .map(|id| id + 1)
            .unwrap_or(1);

        Ok((next_id, files))
    }

    /// Reads all entries from a list of WAL files.
    fn read_entries_from_files(files: &[PathBuf]) -> Result<Vec<WalEntry>> {
        let mut entries = Vec::new();
        for path in files {
            let mut file = File::open(path)?;
            let _header = WalFileHeader::read(&mut file)?;
            // Skip header
            file.seek(SeekFrom::Start(HEADER_SIZE as u64))?;

            loop {
                let mut len_bytes = [0u8; 4];
                if file.read_exact(&mut len_bytes).is_err() {
                    break; // EOF
                }
                let len = u32::from_le_bytes(len_bytes) as usize;
                let mut buffer = vec![0u8; len];
                file.read_exact(&mut buffer)?;
                let entry: WalEntry = bincode::deserialize(&buffer)?;
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Ensures the current file is open; if not, creates a new one.
    fn ensure_file_open(&mut self) -> Result<()> {
        if self.current_file.is_none() {
            let filename = self
                .dir
                .join(format!("wal_{:04}.wal", self.current_file_id));
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&filename)?;
            let mut writer = BufWriter::new(file);
            // Write header if file is newly created (size zero)
            if self.current_file_size == 0 {
                let header = WalFileHeader::new();
                header.write(&mut writer)?;
                self.current_file_size = HEADER_SIZE as u64;
            }
            self.current_file = Some(writer);
        }
        Ok(())
    }

    /// Rotates the current file if its size exceeds the limit.
    fn maybe_rotate(&mut self) -> Result<()> {
        if self.current_file_size >= self.max_file_size {
            self.current_file = None;
            self.current_file_id += 1;
            self.current_file_size = 0;
            self.ensure_file_open()?;
        }
        Ok(())
    }

    /// Appends a serialized entry to the current WAL file.
    fn append_entry(&mut self, entry: &WalEntry) -> Result<()> {
        self.ensure_file_open()?;
        self.maybe_rotate()?;

        let serialized = bincode::serialize(entry)?;
        let len = serialized.len() as u32;
        let writer = self.current_file.as_mut().unwrap();

        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&serialized)?;
        writer.flush()?;
        if self.sync_on_write {
            writer.get_ref().sync_all()?;
        }

        self.current_file_size += 4 + serialized.len() as u64;
        self.entries.push(entry.clone());
        Ok(())
    }

    /// Reads all entries from all WAL files in the directory.
    pub fn read_all_entries(dir: impl AsRef<Path>) -> Result<Vec<WalEntry>> {
        let dir = dir.as_ref();
        let (_, files) = Self::scan_existing_files(dir)?;
        Self::read_entries_from_files(&files)
    }

    /// Returns the path to the current WAL file.
    pub fn current_file_path(&self) -> PathBuf {
        self.dir
            .join(format!("wal_{:04}.wal", self.current_file_id))
    }

    /// Clears the WAL (deletes all files). Use with caution.
    pub fn clear(&mut self) -> Result<()> {
        self.current_file = None;
        self.current_file_id = 1;
        self.current_file_size = 0;
        self.next_transaction_id = 1;
        self.entries.clear();

        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "wal" {
                        fs::remove_file(path)?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl crate::persistence::wal::Wal for FileWal {
    fn begin_transaction(&mut self) -> u64 {
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        id
    }

    async fn log_operation(
        &mut self,
        transaction_id: u64,
        sequence: u32,
        operation: WalOp,
    ) -> Result<()> {
        let entry = WalEntry::new(transaction_id, sequence, operation);
        self.append_entry(&entry)
    }

    async fn log_commit(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self
            .entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .count() as u32;
        let entry = WalEntry::new(transaction_id, seq, WalOp::Commit { transaction_id });
        self.append_entry(&entry)
    }

    async fn log_rollback(&mut self, transaction_id: u64) -> Result<()> {
        let seq = self
            .entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .count() as u32;
        let entry = WalEntry::new(transaction_id, seq, WalOp::Rollback { transaction_id });
        self.append_entry(&entry)
    }

    fn entries_for_transaction(&self, transaction_id: u64) -> Vec<&WalEntry> {
        self.entries
            .iter()
            .filter(|e| e.transaction_id == transaction_id)
            .collect()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn clear(&mut self) {
        self.clear().expect("Failed to clear WAL files");
    }

    async fn sync(&self) -> Result<()> {
        if let Some(writer) = &self.current_file {
            writer.get_ref().sync_all()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::wal::Wal;
    use tempfile::tempdir;

    #[test]
    fn test_wal_file_header() {
        let mut buffer = Vec::new();
        let header = WalFileHeader::new();
        header.write(&mut buffer).unwrap();
        assert_eq!(buffer.len(), HEADER_SIZE);

        let mut reader = std::io::Cursor::new(buffer);
        let read_header = WalFileHeader::read(&mut reader).unwrap();
        assert_eq!(read_header.magic, WAL_MAGIC);
        assert_eq!(read_header.version, WAL_VERSION);
    }

    #[tokio::test]
    async fn test_file_wal_basic() {
        let temp_dir = tempdir().unwrap();
        let mut wal = FileWal::open(temp_dir.path(), Some(1024)).unwrap();

        let txn_id = wal.begin_transaction();
        wal.log_operation(
            txn_id,
            0,
            WalOp::Insert {
                table_id: 1,
                entity_id: 100,
                data: vec![1, 2, 3],
            },
        )
        .await
        .unwrap();
        wal.log_commit(txn_id).await.unwrap();
        wal.sync().await.unwrap();

        assert_eq!(wal.len(), 2);
        let entries = wal.entries_for_transaction(txn_id);
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_file_wal_rotation() {
        let temp_dir = tempdir().unwrap();
        // Set tiny max size to force rotation
        let mut wal = FileWal::open(temp_dir.path(), Some(128)).unwrap();

        // Write enough entries to exceed 128 bytes
        for i in 0..10 {
            let txn_id = wal.begin_transaction();
            wal.log_operation(
                txn_id,
                0,
                WalOp::Insert {
                    table_id: 1,
                    entity_id: i,
                    data: vec![1, 2, 3],
                },
            )
            .await
            .unwrap();
            wal.log_commit(txn_id).await.unwrap();
        }
        wal.sync().await.unwrap();

        // Should have created multiple files
        let files: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wal"))
            .collect();
        assert!(files.len() > 1);
    }

    #[tokio::test]
    async fn test_wal_replay() {
        let temp_dir = tempdir().unwrap();
        let mut wal = FileWal::open(temp_dir.path(), None).unwrap();

        let txn_id = wal.begin_transaction();
        wal.log_operation(
            txn_id,
            0,
            WalOp::Update {
                table_id: 2,
                entity_id: 200,
                data: vec![4, 5, 6],
            },
        )
        .await
        .unwrap();
        wal.log_commit(txn_id).await.unwrap();
        wal.sync().await.unwrap();

        // Read entries back using static method
        let entries = FileWal::read_all_entries(temp_dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
        match &entries[0].operation {
            WalOp::Update { table_id, .. } => assert_eq!(*table_id, 2),
            _ => panic!("expected Update"),
        }
    }
}
