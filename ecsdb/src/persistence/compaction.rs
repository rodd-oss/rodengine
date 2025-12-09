//! Compaction worker for merging snapshots and WAL files.

use crate::error::{EcsDbError, Result};
use crate::persistence::snapshot::DatabaseSnapshot;
use crate::persistence::file_wal::FileWal;
use crate::transaction::wal::WalOp;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Compaction worker that periodically merges old snapshots with WAL files.
pub struct CompactionWorker {
    config: crate::config::PersistenceConfig,
    running: bool,
}

impl CompactionWorker {
    /// Creates a new compaction worker with the given configuration.
    pub fn new(config: crate::config::PersistenceConfig) -> Self {
        Self {
            config,
            running: false,
        }
    }

    /// Runs a single compaction cycle.
    /// This merges the oldest snapshot with WAL files that are older than the next snapshot.
    /// The merged snapshot is saved as a new snapshot, and the old snapshot and merged WAL files
    /// are moved to the archive directory (or deleted if archiving is disabled).
    pub fn run_compaction_cycle(&mut self) -> Result<()> {
        eprintln!("Starting compaction cycle");
        
        // 1. List snapshot files
        let snapshots = Self::list_snapshot_files(&self.config.snapshot_dir)?;
        if snapshots.len() < 2 {
            eprintln!("Not enough snapshots to compact (need at least 2)");
            return Ok(());
        }

        // 2. Identify oldest snapshot and the next snapshot
        let (oldest_path, oldest_version) = snapshots.first().unwrap().clone();
        let (_next_path, next_version) = snapshots.get(1).unwrap().clone();

        // 3. List WAL files that belong to the interval [oldest_version, next_version)
        let wal_files = Self::list_wal_files(&self.config.wal_dir)?;
        let relevant_wal_files: Vec<_> = wal_files
            .into_iter()
            .filter(|(_, version)| *version >= oldest_version && *version < next_version)
            .map(|(path, _)| path)
            .collect();

        if relevant_wal_files.is_empty() {
            eprintln!("No WAL files to compact for snapshot version {}", oldest_version);
            return Ok(());
        }

        // 4. Load oldest snapshot
        let mut snapshot = DatabaseSnapshot::from_file(&oldest_path)?;

        // 5. Replay relevant WAL entries onto snapshot
        for wal_path in relevant_wal_files.iter() {
            Self::replay_wal_file_onto_snapshot(wal_path, &mut snapshot)?;
        }

        // 6. Save merged snapshot with a new version (use next_version?)
        let merged_version = next_version; // we'll keep the next snapshot's version as merged
        let merged_path = self.config.snapshot_dir.join(format!("snapshot_{:016x}.bin", merged_version));
        snapshot.write_to_file(&merged_path, self.config.compress_snapshots)?;
         eprintln!("Saved merged snapshot to {:?}", merged_path);

        // 7. Move old snapshot and WAL files to archive (or delete)
        if self.config.compress_archived_wal {
            // Move to archive directory
            let archive_dir = &self.config.archive_dir;
            fs::create_dir_all(archive_dir)?;
            let archive_old_snapshot = archive_dir.join(oldest_path.file_name().unwrap());
            fs::rename(&oldest_path, archive_old_snapshot)?;
            for wal_path in relevant_wal_files {
                let archive_wal = archive_dir.join(wal_path.file_name().unwrap());
                fs::rename(&wal_path, archive_wal)?;
            }
        } else {
            // Delete old files
            fs::remove_file(&oldest_path)?;
            for wal_path in relevant_wal_files {
                fs::remove_file(&wal_path)?;
            }
        }

         eprintln!("Compaction cycle completed successfully");
        Ok(())
    }

    /// Lists snapshot files in the directory, sorted by version.
    /// Snapshot files are expected to be named `snapshot_<version>.bin` where version is a hex number.
    fn list_snapshot_files(dir: &Path) -> Result<Vec<(PathBuf, u64)>> {
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if filename.starts_with("snapshot_") && filename.ends_with(".bin") {
                        let version_str = &filename[9..filename.len() - 4];
                        if let Ok(version) = u64::from_str_radix(version_str, 16) {
                            snapshots.push((path, version));
                        }
                    }
                }
            }
        }
        snapshots.sort_by_key(|(_, version)| *version);
        Ok(snapshots)
    }

    /// Lists WAL files in the directory, sorted by file ID.
    /// WAL files are named `wal_<id>.wal` where id is a decimal number.
    fn list_wal_files(dir: &Path) -> Result<Vec<(PathBuf, u64)>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if filename.starts_with("wal_") && filename.ends_with(".wal") {
                        let id_str = &filename[4..filename.len() - 4];
                        if let Ok(id) = id_str.parse::<u64>() {
                            files.push((path, id));
                        }
                    }
                }
            }
        }
        files.sort_by_key(|(_, id)| *id);
        Ok(files)
    }

    /// Replays all entries from a single WAL file onto a snapshot.
    fn replay_wal_file_onto_snapshot(wal_path: &Path, snapshot: &mut DatabaseSnapshot) -> Result<()> {
        let entries = FileWal::read_all_entries(wal_path)?;
        // Group operations by transaction, apply only committed transactions
        use std::collections::HashMap;
        let mut pending_ops: HashMap<u64, Vec<WalOp>> = HashMap::new();
        let mut committed_transactions = Vec::new();
        for entry in entries {
            match entry.operation {
                WalOp::Commit { transaction_id } => {
                    // Mark transaction as committed
                    if let Some(ops) = pending_ops.remove(&transaction_id) {
                        committed_transactions.push((transaction_id, ops));
                    }
                }
                WalOp::Rollback { transaction_id } => {
                    // Discard pending ops for this transaction
                    pending_ops.remove(&transaction_id);
                }
                op => {
                    // Insert, Update, Delete: accumulate per transaction
                    pending_ops.entry(entry.transaction_id).or_default().push(op);
                }
            }
        }
        // Apply all committed operations
        for (transaction_id, ops) in committed_transactions {
            for op in ops {
                snapshot.apply_wal_op(&op)?;
            }
            // Update snapshot version to max transaction id
            if transaction_id > snapshot.version {
                snapshot.version = transaction_id;
            }
        }
        Ok(())
    }

    /// Starts the compaction worker as a background task.
    /// The worker runs periodically according to `config.compaction_interval_seconds`.
    pub async fn start(mut self) -> Result<()> {
        use tokio::time::{sleep, Duration};

        self.running = true;
        let interval = Duration::from_secs(self.config.compaction_interval_seconds);
        while self.running {
            sleep(interval).await;
            if let Err(e) = self.run_compaction_cycle() {
                 eprintln!("Compaction cycle failed: {}", e);
            }
        }
        Ok(())
    }

    /// Stops the compaction worker.
    pub fn stop(&mut self) {
        self.running = false;
    }
}

/// Offline compaction utility: merges all snapshots and WAL files into a single snapshot.
/// This is useful for manual maintenance and reduces disk space.
pub fn compact_offline(snapshot_dir: &Path, wal_dir: &Path, output_path: &Path, compress: bool) -> Result<()> {
    // Implementation similar to run_compaction_cycle but merges everything.
    // For simplicity, we'll just call the worker's method after creating a dummy config.
    let config = crate::config::PersistenceConfig {
        snapshot_dir: snapshot_dir.to_path_buf(),
        wal_dir: wal_dir.to_path_buf(),
        archive_dir: wal_dir.join("archive"),
        ..Default::default()
    };
    let mut worker = CompactionWorker::new(config);
    // We cannot easily reuse run_compaction_cycle because it expects versioned snapshots.
    // Instead, we'll implement a simpler merge-all.
    // For now, placeholder.
    Err(EcsDbError::CompactionError("Offline compaction not yet implemented".into()))
}