//! Burst-error mitigation helpers: interleave and deinterleave payloads.
//!
//! These helpers let writers spread contiguous data across multiple frames
//! in fixed-size stripes so that a burst error damages at most a few bytes
//! of each frame instead of fully destroying a single large region. On the
//! reader side, deinterleaving reconstructs the original byte stream.
//!
//! The on-disk Durapack frame format is unchanged; applications can choose
//! to use these helpers at the payload level. Include the parameters you
//! used (group, shard_len) in your own metadata or superframe index so the
//! reader knows how to reassemble.

use alloc::vec;
use alloc::vec::Vec;
use bytes::{BufMut, Bytes, BytesMut};

/// Parameters controlling the interleaver
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterleaveParams {
    /// Number of stripes (and typically number of consecutive frames) to spread across
    pub group: usize,
    /// Size of each stripe in bytes (per frame per round)
    pub shard_len: usize,
}

impl InterleaveParams {
    /// Create new interleaving parameters
    pub const fn new(group: usize, shard_len: usize) -> Self {
        Self { group, shard_len }
    }
}

/// Split a contiguous buffer into `group` stripes in round-robin blocks of `shard_len`.
///
/// The result is `group` buffers; when emitting frames i = 0..group-1 in order,
/// append `out[i]` as the (next) payload chunk for frame i. Repeating this over
/// successive calls (or with a larger shard_len) spreads a contiguous region across
/// multiple frames, mitigating burst errors.
pub fn interleave_bytes(input: &[u8], params: InterleaveParams) -> Vec<Bytes> {
    assert!(
        params.group > 0 && params.shard_len > 0,
        "group/shard_len must be > 0",
    );

    let mut out: Vec<BytesMut> = (0..params.group).map(|_| BytesMut::new()).collect();
    let mut idx = 0usize;
    while idx < input.len() {
        for lane_buf in out.iter_mut().take(params.group) {
            if idx >= input.len() {
                break;
            }
            let end = core::cmp::min(idx + params.shard_len, input.len());
            lane_buf.put_slice(&input[idx..end]);
            idx = end;
        }
    }
    out.into_iter().map(|b| b.freeze()).collect()
}

/// Reassemble a buffer that was striped with [`interleave_bytes`].
///
/// The `stripes` vector must be ordered by lane (0..group-1). This function
/// consumes up to the length of the shortest stripe per round to preserve
/// original order. Any final partial shards are appended in lane order.
pub fn deinterleave_bytes(stripes: &[Bytes], params: InterleaveParams) -> Bytes {
    assert!(
        params.group > 0 && params.shard_len > 0,
        "group/shard_len must be > 0",
    );
    assert_eq!(
        stripes.len(),
        params.group,
        "expected {} lanes",
        params.group
    );

    let total: usize = stripes.iter().map(|b| b.len()).sum();
    let mut cursors = vec![0usize; params.group];
    let mut out = BytesMut::with_capacity(total);

    // Pull blocks round-robin
    loop {
        let mut advanced_any = false;
        for (lane, lane_buf) in stripes.iter().enumerate().take(params.group) {
            let cur = cursors[lane];
            if cur >= lane_buf.len() {
                continue;
            }
            let end = core::cmp::min(cur + params.shard_len, lane_buf.len());
            out.put_slice(&lane_buf[cur..end]);
            cursors[lane] = end;
            advanced_any = true;
        }
        if !advanced_any {
            break;
        }
    }

    out.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng};

    #[test]
    fn round_trip_interleave_deinterleave() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(123);
        let mut data = vec![0u8; 10_000];
        rng.fill_bytes(&mut data);

        let params = InterleaveParams::new(5, 257);
        let stripes = interleave_bytes(&data, params);
        assert_eq!(stripes.len(), 5);
        let reconstructed = deinterleave_bytes(&stripes, params);
        assert_eq!(reconstructed.len(), data.len());
        assert_eq!(&reconstructed[..], &data[..]);
    }
}
