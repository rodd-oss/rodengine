//! Delta serialization format for network transmission.
//!
//! Defines a binary frame format with header, payload, and checksum.
//! Supports optional zstd compression.

use crate::error::{EcsDbError, Result};
use crate::storage::delta::{Delta, DeltaOp};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc32fast::Hasher;

/// Frame header magic number: "ECSD" (0x45 0x43 0x53 0x44).
const MAGIC: [u8; 4] = [0x45, 0x43, 0x53, 0x44];
/// Current protocol version.
const PROTOCOL_VERSION: u8 = 1;

/// Frame flags.
#[derive(Debug, Clone, Copy)]
pub enum FrameFlag {
    /// Payload is compressed with zstd.
    Compressed = 0x01,
    /// Frame is a heartbeat ping.
    Heartbeat = 0x02,
    /// Frame is a full snapshot.
    Snapshot = 0x04,
    /// Frame is a delta batch.
    Delta = 0x08,
}

impl FrameFlag {
    fn to_bits(self) -> u8 {
        self as u8
    }

    fn from_bits(bits: u8) -> Vec<Self> {
        let mut flags = Vec::new();
        if bits & Self::Compressed.to_bits() != 0 {
            flags.push(Self::Compressed);
        }
        if bits & Self::Heartbeat.to_bits() != 0 {
            flags.push(Self::Heartbeat);
        }
        if bits & Self::Snapshot.to_bits() != 0 {
            flags.push(Self::Snapshot);
        }
        if bits & Self::Delta.to_bits() != 0 {
            flags.push(Self::Delta);
        }
        flags
    }
}

/// Binary frame for network transmission.
#[derive(Debug)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub payload: Bytes,
}

impl Frame {
    /// Creates a new frame with the given flags and payload.
    pub fn new(flags: u8, payload: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            flags,
            payload,
        }
    }

    /// Encodes the frame into bytes (including header and checksum).
    pub fn encode(&self) -> Bytes {
        let payload_len = self.payload.len() as u32;
        let mut buf = BytesMut::with_capacity(4 + 1 + 1 + 4 + payload_len as usize + 4);
        buf.put_slice(&MAGIC);
        buf.put_u8(self.version);
        buf.put_u8(self.flags);
        buf.put_u32(payload_len);
        buf.put_slice(&self.payload);
        // Compute CRC32 checksum over everything except the checksum field itself.
        let mut hasher = Hasher::new();
        hasher.update(&buf);
        let checksum = hasher.finalize();
        buf.put_u32(checksum);
        buf.freeze()
    }

    /// Decodes a frame from bytes, validating magic and checksum.
    pub fn decode(mut bytes: Bytes) -> Result<Self> {
        if bytes.len() < 4 + 1 + 1 + 4 + 4 {
            return Err(EcsDbError::ReplicationError("Frame too short".to_string()));
        }
        let magic = &bytes[0..4];
        if magic != MAGIC {
            return Err(EcsDbError::ReplicationError("Invalid magic".to_string()));
        }
        bytes.advance(4);
        let version = bytes.get_u8();
        if version != PROTOCOL_VERSION {
            return Err(EcsDbError::ReplicationError(format!(
                "Unsupported protocol version {}",
                version
            )));
        }
        let flags = bytes.get_u8();
        let payload_len = bytes.get_u32() as usize;
        let total_len_expected = 4 + 1 + 1 + 4 + payload_len + 4;
        if bytes.len() < payload_len + 4 {
            return Err(EcsDbError::ReplicationError("Incomplete frame".to_string()));
        }
        let payload = bytes.copy_to_bytes(payload_len);
        let checksum_received = bytes.get_u32();

        // Verify checksum (over everything except the last 4 bytes)
        let mut hasher = Hasher::new();
        hasher.update(&MAGIC);
        hasher.update(&[version, flags]);
        hasher.update(&(payload_len as u32).to_be_bytes());
        hasher.update(&payload);
        let checksum_computed = hasher.finalize();
        if checksum_received != checksum_computed {
            return Err(EcsDbError::ReplicationError("Checksum mismatch".to_string()));
        }

        Ok(Self {
            version,
            flags,
            payload,
        })
    }

    /// Returns whether the frame is compressed.
    pub fn is_compressed(&self) -> bool {
        self.flags & FrameFlag::Compressed.to_bits() != 0
    }

    /// Compresses the payload with zstd (if not already compressed).
    pub fn compress(&mut self, level: i32) -> Result<()> {
        if self.is_compressed() {
            return Ok(());
        }
        let compressed = zstd::encode_all(&self.payload[..], level)
            .map_err(|e| EcsDbError::CompressionError(e.to_string()))?;
        self.payload = Bytes::from(compressed);
        self.flags |= FrameFlag::Compressed.to_bits();
        Ok(())
    }

    /// Decompresses the payload (if compressed).
    pub fn decompress(&mut self) -> Result<()> {
        if !self.is_compressed() {
            return Ok(());
        }
        let decompressed = zstd::decode_all(&self.payload[..])
            .map_err(|e| EcsDbError::CompressionError(e.to_string()))?;
        self.payload = Bytes::from(decompressed);
        self.flags &= !FrameFlag::Compressed.to_bits();
        Ok(())
    }
}

/// Encoder for delta batches.
pub struct DeltaEncoder;

impl DeltaEncoder {
    /// Encodes a delta into a network frame, optionally compressed.
    pub fn encode(delta: &Delta, compress: bool) -> Result<Frame> {
        let payload = delta.serialize()?;
        let mut flags = FrameFlag::Delta.to_bits();
        let mut payload_bytes = Bytes::from(payload);
        let mut frame = Frame::new(flags, payload_bytes);
        if compress {
            frame.compress(3)?; // default compression level 3
        }
        Ok(frame)
    }

    /// Decodes a frame into a delta, handling decompression if needed.
    pub fn decode(frame: Frame) -> Result<Delta> {
        let mut frame = frame;
        frame.decompress()?;
        Delta::deserialize(&frame.payload)
    }
}

/// Decoder for delta batches (convenience alias).
pub type DeltaDecoder = DeltaEncoder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::delta::{Delta, DeltaOp};

    #[test]
    fn test_frame_encode_decode() -> Result<()> {
        let payload = Bytes::from(vec![1, 2, 3, 4, 5]);
        let frame = Frame::new(FrameFlag::Delta.to_bits(), payload.clone());
        let encoded = frame.encode();
        let decoded = Frame::decode(encoded)?;
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.flags, FrameFlag::Delta.to_bits());
        assert_eq!(decoded.payload, payload);
        Ok(())
    }

    #[test]
    fn test_frame_compression() -> Result<()> {
        let payload = Bytes::from(vec![1, 2, 3, 4, 5]);
        let mut frame = Frame::new(FrameFlag::Delta.to_bits(), payload);
        frame.compress(3)?;
        assert!(frame.is_compressed());
        frame.decompress()?;
        assert!(!frame.is_compressed());
        Ok(())
    }

    #[test]
    fn test_delta_encoder() -> Result<()> {
        let delta = Delta {
            ops: vec![DeltaOp::Insert {
                table_id: 1,
                entity_id: 100,
                data: vec![1, 2, 3],
            }],
            version: 5,
            timestamp: 12345,
        };
        let frame = DeltaEncoder::encode(&delta, false)?;
        let decoded = DeltaEncoder::decode(frame)?;
        assert_eq!(decoded.version, delta.version);
        assert_eq!(decoded.timestamp, delta.timestamp);
        assert_eq!(decoded.ops.len(), 1);
        Ok(())
    }
}
