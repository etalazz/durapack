//! Core types for Durapack frames

use crate::constants::{FrameFlags, BLAKE3_HASH_SIZE, MAX_PAYLOAD_SIZE, PROTOCOL_VERSION};
use crate::error::FrameError;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Frame header containing metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameHeader {
    /// Protocol version
    pub version: u8,

    /// Unique frame identifier
    pub frame_id: u64,

    /// Hash of the previous frame (BLAKE3, 32 bytes)
    /// All zeros for the first frame
    pub prev_hash: [u8; BLAKE3_HASH_SIZE],

    /// Length of the payload in bytes
    pub payload_len: u32,

    /// Frame flags (trailer type, first/last markers, etc.)
    pub flags: FrameFlags,
}

impl FrameHeader {
    /// Create a new frame header
    pub fn new(frame_id: u64, prev_hash: [u8; BLAKE3_HASH_SIZE], payload_len: u32) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            frame_id,
            prev_hash,
            payload_len,
            flags: FrameFlags::default(),
        }
    }

    /// Create a new frame header with flags
    pub fn with_flags(
        frame_id: u64,
        prev_hash: [u8; BLAKE3_HASH_SIZE],
        payload_len: u32,
        flags: FrameFlags,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            frame_id,
            prev_hash,
            payload_len,
            flags,
        }
    }

    /// Validate the header
    pub fn validate(&self) -> Result<(), FrameError> {
        if self.version != PROTOCOL_VERSION {
            return Err(FrameError::UnsupportedVersion(self.version));
        }

        if self.payload_len > MAX_PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooLarge(
                self.payload_len,
                MAX_PAYLOAD_SIZE,
            ));
        }

        Ok(())
    }

    /// Check if this is the first frame in a sequence
    pub fn is_first(&self) -> bool {
        self.prev_hash == [0u8; BLAKE3_HASH_SIZE]
    }
}

/// Complete Durapack frame
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Frame header
    pub header: FrameHeader,

    /// Frame payload (application data)
    pub payload: Bytes,

    /// Optional trailer (checksum or hash)
    pub trailer: Option<Bytes>,

    /// Optional superframe index embedded in the payload of superframes
    pub super_index: Option<SuperframeIndex>,

    /// Optional skip-list backlinks present in payload
    pub skip_links: Option<alloc::vec::Vec<SkipLink>>,
}

/// Optional superframe index embedded in the payload of superframes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperframeIndex {
    /// Frame ID range summarized by this superframe (inclusive)
    pub range_start: u64,
    /// End of the summarized frame ID range (inclusive)
    pub range_end: u64,
    /// Recent frame IDs (last N) for quick local resync
    pub recent_ids: alloc::vec::Vec<u64>,
    /// Byte offsets of frames relative to the superframe position (best-effort)
    pub offsets: alloc::vec::Vec<u32>,
    /// Checksums of summarized frames for validation (e.g., CRC32C)
    pub checksums: alloc::vec::Vec<u32>,
}

/// Optional skip-list backlink entry enabling O(log n) seeks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipLink {
    /// Exponent distance (2^k frames back)
    pub level: u8,
    /// Target frame ID
    pub target_id: u64,
    /// Optional hint (e.g., relative offset)
    pub hint: Option<u32>,
}

impl Frame {
    /// Create a new frame
    pub fn new(header: FrameHeader, payload: Bytes) -> Self {
        Self {
            header,
            payload,
            trailer: None,
            super_index: None,
            skip_links: None,
        }
    }

    /// Create a new frame with trailer
    pub fn with_trailer(header: FrameHeader, payload: Bytes, trailer: Bytes) -> Self {
        Self {
            header,
            payload,
            trailer: Some(trailer),
            super_index: None,
            skip_links: None,
        }
    }

    /// Validate the frame
    pub fn validate(&self) -> Result<(), FrameError> {
        self.header.validate()?;

        if self.payload.len() as u32 != self.header.payload_len {
            return Err(FrameError::InvalidStructure(format!(
                "Payload length mismatch: header says {}, actual {}",
                self.header.payload_len,
                self.payload.len()
            )));
        }

        Ok(())
    }

    /// Calculate the total frame size in bytes
    pub fn total_size(&self) -> usize {
        crate::constants::MIN_HEADER_SIZE
            + self.payload.len()
            + self.trailer.as_ref().map_or(0, |t| t.len())
    }

    /// Get frame ID
    pub fn frame_id(&self) -> u64 {
        self.header.frame_id
    }

    /// Get previous hash
    pub fn prev_hash(&self) -> &[u8; BLAKE3_HASH_SIZE] {
        &self.header.prev_hash
    }

    /// Compute BLAKE3 hash of this frame's header + payload
    pub fn compute_hash(&self) -> [u8; BLAKE3_HASH_SIZE] {
        let mut hasher = blake3::Hasher::new();

        // Hash the header fields
        hasher.update(&[self.header.version]);
        hasher.update(&self.header.frame_id.to_be_bytes());
        hasher.update(&self.header.prev_hash);
        hasher.update(&self.header.payload_len.to_be_bytes());
        hasher.update(&[self.header.flags.as_u8()]);

        // Hash the payload
        hasher.update(&self.payload);

        let hash = hasher.finalize();
        let mut result = [0u8; BLAKE3_HASH_SIZE];
        result.copy_from_slice(hash.as_bytes());
        result
    }
}

/// Trait for types that can be serialized into Durapack frames
pub trait DurapackSerializable {
    /// Serialize this type into bytes for frame payload
    fn to_payload(&self) -> Result<Bytes, FrameError>;

    /// Deserialize from frame payload bytes
    fn from_payload(bytes: &[u8]) -> Result<Self, FrameError>
    where
        Self: Sized;
}

// Implement for common types
impl DurapackSerializable for Vec<u8> {
    fn to_payload(&self) -> Result<Bytes, FrameError> {
        Ok(Bytes::copy_from_slice(self))
    }

    fn from_payload(bytes: &[u8]) -> Result<Self, FrameError> {
        Ok(bytes.to_vec())
    }
}

impl DurapackSerializable for Bytes {
    fn to_payload(&self) -> Result<Bytes, FrameError> {
        Ok(self.clone())
    }

    fn from_payload(bytes: &[u8]) -> Result<Self, FrameError> {
        Ok(Bytes::copy_from_slice(bytes))
    }
}

impl DurapackSerializable for String {
    fn to_payload(&self) -> Result<Bytes, FrameError> {
        Ok(Bytes::copy_from_slice(self.as_bytes()))
    }

    fn from_payload(bytes: &[u8]) -> Result<Self, FrameError> {
        String::from_utf8(bytes.to_vec()).map_err(|e| FrameError::Serialization(e.to_string()))
    }
}
