//! Error types for Durapack operations

use alloc::string::String;

/// Errors that can occur during Durapack frame operations
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[derive(Debug, Clone, PartialEq)]
pub enum FrameError {
    /// Invalid frame marker detected
    #[cfg_attr(feature = "std", error("Invalid frame marker: expected DURP, got {0:?}"))]
    BadMarker([u8; 4]),

    /// Unsupported protocol version
    #[cfg_attr(feature = "std", error("Unsupported protocol version: {0}"))]
    UnsupportedVersion(u8),

    /// Frame size exceeds maximum allowed
    #[cfg_attr(feature = "std", error("Frame size {0} exceeds maximum {1}"))]
    FrameTooLarge(u32, u32),

    /// Payload size exceeds maximum allowed
    #[cfg_attr(feature = "std", error("Payload size {0} exceeds maximum {1}"))]
    PayloadTooLarge(u32, u32),

    /// Incomplete frame - not enough data
    #[cfg_attr(feature = "std", error("Incomplete frame: expected {expected} bytes, got {actual}"))]
    IncompleteFrame {
        /// The number of bytes expected.
        expected: usize,
        /// The number of bytes actually found.
        actual: usize,
    },

    /// Checksum mismatch
    #[cfg_attr(feature = "std", error("Checksum mismatch: expected {expected:x}, got {actual:x}"))]
    ChecksumMismatch {
        /// The expected checksum.
        expected: u32,
        /// The actual checksum calculated.
        actual: u32,
    },

    /// Hash mismatch
    #[cfg_attr(feature = "std", error("Hash mismatch"))]
    HashMismatch,

    /// IO error during read/write
    #[cfg_attr(feature = "std", error("IO error: {0}"))]
    Io(String),

    /// Serialization error
    #[cfg_attr(feature = "std", error("Serialization error: {0}"))]
    Serialization(String),

    /// Invalid frame structure
    #[cfg_attr(feature = "std", error("Invalid frame structure: {0}"))]
    InvalidStructure(String),

    /// No frames found in stream
    #[cfg_attr(feature = "std", error("No valid frames found in stream"))]
    NoFramesFound,

    /// Gap detected in frame sequence
    #[cfg_attr(feature = "std", error("Gap in sequence: frame {0} references prev_id {1} which is missing"))]
    SequenceGap(u64, u64),

    /// Back-link hash mismatch
    #[cfg_attr(feature = "std", error("Back-link hash mismatch for frame {0}"))]
    BackLinkMismatch(u64),
}

#[cfg(feature = "std")]
impl From<std::io::Error> for FrameError {
    fn from(err: std::io::Error) -> Self {
        FrameError::Io(err.to_string())
    }
}
