//! Forward Error Correction traits (interface only)
//!
//! This module defines traits for FEC encoding and decoding.
//! Concrete implementations (Reed-Solomon, Raptor, etc.) are left for future work.

use crate::error::FrameError;
use crate::types::Frame;

/// A Forward Error Correction block
#[derive(Debug, Clone, PartialEq)]
pub struct FecBlock {
    /// Block identifier
    pub block_id: u64,

    /// Index of this block in the batch
    pub index: u32,

    /// Total blocks in the batch
    pub total_blocks: u32,

    /// Encoded data
    pub data: Vec<u8>,
}

/// Trait for encoding frames into FEC blocks
pub trait RedundancyEncoder {
    /// Encode a batch of frames into FEC blocks
    ///
    /// # Arguments
    /// * `frames` - The frames to encode
    /// * `redundancy` - Number of additional redundant blocks to generate
    ///
    /// # Returns
    /// A vector of FEC blocks. The first N blocks are the original frames,
    /// followed by K redundant blocks (where K = redundancy).
    fn encode_batch(
        &self,
        frames: &[Frame],
        redundancy: usize,
    ) -> Result<Vec<FecBlock>, FrameError>;
}

/// Trait for decoding frames from FEC blocks
pub trait RedundancyDecoder {
    /// Reconstruct original frames from a subset of FEC blocks
    ///
    /// # Arguments
    /// * `blocks` - Available FEC blocks (may be incomplete)
    /// * `total_frames` - Expected number of original frames
    ///
    /// # Returns
    /// Reconstructed frames, or an error if insufficient blocks are available
    fn decode_batch(
        &self,
        blocks: &[FecBlock],
        total_frames: usize,
    ) -> Result<Vec<Frame>, FrameError>;

    /// Check if we have enough blocks to reconstruct the original data
    fn can_reconstruct(&self, available_blocks: usize, total_frames: usize) -> bool;
}

/// Placeholder implementation for future use
#[derive(Debug, Clone)]
pub struct NoopEncoder;

impl RedundancyEncoder for NoopEncoder {
    fn encode_batch(
        &self,
        _frames: &[Frame],
        _redundancy: usize,
    ) -> Result<Vec<FecBlock>, FrameError> {
        Err(FrameError::InvalidStructure(
            "FEC encoding not implemented".to_string(),
        ))
    }
}

/// Placeholder implementation for future use
#[derive(Debug, Clone)]
pub struct NoopDecoder;

impl RedundancyDecoder for NoopDecoder {
    fn decode_batch(
        &self,
        _blocks: &[FecBlock],
        _total_frames: usize,
    ) -> Result<Vec<Frame>, FrameError> {
        Err(FrameError::InvalidStructure(
            "FEC decoding not implemented".to_string(),
        ))
    }

    fn can_reconstruct(&self, _available_blocks: usize, _total_frames: usize) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_encoder() {
        let encoder = NoopEncoder;
        let result = encoder.encode_batch(&[], 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_noop_decoder() {
        let decoder = NoopDecoder;
        let result = decoder.decode_batch(&[], 2);
        assert!(result.is_err());
        assert!(!decoder.can_reconstruct(5, 10));
    }
}
