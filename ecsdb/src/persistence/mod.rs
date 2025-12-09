//! Persistence layer for durable storage.
//!
//! Provides snapshot creation/restoration, WAL archiving, and crash recovery.

pub mod compaction;
pub mod file_wal;
pub mod manager;
pub mod snapshot;
pub mod wal;
