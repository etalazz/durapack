//! Frame encoding

use crate::constants::{FrameFlags, TrailerType, FRAME_MARKER, MIN_HEADER_SIZE};
use crate::error::FrameError;
use crate::types::{Frame, FrameHeader};
use alloc::format;
use bytes::{BufMut, Bytes, BytesMut};

/// Encode a frame into bytes
///
/// The frame is encoded with the following layout:
/// 1. Marker (4 bytes): "DURP"
/// 2. Header:
///    - Version (1 byte)
///    - Frame ID (8 bytes, big-endian)
///    - Previous hash (32 bytes)
///    - Payload length (4 bytes, big-endian)
///    - Flags (1 byte)
/// 3. Payload (variable length)
/// 4. Trailer (optional, CRC32C or BLAKE3)
pub fn encode_frame(header: &FrameHeader, payload: &[u8]) -> Result<Bytes, FrameError> {
    header.validate()?;

    if payload.len() as u32 != header.payload_len {
        return Err(FrameError::InvalidStructure(format!(
            "Payload length mismatch: header says {}, actual {}",
            header.payload_len,
            payload.len()
        )));
    }

    let trailer_type = header.flags.trailer_type();
    let trailer_size = trailer_type.size();
    let total_size = MIN_HEADER_SIZE + payload.len() + trailer_size;

    let mut buf = BytesMut::with_capacity(total_size);

    // Write marker
    buf.put_slice(FRAME_MARKER);

    // Write header
    buf.put_u8(header.version);
    buf.put_u64(header.frame_id);
    buf.put_slice(&header.prev_hash);
    buf.put_u32(header.payload_len);
    buf.put_u8(header.flags.as_u8());

    // Write payload
    buf.put_slice(payload);

    // Write trailer if needed
    match trailer_type {
        TrailerType::None => {}
        TrailerType::Crc32c => {
            let checksum = compute_crc32c(&buf);
            buf.put_u32(checksum);
        }
        TrailerType::Blake3 => {
            let hash = compute_blake3(&buf);
            buf.put_slice(&hash);
        }
    }

    Ok(buf.freeze())
}

/// Encode a complete Frame struct
pub fn encode_frame_struct(frame: &Frame) -> Result<Bytes, FrameError> {
    encode_frame(&frame.header, &frame.payload)
}

/// Compute CRC32C checksum of data
fn compute_crc32c(data: &[u8]) -> u32 {
    crc32c::crc32c(data)
}

/// Compute BLAKE3 hash of data
fn compute_blake3(data: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(data);
    let mut result = [0u8; 32];
    result.copy_from_slice(hash.as_bytes());
    result
}

/// Builder for constructing frames with various options
pub struct FrameBuilder {
    frame_id: u64,
    prev_hash: [u8; 32],
    payload: Bytes,
    flags: u8,
}

impl FrameBuilder {
    /// Create a new frame builder
    pub fn new(frame_id: u64) -> Self {
        Self {
            frame_id,
            prev_hash: [0u8; 32],
            payload: Bytes::new(),
            flags: FrameFlags::NONE,
        }
    }

    /// Set the previous hash
    pub fn prev_hash(mut self, hash: [u8; 32]) -> Self {
        self.prev_hash = hash;
        self
    }

    /// Set the payload
    pub fn payload(mut self, payload: Bytes) -> Self {
        self.payload = payload;
        self
    }

    /// Enable CRC32C trailer
    pub fn with_crc32c(mut self) -> Self {
        self.flags |= FrameFlags::HAS_CRC32C;
        self
    }

    /// Enable BLAKE3 trailer
    pub fn with_blake3(mut self) -> Self {
        self.flags |= FrameFlags::HAS_BLAKE3;
        self
    }

    /// Mark as first frame
    pub fn mark_first(mut self) -> Self {
        self.flags |= FrameFlags::IS_FIRST;
        self.prev_hash = [0u8; 32];
        self
    }

    /// Mark as last frame
    pub fn mark_last(mut self) -> Self {
        self.flags |= FrameFlags::IS_LAST;
        self
    }

    /// Build and encode the frame
    pub fn build(self) -> Result<Bytes, FrameError> {
        let header = FrameHeader::with_flags(
            self.frame_id,
            self.prev_hash,
            self.payload.len() as u32,
            FrameFlags::new(self.flags),
        );

        encode_frame(&header, &self.payload)
    }

    /// Build the frame struct without encoding
    pub fn build_struct(self) -> Result<Frame, FrameError> {
        let header = FrameHeader::with_flags(
            self.frame_id,
            self.prev_hash,
            self.payload.len() as u32,
            FrameFlags::new(self.flags),
        );

        header.validate()?;

        Ok(Frame::new(header, self.payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_simple_frame() {
        let payload = b"Hello, Durapack!";
        let header = FrameHeader::new(1, [0u8; 32], payload.len() as u32);

        let encoded = encode_frame(&header, payload).unwrap();

        // Check marker
        assert_eq!(&encoded[0..4], b"DURP");

        // Check version
        assert_eq!(encoded[4], 1);

        // Check frame ID (big-endian)
        assert_eq!(&encoded[5..13], &1u64.to_be_bytes());
    }

    #[test]
    fn test_frame_builder() {
        let payload = Bytes::from("test payload");
        let encoded = FrameBuilder::new(42)
            .payload(payload)
            .with_crc32c()
            .mark_first()
            .build()
            .unwrap();

        assert!(encoded.len() > MIN_HEADER_SIZE + 12);
        assert_eq!(&encoded[0..4], b"DURP");
    }

    #[test]
    fn test_encode_with_blake3() {
        let payload = b"test";
        let header = FrameHeader::with_flags(
            1,
            [0u8; 32],
            payload.len() as u32,
            FrameFlags::new(FrameFlags::HAS_BLAKE3),
        );

        let encoded = encode_frame(&header, payload).unwrap();

        // Should include 32-byte BLAKE3 hash at the end
        assert_eq!(encoded.len(), MIN_HEADER_SIZE + 4 + 32);
    }
}
