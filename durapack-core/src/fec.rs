//! Forward Error Correction traits (interface only)
//!
//! This module defines traits for FEC encoding and decoding.
//! Concrete implementations (Reed-Solomon, Raptor, etc.) are left for future work.

use crate::error::FrameError;
use crate::types::Frame;
use alloc::string::ToString;
use alloc::vec::Vec;

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

#[cfg(feature = "fec-rs")]
use reed_solomon_erasure::galois_8::ReedSolomon;

/// Reed–Solomon (systematic) encoder/decoder using reed-solomon-erasure
#[cfg(feature = "fec-rs")]
#[derive(Debug, Clone)]
pub struct RsEncoder {
    /// Data shards (N) per block
    pub data_shards: usize,
    /// Parity shards (K) per block
    pub parity_shards: usize,
}

#[cfg(feature = "fec-rs")]
impl RsEncoder {
    /// Create a new Reed–Solomon encoder with data and parity shard counts
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }
}

#[cfg(feature = "fec-rs")]
impl RedundancyEncoder for RsEncoder {
    fn encode_batch(
        &self,
        frames: &[Frame],
        redundancy: usize,
    ) -> Result<Vec<FecBlock>, FrameError> {
        // If redundancy is provided, prefer it over configured parity_shards
        let k = if redundancy > 0 {
            redundancy
        } else {
            self.parity_shards
        };
        let n = frames.len();
        if n == 0 {
            return Ok(Vec::new());
        }
        // Ensure configured data_shards matches input frames when using block RS
        let data_shards = self.data_shards.max(n);

        // Gather payloads as shards; for simplicity use equal-sized shards via padding
        // In a production system you would stripe and align shards carefully.
        let max_len = frames.iter().map(|f| f.payload.len()).max().unwrap_or(0);
        let shard_len = max_len;

        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(data_shards + k);
        // Data shards from frames; pad to shard_len
        for frame in frames.iter().take(data_shards) {
            let p = &frame.payload;
            let mut v = vec![0u8; shard_len];
            v[..p.len()].copy_from_slice(p);
            shards.push(v);
        }
        // If data_shards exceeds provided frames, fill remaining with zeros
        if data_shards > n {
            for _ in 0..(data_shards - n) {
                shards.push(vec![0u8; shard_len]);
            }
        }
        // Parity shards: empty buffers to be filled
        for _ in 0..k {
            shards.push(vec![0u8; shard_len]);
        }

        // Build RS over (data_shards, k)
        let rsc = ReedSolomon::new(data_shards, k)
            .map_err(|_| FrameError::InvalidStructure("invalid RS parameters".to_string()))?;
        let mut shard_refs: Vec<&mut [u8]> = shards.iter_mut().map(|v| v.as_mut_slice()).collect();
        rsc.encode(&mut shard_refs)
            .map_err(|_| FrameError::InvalidStructure("RS encode failed".to_string()))?;

        // Build FecBlocks: original frames as first N blocks, then K parity blocks
        let mut out = Vec::with_capacity(data_shards + k);
        let block_id = 0u64; // caller can set/track higher-level IDs; keep 0 here
        for (i, data) in shards.into_iter().enumerate() {
            out.push(FecBlock {
                block_id,
                index: i as u32,
                total_blocks: (data_shards + k) as u32,
                data,
            });
        }
        Ok(out)
    }
}

#[cfg(feature = "fec-rs")]
/// Reed–Solomon decoder for reconstructing missing shards (requires `fec-rs` feature)
#[derive(Debug, Clone)]
pub struct RsDecoder;

#[cfg(feature = "fec-rs")]
impl RedundancyDecoder for RsDecoder {
    fn decode_batch(
        &self,
        blocks: &[FecBlock],
        total_frames: usize,
    ) -> Result<Vec<Frame>, FrameError> {
        if blocks.is_empty() {
            return Err(FrameError::InvalidStructure("no blocks".to_string()));
        }
        // All blocks must have same total_blocks and shard_len
        let total_blocks = blocks[0].total_blocks as usize;
        let shard_len = blocks[0].data.len();
        let data_shards = total_frames;
        let parity_shards = total_blocks.saturating_sub(data_shards);

        let rsc = ReedSolomon::new(data_shards, parity_shards)
            .map_err(|_| FrameError::InvalidStructure("invalid RS parameters".to_string()))?;

        // Prepare option shards (None for missing); let RS allocate/overwrite as needed
        let mut shards: Vec<Option<Vec<u8>>> = vec![None; total_blocks];
        for b in blocks {
            if (b.index as usize) < total_blocks && b.data.len() == shard_len {
                shards[b.index as usize] = Some(b.data.clone());
            }
        }

        rsc.reconstruct(&mut shards)
            .map_err(|_| FrameError::InvalidStructure("RS reconstruct failed".to_string()))?;

        // Collect first N data shards into frames with empty headers (caller to relink)
        let mut frames = Vec::with_capacity(total_frames);
        for maybe in shards.iter().take(total_frames) {
            if let Some(buf_vec) = maybe.as_ref() {
                // Trim trailing zeros (padding); in a real system carry length metadata
                let mut end = buf_vec.len();
                while end > 0 && buf_vec[end - 1] == 0 {
                    end -= 1;
                }
                let payload = bytes::Bytes::copy_from_slice(&buf_vec[..end]);
                // Minimal frame wrapper; header must be supplied by higher layer.
                // For now, we create empty headers that will be relinked by caller logic.
                let header = crate::types::FrameHeader::new(0, [0u8; 32], payload.len() as u32);
                frames.push(crate::types::Frame::new(header, payload));
            } else {
                return Err(FrameError::InvalidStructure(
                    "missing reconstructed shard".to_string(),
                ));
            }
        }
        Ok(frames)
    }

    fn can_reconstruct(&self, available_blocks: usize, total_frames: usize) -> bool {
        available_blocks >= total_frames
    }
}

/// Interleaved RS helper: split stripes then RS across stripes for burst-damage media
#[cfg(feature = "fec-rs")]
pub fn rs_interleaved_encode(
    stripes: &[bytes::Bytes],
    parity_shards: usize,
) -> Result<Vec<FecBlock>, FrameError> {
    let data_shards = stripes.len();
    let mut shards: Vec<Vec<u8>> = stripes.iter().map(|b| b.to_vec()).collect();
    let shard_len = shards.iter().map(|v| v.len()).max().unwrap_or(0);
    for v in shards.iter_mut() {
        if v.len() < shard_len {
            v.resize(shard_len, 0);
        }
    }
    for _ in 0..parity_shards {
        shards.push(vec![0u8; shard_len]);
    }
    let rsc = ReedSolomon::new(data_shards, parity_shards)
        .map_err(|_| FrameError::InvalidStructure("invalid RS parameters".to_string()))?;
    let mut refs: Vec<&mut [u8]> = shards.iter_mut().map(|v| v.as_mut_slice()).collect();
    rsc.encode(&mut refs)
        .map_err(|_| FrameError::InvalidStructure("RS encode failed".to_string()))?;
    let block_id = 0u64;
    Ok(refs
        .into_iter()
        .enumerate()
        .map(|(i, r)| FecBlock {
            block_id,
            index: i as u32,
            total_blocks: (data_shards + parity_shards) as u32,
            data: r.to_vec(),
        })
        .collect())
}

/// Proof-of-concept RaptorQ encoder (stub behind `fec-raptorq` feature)
#[cfg(feature = "fec-raptorq")]
#[derive(Debug, Clone)]
pub struct RaptorQEncoder;

#[cfg(feature = "fec-raptorq")]
impl RedundancyEncoder for RaptorQEncoder {
    fn encode_batch(
        &self,
        _frames: &[Frame],
        _redundancy: usize,
    ) -> Result<Vec<FecBlock>, FrameError> {
        Err(FrameError::InvalidStructure(
            "RaptorQ PoC not implemented".to_string(),
        ))
    }
}

/// Proof-of-concept RaptorQ decoder (stub behind `fec-raptorq` feature)
#[cfg(feature = "fec-raptorq")]
#[derive(Debug, Clone)]
pub struct RaptorQDecoder;

#[cfg(feature = "fec-raptorq")]
impl RedundancyDecoder for RaptorQDecoder {
    fn decode_batch(
        &self,
        _blocks: &[FecBlock],
        _total_frames: usize,
    ) -> Result<Vec<Frame>, FrameError> {
        Err(FrameError::InvalidStructure(
            "RaptorQ PoC not implemented".to_string(),
        ))
    }

    fn can_reconstruct(&self, _available_blocks: usize, _total_frames: usize) -> bool {
        false
    }
}

/// Proof-of-concept LDPC encoder (stub behind `fec-ldpc` feature)
#[cfg(feature = "fec-ldpc")]
#[derive(Debug, Clone)]
pub struct LdpcEncoder;

#[cfg(feature = "fec-ldpc")]
impl RedundancyEncoder for LdpcEncoder {
    fn encode_batch(
        &self,
        _frames: &[Frame],
        _redundancy: usize,
    ) -> Result<Vec<FecBlock>, FrameError> {
        Err(FrameError::InvalidStructure(
            "LDPC PoC not implemented".to_string(),
        ))
    }
}

/// Proof-of-concept LDPC decoder (stub behind `fec-ldpc` feature)
#[cfg(feature = "fec-ldpc")]
#[derive(Debug, Clone)]
pub struct LdpcDecoder;

#[cfg(feature = "fec-ldpc")]
impl RedundancyDecoder for LdpcDecoder {
    fn decode_batch(
        &self,
        _blocks: &[FecBlock],
        _total_frames: usize,
    ) -> Result<Vec<Frame>, FrameError> {
        Err(FrameError::InvalidStructure(
            "LDPC PoC not implemented".to_string(),
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

    #[cfg(feature = "fec-rs")]
    #[test]
    fn rs_round_trip_simple() {
        use bytes::Bytes;
        // Build a small batch of frames
        let payloads = [Bytes::from_static(b"A"), Bytes::from_static(b"BC")];
        let frames: Vec<Frame> = payloads
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let header = crate::types::FrameHeader::new(i as u64, [0u8; 32], p.len() as u32);
                Frame::new(header, p.clone())
            })
            .collect();
        let enc = RsEncoder::new(frames.len(), 1);
        let blocks = enc.encode_batch(&frames, 0).expect("encode ok");
        // Drop one data shard and attempt reconstruction
        let avail: Vec<FecBlock> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| if i == 0 { None } else { Some(b.clone()) })
            .collect();
        let dec = RsDecoder;
        let rec = dec.decode_batch(&avail, frames.len()).expect("decode ok");
        assert_eq!(rec.len(), frames.len());
        assert_eq!(rec[0].payload, frames[0].payload);
        assert_eq!(rec[1].payload, frames[1].payload);
    }
}
