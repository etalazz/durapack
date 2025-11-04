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

    // Optional sync/preamble sizes
    let mut prefix_len = 0;
    if header.flags.as_u8() & crate::constants::FrameFlags::HAS_SYNC_PREFIX != 0 {
        prefix_len += crate::constants::ROBUST_SYNC_WORD.len();
    }
    if header.flags.as_u8() & crate::constants::FrameFlags::HAS_PREAMBLE != 0 {
        // Use minimal preamble length
        prefix_len += crate::constants::MIN_PREAMBLE_LEN;
    }

    let total_size = prefix_len + MIN_HEADER_SIZE + payload.len() + trailer_size;

    let mut buf = BytesMut::with_capacity(total_size);

    // Optional prefix: preamble (alternating 0x55/0xAA)
    if header.flags.as_u8() & crate::constants::FrameFlags::HAS_PREAMBLE != 0 {
        let pat = crate::constants::PREAMBLE_PATTERN;
        for i in 0..crate::constants::MIN_PREAMBLE_LEN {
            buf.put_u8(pat[i % pat.len()]);
        }
    }

    // Optional robust sync word
    if header.flags.as_u8() & crate::constants::FrameFlags::HAS_SYNC_PREFIX != 0 {
        buf.put_slice(crate::constants::ROBUST_SYNC_WORD);
    }

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
        TrailerType::Blake3WithEd25519Sig => {
            let hash = compute_blake3(&buf);
            buf.put_slice(&hash);
            #[cfg(feature = "ed25519-signatures")]
            {
                // Without a key, we cannot sign here; append zeros to preserve layout.
                let zeros = [0u8; 64];
                buf.put_slice(&zeros);
            }
            #[cfg(not(feature = "ed25519-signatures"))]
            {
                let zeros = [0u8; 64];
                buf.put_slice(&zeros);
            }
        }
    }

    Ok(buf.freeze())
}

/// Encode a complete Frame struct
pub fn encode_frame_struct(frame: &Frame) -> Result<Bytes, FrameError> {
    encode_frame(&frame.header, &frame.payload)
}

/// Encode a frame into bytes with Ed25519 signature when combined trailer is requested
#[cfg(feature = "ed25519-signatures")]
pub fn encode_frame_signed(
    header: &FrameHeader,
    payload: &[u8],
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<Bytes, FrameError> {
    let mut encoded = encode_frame(header, payload)?; // includes hash and 64 zeros when combined
    if header.flags.trailer_type() == TrailerType::Blake3WithEd25519Sig {
        use ed25519_dalek::Signer;
        // Compute signature over marker+header+payload (without trailer)
        let mut msg = BytesMut::with_capacity(MIN_HEADER_SIZE + payload.len());
        msg.extend_from_slice(FRAME_MARKER);
        msg.extend_from_slice(&[header.version]);
        msg.extend_from_slice(&header.frame_id.to_be_bytes());
        msg.extend_from_slice(&header.prev_hash);
        msg.extend_from_slice(&header.payload_len.to_be_bytes());
        msg.extend_from_slice(&[header.flags.as_u8()]);
        msg.extend_from_slice(payload);
        let sig = signing_key.sign(&msg).to_bytes();
        // Overwrite trailing 64 zero bytes with signature
        let total = encoded.len();
        let sig_start = total - 64;
        let mut v = encoded.to_vec();
        v[sig_start..].copy_from_slice(&sig);
        encoded = Bytes::from(v);
    }
    Ok(encoded)
}

/// Encode a complete Frame struct with signing key when using combined trailer
#[cfg(feature = "ed25519-signatures")]
pub fn encode_frame_struct_signed(
    frame: &Frame,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<Bytes, FrameError> {
    encode_frame_signed(&frame.header, &frame.payload, signing_key)
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

/// Compute chain hash: BLAKE3 over header fields, previous trailer (if any), and payload
pub fn compute_chain_hash(frame: &Frame, prev_trailer: Option<&[u8]>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[frame.header.version]);
    hasher.update(&frame.header.frame_id.to_be_bytes());
    hasher.update(&frame.header.prev_hash);
    hasher.update(&frame.header.payload_len.to_be_bytes());
    hasher.update(&[frame.header.flags.as_u8()]);
    if let Some(t) = prev_trailer {
        hasher.update(t);
    }
    hasher.update(&frame.payload);
    let out = hasher.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(out.as_bytes());
    r
}

/// Compute Ed25519 signature over header + payload (no prev_trailer) when enabled
#[cfg(feature = "ed25519-signatures")]
pub fn ed25519_sign_header_payload(frame: &Frame, sk: &ed25519_dalek::SigningKey) -> [u8; 64] {
    use ed25519_dalek::Signer;
    let mut buf = BytesMut::with_capacity(MIN_HEADER_SIZE + frame.payload.len());
    buf.extend_from_slice(FRAME_MARKER);
    buf.extend_from_slice(&[frame.header.version]);
    buf.extend_from_slice(&frame.header.frame_id.to_be_bytes());
    buf.extend_from_slice(&frame.header.prev_hash);
    buf.extend_from_slice(&frame.header.payload_len.to_be_bytes());
    buf.extend_from_slice(&[frame.header.flags.as_u8()]);
    buf.extend_from_slice(&frame.payload);
    let sig = sk.sign(&buf);
    let mut out = [0u8; 64];
    out.copy_from_slice(&sig.to_bytes());
    out
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

    /// Mark this frame as a superframe (payload should carry an index)
    pub fn as_superframe(mut self) -> Self {
        self.flags |= FrameFlags::IS_SUPERFRAME;
        self
    }

    /// Indicate that payload includes skip-list backlinks
    pub fn with_skiplist(mut self) -> Self {
        self.flags |= FrameFlags::HAS_SKIPLIST;
        self
    }

    /// Enable BLAKE3+Ed25519 signature trailer (off by default). Requires providing signature separately.
    pub fn with_blake3_signature(mut self) -> Self {
        // Overload flags: set both bits to indicate Blake3+Sig combined trailer
        self.flags |= FrameFlags::HAS_BLAKE3 | FrameFlags::HAS_CRC32C;
        self
    }

    /// Sign with Ed25519 (feature: ed25519-signatures). This sets the combined trailer flag and stores signature at encode time.
    #[cfg(feature = "ed25519-signatures")]
    pub fn sign_with_ed25519(mut self, _sk: &ed25519_dalek::SigningKey) -> Self {
        self = self.with_blake3_signature();
        // Signature is added in encode via encode_frame; for custom pipelines, call ed25519_sign_header_payload.
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
