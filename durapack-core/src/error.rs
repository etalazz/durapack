//! Error types for Durapack operations

use thiserror::Error;

/// Errors that can occur during Durapack frame operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FrameError {
    /// Invalid frame marker detected
    #[error("Invalid frame marker: expected DURP, got {0:?}")]
    BadMarker([u8; 4]),

    /// Unsupported protocol version
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u8),

    /// Frame size exceeds maximum allowed
    #[error("Frame size {0} exceeds maximum {1}")]
    FrameTooLarge(u32, u32),

    /// Payload size exceeds maximum allowed
    #[error("Payload size {0} exceeds maximum {1}")]
    PayloadTooLarge(u32, u32),

    /// Incomplete frame - not enough data
    #[error("Incomplete frame: expected {expected} bytes, got {actual}")]
    IncompleteFrame {
        /// The number of bytes expected.
        expected: usize,
        /// The number of bytes actually found.
        actual: usize,
    },

    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected:x}, got {actual:x}")]
    ChecksumMismatch {
        /// The expected checksum.
        expected: u32,
        /// The actual checksum calculated.
        actual: u32,
    },

    /// Hash mismatch
    #[error("Hash mismatch")]
    HashMismatch,

    /// IO error during read/write
    #[error("IO error: {0}")]
    Io(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid frame structure
    #[error("Invalid frame structure: {0}")]
    InvalidStructure(String),

    /// No frames found in stream
    #[error("No valid frames found in stream")]
    NoFramesFound,

    /// Gap detected in frame sequence
    #[error("Gap in sequence: frame {0} references prev_id {1} which is missing")]
    SequenceGap(u64, u64),

    /// Back-link hash mismatch
    #[error("Back-link hash mismatch for frame {0}")]
    BackLinkMismatch(u64),
}

impl From<std::io::Error> for FrameError {
    fn from(err: std::io::Error) -> Self {
        FrameError::Io(err.to_string())
    }
}
