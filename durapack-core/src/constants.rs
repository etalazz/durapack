//! Constants and limits for the Durapack frame format

use serde::{Deserialize, Serialize};

/// Frame marker - 4 bytes for synchronization
pub const FRAME_MARKER: &[u8; 4] = b"DURP";

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
}

impl TrailerType {
    /// Returns the size of the trailer in bytes
    pub const fn size(&self) -> usize {
        match self {
            TrailerType::None => 0,
            TrailerType::Crc32c => CRC32C_SIZE,
            TrailerType::Blake3 => BLAKE3_HASH_SIZE,
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

    /// Get the trailer type
    pub const fn trailer_type(&self) -> TrailerType {
        if self.has_blake3() {
            TrailerType::Blake3
        } else if self.has_crc32c() {
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

