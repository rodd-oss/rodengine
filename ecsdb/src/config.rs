//! Configuration system for database persistence.
//!
//! Supports TOML config files, environment variable overrides, and defaults.

use crate::error::{EcsDbError, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuration for persistence (snapshots, WAL, compaction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Directory for snapshot files (default: "./snapshots")
    pub snapshot_dir: PathBuf,
    /// Directory for write‑ahead log files (default: "./wal")
    pub wal_dir: PathBuf,
    /// Directory for archived WAL files after compaction (default: "./wal/archive")
    pub archive_dir: PathBuf,
    /// Maximum size per WAL file before rotation, in bytes (default: 64 MiB)
    pub max_wal_file_size: u64,
    /// Whether to sync each WAL write to disk (default: true)
    pub sync_on_write: bool,
    /// Snapshot interval in number of committed transactions (default: 1000)
    pub snapshot_interval_transactions: u64,
    /// Snapshot interval in seconds (default: 3600 = 1 hour)
    pub snapshot_interval_seconds: u64,
    /// Enable compression for snapshots (default: true)
    pub compress_snapshots: bool,
    /// Compression level for zstd (1–22, default: 3)
    pub snapshot_compression_level: i32,
    /// Enable compression for archived WAL files (default: false)
    pub compress_archived_wal: bool,
    /// Interval in seconds between compaction runs (default: 86400 = 1 day)
    pub compaction_interval_seconds: u64,
    /// Minimum number of WAL files before compaction is triggered (default: 5)
    pub min_wal_files_for_compaction: usize,
    /// Keep at least this many snapshots after compaction (default: 2)
    pub keep_snapshots: usize,
    /// Keep at least this many archived WAL files after compaction (default: 1)
    pub keep_archived_wal_files: usize,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("./snapshots"),
            wal_dir: PathBuf::from("./wal"),
            archive_dir: PathBuf::from("./wal/archive"),
            max_wal_file_size: 64 * 1024 * 1024, // 64 MiB
            sync_on_write: true,
            snapshot_interval_transactions: 1000,
            snapshot_interval_seconds: 3600, // 1 hour
            compress_snapshots: true,
            snapshot_compression_level: 3,
            compress_archived_wal: false,
            compaction_interval_seconds: 86400, // 1 day
            min_wal_files_for_compaction: 5,
            keep_snapshots: 2,
            keep_archived_wal_files: 1,
        }
    }
}

impl PersistenceConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| EcsDbError::ConfigError(format!("Failed to read config file: {}", e)))?;
        Self::from_toml(&content)
    }

    /// Parses configuration from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str)
            .map_err(|e| EcsDbError::ConfigError(format!("Invalid TOML: {}", e)))
    }

    /// Saves the configuration to a TOML file.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let toml = toml::to_string_pretty(self)
            .map_err(|e| EcsDbError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(path.as_ref(), toml)
            .map_err(|e| EcsDbError::ConfigError(format!("Failed to write config file: {}", e)))?;
        Ok(())
    }

    /// Applies environment variable overrides.
    /// Environment variables are prefixed with `ECDB_` (ECS Database).
    /// Example: `ECDB_SNAPSHOT_DIR=/path` overrides `snapshot_dir`.
    pub fn apply_env_overrides(&mut self) -> Result<()> {
        if let Ok(val) = env::var("ECDB_SNAPSHOT_DIR") {
            self.snapshot_dir = PathBuf::from(val);
        }
        if let Ok(val) = env::var("ECDB_WAL_DIR") {
            self.wal_dir = PathBuf::from(val);
        }
        if let Ok(val) = env::var("ECDB_ARCHIVE_DIR") {
            self.archive_dir = PathBuf::from(val);
        }
        if let Ok(val) = env::var("ECDB_MAX_WAL_FILE_SIZE") {
            self.max_wal_file_size = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid max_wal_file_size: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_SYNC_ON_WRITE") {
            self.sync_on_write = val
                .parse()
                .map_err(|_| EcsDbError::ConfigError(format!("Invalid sync_on_write: {}", val)))?;
        }
        if let Ok(val) = env::var("ECDB_SNAPSHOT_INTERVAL_TX") {
            self.snapshot_interval_transactions = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid snapshot_interval_transactions: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_SNAPSHOT_INTERVAL_SEC") {
            self.snapshot_interval_seconds = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid snapshot_interval_seconds: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_COMPRESS_SNAPSHOTS") {
            self.compress_snapshots = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid compress_snapshots: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_SNAPSHOT_COMPRESSION_LEVEL") {
            self.snapshot_compression_level = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid snapshot_compression_level: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_COMPRESS_ARCHIVED_WAL") {
            self.compress_archived_wal = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid compress_archived_wal: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_COMPACTION_INTERVAL_SEC") {
            self.compaction_interval_seconds = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid compaction_interval_seconds: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_MIN_WAL_FILES_FOR_COMPACTION") {
            self.min_wal_files_for_compaction = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid min_wal_files_for_compaction: {}", val))
            })?;
        }
        if let Ok(val) = env::var("ECDB_KEEP_SNAPSHOTS") {
            self.keep_snapshots = val
                .parse()
                .map_err(|_| EcsDbError::ConfigError(format!("Invalid keep_snapshots: {}", val)))?;
        }
        if let Ok(val) = env::var("ECDB_KEEP_ARCHIVED_WAL_FILES") {
            self.keep_archived_wal_files = val.parse().map_err(|_| {
                EcsDbError::ConfigError(format!("Invalid keep_archived_wal_files: {}", val))
            })?;
        }
        Ok(())
    }

    /// Returns the snapshot interval as a `Duration`.
    pub fn snapshot_interval(&self) -> Duration {
        Duration::from_secs(self.snapshot_interval_seconds)
    }

    /// Returns the compaction interval as a `Duration`.
    pub fn compaction_interval(&self) -> Duration {
        Duration::from_secs(self.compaction_interval_seconds)
    }

    /// Ensures all configured directories exist.
    pub fn create_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.snapshot_dir).map_err(|e| {
            EcsDbError::ConfigError(format!("Failed to create snapshot dir: {}", e))
        })?;
        std::fs::create_dir_all(&self.wal_dir)
            .map_err(|e| EcsDbError::ConfigError(format!("Failed to create wal dir: {}", e)))?;
        std::fs::create_dir_all(&self.archive_dir)
            .map_err(|e| EcsDbError::ConfigError(format!("Failed to create archive dir: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = PersistenceConfig::default();
        assert_eq!(config.snapshot_dir, PathBuf::from("./snapshots"));
        assert!(config.compress_snapshots);
    }

    #[test]
    fn test_from_toml() {
        let toml = r#"
            snapshot_dir = "/custom/snapshots"
            wal_dir = "/custom/wal"
            archive_dir = "/custom/archive"
            max_wal_file_size = 10485760
            sync_on_write = false
            snapshot_interval_transactions = 500
            snapshot_interval_seconds = 1800
            compress_snapshots = false
            snapshot_compression_level = 1
            compress_archived_wal = true
            compaction_interval_seconds = 43200
            min_wal_files_for_compaction = 3
            keep_snapshots = 5
            keep_archived_wal_files = 2
        "#;
        let config = PersistenceConfig::from_toml(toml).unwrap();
        assert_eq!(config.snapshot_dir, PathBuf::from("/custom/snapshots"));
        assert_eq!(config.max_wal_file_size, 10_485_760);
        assert!(!config.sync_on_write);
        assert!(!config.compress_snapshots);
        assert_eq!(config.snapshot_compression_level, 1);
        assert!(config.compress_archived_wal);
        assert_eq!(config.compaction_interval_seconds, 43200);
        assert_eq!(config.min_wal_files_for_compaction, 3);
        assert_eq!(config.keep_snapshots, 5);
        assert_eq!(config.keep_archived_wal_files, 2);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.toml");
        let mut config = PersistenceConfig::default();
        config.snapshot_dir = PathBuf::from("/test/snap");
        config.save_to_file(&file_path).unwrap();
        let loaded = PersistenceConfig::from_file(&file_path).unwrap();
        assert_eq!(loaded.snapshot_dir, PathBuf::from("/test/snap"));
    }

    #[test]
    fn test_create_directories() {
        let dir = tempdir().unwrap();
        let mut config = PersistenceConfig::default();
        config.snapshot_dir = dir.path().join("snap");
        config.wal_dir = dir.path().join("wal");
        config.archive_dir = dir.path().join("wal/archive");
        config.create_directories().unwrap();
        assert!(config.snapshot_dir.exists());
        assert!(config.wal_dir.exists());
        assert!(config.archive_dir.exists());
    }
}
