# Phase 3: Persistence
**Estimated Time:** Weeks 9-12

## Overview
Add durability to the database through snapshots, write‑ahead log archival, async I/O, compaction, and crash recovery. Ensure data survives process restarts and system crashes.

## Dependencies
- Phase 2 completed (advanced storage with WAL)
- Tokio async runtime available

## Subtasks

### 3.1 Snapshot Creation & Restoration
- **Snapshot Format**: Binary format containing schema version, entity registry, all component tables
- **Snapshot Writer**: Serialize entire database state to a file (background task)
- **Snapshot Reader**: Load database from snapshot file (startup or restore)
- **Incremental Snapshots**: Optional diff‑based snapshots to reduce size

### 3.2 WAL Archival and Replay
- **WAL Rotation**: Close current WAL file after certain size/time, start new one
- **WAL Archive**: Move old WAL files to archive directory, compress optionally
- **WAL Replay**: On startup, replay WAL entries since last snapshot to bring database to latest state
- **WAL Truncation**: After successful snapshot, delete WAL files that are no longer needed

### 3.3 Async I/O Integration (Tokio)
- **AsyncFile**: Wrap `tokio::fs::File` for non‑blocking reads/writes
- **I/O Scheduler**: Queue disk operations and process them in background Tokio tasks
- **Write Batching**: Group multiple small writes into larger blocks for disk efficiency
- **Read‑Ahead**: Prefetch snapshot/WAL data during startup

### 3.4 Compaction Worker
- **Background Compactor**: Periodic task that merges snapshots and WAL files, reducing disk space
- **Compaction Strategy**: Based on WAL size, snapshot age, or manual trigger
- **Online Compaction**: Run while database is serving reads/writes (pause writes briefly)
- **Progress Reporting**: Log compaction progress and estimated time remaining

### 3.5 Crash Recovery
- **Recovery Manager**: On startup, detect incomplete transactions (missing commit record) and roll them back
- **Consistency Check**: Verify snapshot + WAL integrity via checksums
- **Automatic Repair**: Attempt to salvage data from corrupted files (optional)
- **Recovery Log**: Detailed log of recovery steps for debugging

### 3.6 Configuration System
- **PersistenceConfig**: Configurable paths for snapshots, WAL, archive; compaction intervals; compression settings
- **Environment Variables**: Override config via env vars (e.g., `ECDB_SNAPSHOT_DIR`)
- **Config File**: TOML‑based config file (e.g., `ecsdb.toml`)

### 3.7 Integration with Database API
- **Database::new_with_persistence()**: Constructor that loads from snapshot/WAL or creates new
- **Auto‑snapshot**: Option to automatically take snapshots after N transactions or time period
- **Manual Snapshot/Restore**: Expose Tauri commands for manual snapshot creation and restoration

### 3.8 End‑to‑End Durability Tests
- **Crash Simulation**: Kill process during writes, restart, verify data consistency
- **Power‑Loss Simulation**: Write partial transactions, ensure recovery rolls back
- **Long‑Running Tests**: Run database for hours with periodic snapshots, verify no data loss
- **Performance Under Persistence**: Measure overhead of WAL and snapshot writes

## Acceptance Criteria
1. Database can be restored to exact state after crash (snapshot + WAL replay)
2. Snapshot creation does not block read/write operations for more than a few milliseconds
3. WAL files are rotated and archived as configured; old files are cleaned up
4. Compaction reduces disk usage without data loss
5. Async I/O does not block the write thread; disk writes happen in background
6. Configuration options are respected and can be changed at runtime
7. Recovery from corrupted snapshot/WAL logs appropriate errors and can optionally repair
8. End‑to‑end tests pass (crash simulation, long‑running)

## Output Artifacts
- Snapshot binary format specification
- WAL archiving and replay implementation
- Compaction worker with scheduling
- Configuration TOML schema
- Crash recovery test suite
- Updated `AGENTS.md` with persistence‑related commands (snapshot, restore, compact)

## Notes
- Durability is critical; test extensively with fault injection
- Use `tokio::fs` for async file operations; avoid blocking the write thread
- Consider using `zstd` compression for snapshots and archived WAL files
- Document recovery procedure for production use
