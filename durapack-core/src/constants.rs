//! Constants and limits for the Durapack frame format

use serde::{Deserialize, Serialize};

/// Frame marker - 4 bytes for synchronization
pub const FRAME_MARKER: &[u8; 4] = b"DURP";

/// Optional robust sync word placed before the marker to aid resync
/// Low autocorrelation pattern (example m-sequence-like bytes)
pub const ROBUST_SYNC_WORD: &[u8; 8] = b"\xA5\x5A\xC3\x3C\x96\x69\x78\x87";

/// Optional preamble pattern for burst-error resync: alternating 0x55, 0xAA
pub const PREAMBLE_PATTERN: &[u8; 2] = b"\x55\xAA";

/// Minimum preamble length (in bytes) considered meaningful by the scanner when present
pub const MIN_PREAMBLE_LEN: usize = 8;

/// Max Hamming distance (in bits) tolerated when matching the 4-byte marker during scanning
/// 0 = only exact matches. Small values (e.g., 1) can help recover through single-bit flips
/// while keeping false positives low.
pub const MAX_MARKER_HAMMING: u32 = 1;

/// Current protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum frame size (16 MB)
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Maximum payload size (slightly less than MAX_FRAME_SIZE to account for header/trailer)
pub const MAX_PAYLOAD_SIZE: u32 = MAX_FRAME_SIZE - 1024;

/// Size of BLAKE3 hash in bytes
pub const BLAKE3_HASH_SIZE: usize = 32;

/// Size of CRC32C checksum in bytes
pub const CRC32C_SIZE: usize = 4;

/// Header size without variable fields (marker + version + frame_id + prev_hash + payload_len + flags)
/// 4 (marker) + 1 (version) + 8 (frame_id) + 32 (prev_hash) + 4 (payload_len) + 1 (flags) = 50 bytes
pub const MIN_HEADER_SIZE: usize = 50;

/// Trailer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerType {
    /// No trailer
    None,
    /// CRC32C checksum (4 bytes)
    Crc32c,
    /// BLAKE3 hash (32 bytes)
    Blake3,
    /// BLAKE3 hash (32 bytes) followed by Ed25519 signature (64 bytes)
    Blake3WithEd25519Sig,
}

impl TrailerType {
    /// Returns the size of the trailer in bytes
    pub const fn size(&self) -> usize {
        match self {
            TrailerType::None => 0,
            TrailerType::Crc32c => CRC32C_SIZE,
            TrailerType::Blake3 => BLAKE3_HASH_SIZE,
            TrailerType::Blake3WithEd25519Sig => BLAKE3_HASH_SIZE + 64,
        }
    }
}

/// Flags for frame options (stored as a single byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameFlags(u8);

impl FrameFlags {
    /// No flags set
    pub const NONE: u8 = 0b0000_0000;

    /// Frame has CRC32C trailer
    pub const HAS_CRC32C: u8 = 0b0000_0001;

    /// Frame has BLAKE3 trailer
    pub const HAS_BLAKE3: u8 = 0b0000_0010;

    /// Frame is the first in a sequence
    pub const IS_FIRST: u8 = 0b0000_0100;

    /// Frame is the last in a sequence
    pub const IS_LAST: u8 = 0b0000_1000;

    /// Frame encoded with a preamble before the marker
    pub const HAS_PREAMBLE: u8 = 0b0001_0000;

    /// Frame encoded with a robust sync word before the marker
    pub const HAS_SYNC_PREFIX: u8 = 0b0010_0000;

    /// Frame encoded as a superframe (contains an index in payload)
    pub const IS_SUPERFRAME: u8 = 0b0100_0000;

    /// Frame payload carries optional logarithmic skip-list backlinks
    pub const HAS_SKIPLIST: u8 = 0b1000_0000;

    /// Create new flags from raw byte
    pub const fn new(flags: u8) -> Self {
        Self(flags)
    }

    /// Get raw flags byte
    pub const fn as_u8(&self) -> u8 {
        self.0
    }

    /// Check if CRC32C trailer is present
    pub const fn has_crc32c(&self) -> bool {
        (self.0 & Self::HAS_CRC32C) != 0
    }

    /// Check if BLAKE3 trailer is present
    pub const fn has_blake3(&self) -> bool {
        (self.0 & Self::HAS_BLAKE3) != 0
    }

    /// Check if this is the first frame
    pub const fn is_first(&self) -> bool {
        (self.0 & Self::IS_FIRST) != 0
    }

    /// Check if this is the last frame
    pub const fn is_last(&self) -> bool {
        (self.0 & Self::IS_LAST) != 0
    }

    /// Check if a preamble prefix is present
    pub const fn has_preamble(&self) -> bool {
        (self.0 & Self::HAS_PREAMBLE) != 0
    }

    /// Check if a robust sync prefix is present
    pub const fn has_sync_prefix(&self) -> bool {
        (self.0 & Self::HAS_SYNC_PREFIX) != 0
    }

    /// Check if this frame is a superframe
    pub const fn is_superframe(&self) -> bool {
        (self.0 & Self::IS_SUPERFRAME) != 0
    }

    /// Check if skip-list backlinks are carried in the payload
    pub const fn has_skiplist(&self) -> bool {
        (self.0 & Self::HAS_SKIPLIST) != 0
    }

    /// Get the trailer type
    pub const fn trailer_type(&self) -> TrailerType {
        let has_b3 = self.has_blake3();
        let has_crc = self.has_crc32c();
        if has_b3 && has_crc {
            // Combined semantics: BLAKE3 + Ed25519 signature
            TrailerType::Blake3WithEd25519Sig
        } else if has_b3 {
            TrailerType::Blake3
        } else if has_crc {
            TrailerType::Crc32c
        } else {
            TrailerType::None
        }
    }
}

impl Default for FrameFlags {
    fn default() -> Self {
        Self(Self::NONE)
    }
}
