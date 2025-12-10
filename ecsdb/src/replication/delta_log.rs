//! Delta logging for monitoring and dashboard display.

use crate::storage::delta::{Delta, DeltaOp};
use serde::{Deserialize, Serialize};

/// A logged delta entry for dashboard display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaLogEntry {
    /// Sequence number (monotonically increasing).
    pub seq: u64,
    /// Delta version (database version after applying this delta).
    pub version: u64,
    /// Timestamp when the delta was enqueued (milliseconds since epoch).
    pub timestamp: u64,
    /// Number of operations in the delta.
    pub operation_count: usize,
    /// Type of the first operation (simplified).
    pub first_op_type: String,
    /// Table ID of the first operation, if any.
    pub first_table_id: Option<u16>,
    /// Entity ID of the first operation, if any.
    pub first_entity_id: Option<u64>,
}

impl DeltaLogEntry {
    pub fn from_delta(seq: u64, delta: &Delta) -> Self {
        let first_op = delta.ops.first();
        let (first_op_type, first_table_id, first_entity_id) = if let Some(op) = first_op {
            let (op_type, table_id, entity_id) = match op {
                DeltaOp::CreateEntity { entity_id } => ("create_entity", None, Some(*entity_id)),
                DeltaOp::DeleteEntity { entity_id } => ("delete_entity", None, Some(*entity_id)),
                DeltaOp::Insert {
                    table_id,
                    entity_id,
                    ..
                } => ("insert", Some(*table_id), Some(*entity_id)),
                DeltaOp::Update {
                    table_id,
                    entity_id,
                    ..
                } => ("update", Some(*table_id), Some(*entity_id)),
                DeltaOp::Delete {
                    table_id,
                    entity_id,
                    ..
                } => ("delete", Some(*table_id), Some(*entity_id)),
            };
            (op_type.to_string(), table_id, entity_id)
        } else {
            ("empty".to_string(), None, None)
        };

        Self {
            seq,
            version: delta.version,
            timestamp: delta.timestamp,
            operation_count: delta.ops.len(),
            first_op_type,
            first_table_id,
            first_entity_id,
        }
    }
}

/// Log of recent deltas for dashboard monitoring.
pub struct DeltaLog {
    entries: Vec<DeltaLogEntry>,
    max_entries: usize,
    next_seq: u64,
}

impl DeltaLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            next_seq: 0,
        }
    }

    pub fn record(&mut self, delta: &Delta) {
        let entry = DeltaLogEntry::from_delta(self.next_seq, delta);
        self.next_seq += 1;
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[DeltaLogEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::delta::{Delta, DeltaOp};

    #[test]
    fn test_delta_log() {
        let mut log = DeltaLog::new(3);
        let delta = Delta {
            ops: vec![DeltaOp::Insert {
                table_id: 1,
                entity_id: 100,
                data: vec![1, 2, 3],
            }],
            version: 5,
            timestamp: 1234567890,
        };
        log.record(&delta);
        assert_eq!(log.entries().len(), 1);
        let entry = &log.entries()[0];
        assert_eq!(entry.version, 5);
        assert_eq!(entry.first_op_type, "insert");
        assert_eq!(entry.first_table_id, Some(1));
        assert_eq!(entry.first_entity_id, Some(100));
    }
}