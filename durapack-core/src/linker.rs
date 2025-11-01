//! Bidirectional linking and timeline reconstruction

use crate::constants::BLAKE3_HASH_SIZE;
use crate::error::FrameError;
use crate::scanner::LocatedFrame;
use crate::types::Frame;
use std::collections::HashMap;

#[cfg(feature = "logging")]
use tracing::{debug, warn};

/// A reconstructed timeline of frames
#[derive(Debug, Clone)]
pub struct Timeline {
    /// Frames in chronological order (by frame_id)
    pub frames: Vec<Frame>,

    /// Detected gaps in the sequence
    pub gaps: Vec<SequenceGap>,

    /// Frames that couldn't be linked (orphans)
    pub orphans: Vec<Frame>,
}

/// Represents a gap in the frame sequence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceGap {
    /// Frame ID before the gap
    pub before: u64,

    /// Frame ID after the gap
    pub after: u64,

    /// Expected hash of the missing frame
    pub expected_hash: Option<[u8; BLAKE3_HASH_SIZE]>,
}

/// Link frames into a timeline using their IDs and back-links
///
/// This function:
/// 1. Builds a hash map of frame_id -> frame
/// 2. Verifies back-link consistency (prev_hash matches actual previous frame)
/// 3. Orders frames chronologically
/// 4. Detects gaps in the sequence
pub fn link_frames(frames: Vec<Frame>) -> Timeline {
    #[cfg(feature = "logging")]
    debug!("Linking {} frames into timeline", frames.len());

    if frames.is_empty() {
        return Timeline {
            frames: Vec::new(),
            gaps: Vec::new(),
            orphans: Vec::new(),
        };
    }

    // Build lookup table
    let mut frame_map: HashMap<u64, Frame> = HashMap::new();
    for frame in frames {
        frame_map.insert(frame.header.frame_id, frame);
    }

    // Find first frame (prev_hash is all zeros)
    let first_frames: Vec<_> = frame_map
        .values()
        .filter(|f| f.header.is_first())
        .collect();

    if first_frames.is_empty() {
        #[cfg(feature = "logging")]
        warn!("No first frame found (prev_hash = 0), attempting to reconstruct anyway");

        return reconstruct_without_first(frame_map);
    }

    if first_frames.len() > 1 {
        #[cfg(feature = "logging")]
        warn!("Multiple first frames found, using lowest frame_id");
    }

    // Start with the first frame (lowest ID if multiple)
    let mut first_frame = first_frames[0].clone();
    for f in &first_frames[1..] {
        if f.header.frame_id < first_frame.header.frame_id {
            first_frame = (*f).clone();
        }
    }

    let mut ordered_frames = vec![first_frame.clone()];
    let mut gaps = Vec::new();
    let mut visited = HashMap::new();
    visited.insert(first_frame.header.frame_id, true);

    let mut current_hash = first_frame.compute_hash();
    let mut current_id = first_frame.header.frame_id;

    // Follow the chain forward by looking for frames that reference the current frame
    loop {
        // Find the next frame (one that has prev_hash matching current_hash)
        let next_frame = frame_map
            .values()
            .find(|f| {
                !visited.contains_key(&f.header.frame_id) && f.header.prev_hash == current_hash
            });

        match next_frame {
            Some(frame) => {
                #[cfg(feature = "logging")]
                debug!(
                    "Linked frame {} -> {}",
                    current_id, frame.header.frame_id
                );

                visited.insert(frame.header.frame_id, true);
                ordered_frames.push(frame.clone());
                current_hash = frame.compute_hash();
                current_id = frame.header.frame_id;
            }
            None => {
                // No matching frame found - check if there are unvisited frames
                let unvisited: Vec<_> = frame_map
                    .values()
                    .filter(|f| !visited.contains_key(&f.header.frame_id))
                    .collect();

                if !unvisited.is_empty() {
                    #[cfg(feature = "logging")]
                    warn!(
                        "Gap detected after frame {}: {} unvisited frames remain",
                        current_id,
                        unvisited.len()
                    );

                    // Try to find the next sequential frame by ID
                    if let Some(next_by_id) = unvisited.iter().min_by_key(|f| f.header.frame_id) {
                        gaps.push(SequenceGap {
                            before: current_id,
                            after: next_by_id.header.frame_id,
                            expected_hash: Some(next_by_id.header.prev_hash),
                        });

                        visited.insert(next_by_id.header.frame_id, true);
                        ordered_frames.push((*next_by_id).clone());
                        current_hash = next_by_id.compute_hash();
                        current_id = next_by_id.header.frame_id;
                        continue;
                    }
                }

                break;
            }
        }
    }

    // Collect orphans (frames that were never visited)
    let orphans: Vec<Frame> = frame_map
        .into_iter()
        .filter(|(id, _)| !visited.contains_key(id))
        .map(|(_, frame)| frame)
        .collect();

    #[cfg(feature = "logging")]
    debug!(
        "Timeline reconstruction complete: {} ordered frames, {} gaps, {} orphans",
        ordered_frames.len(),
        gaps.len(),
        orphans.len()
    );

    Timeline {
        frames: ordered_frames,
        gaps,
        orphans,
    }
}

/// Reconstruct timeline when no first frame is available
fn reconstruct_without_first(frame_map: HashMap<u64, Frame>) -> Timeline {
    let mut frames: Vec<_> = frame_map.into_values().collect();
    frames.sort_by_key(|f| f.header.frame_id);

    // Detect gaps by looking at frame ID sequence
    let mut gaps = Vec::new();
    for window in frames.windows(2) {
        let curr = &window[0];
        let next = &window[1];

        // Check if there's a gap in IDs or hash mismatch
        if next.header.frame_id != curr.header.frame_id + 1
            || next.header.prev_hash != curr.compute_hash()
        {
            gaps.push(SequenceGap {
                before: curr.header.frame_id,
                after: next.header.frame_id,
                expected_hash: Some(next.header.prev_hash),
            });
        }
    }

    Timeline {
        frames,
        gaps,
        orphans: Vec::new(),
    }
}

/// Link located frames (from scanner) into a timeline
pub fn link_located_frames(located_frames: Vec<LocatedFrame>) -> Timeline {
    let frames: Vec<Frame> = located_frames.into_iter().map(|lf| lf.frame).collect();
    link_frames(frames)
}

/// Verify back-link consistency of a timeline
///
/// Returns errors for any frames where the prev_hash doesn't match
/// the actual hash of the previous frame.
pub fn verify_backlinks(timeline: &Timeline) -> Vec<FrameError> {
    let mut errors = Vec::new();

    for window in timeline.frames.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        let expected_hash = prev.compute_hash();

        if curr.header.prev_hash != expected_hash {
            errors.push(FrameError::BackLinkMismatch(curr.header.frame_id));
        }
    }

    errors
}

/// Timeline statistics
#[derive(Debug, Clone)]
pub struct TimelineStats {
    /// Total number of frames
    pub total_frames: usize,

    /// Number of gaps detected
    pub gaps: usize,

    /// Number of orphaned frames
    pub orphans: usize,

    /// Continuity percentage (frames without gaps)
    pub continuity: f64,
}

impl Timeline {
    /// Get statistics about this timeline
    pub fn stats(&self) -> TimelineStats {
        let total = self.frames.len() + self.orphans.len();
        let continuity = if total == 0 {
            0.0
        } else {
            (self.frames.len() as f64 / total as f64) * 100.0
        };

        TimelineStats {
            total_frames: total,
            gaps: self.gaps.len(),
            orphans: self.orphans.len(),
            continuity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FrameHeader;
    use bytes::Bytes;

    #[test]
    fn test_link_continuous_frames() {
        // Create a chain of frames
        let frame1 = Frame::new(
            FrameHeader::new(1, [0u8; 32], 4),
            Bytes::from("test"),
        );

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(
            FrameHeader::new(2, hash1, 4),
            Bytes::from("test"),
        );

        let hash2 = frame2.compute_hash();

        let frame3 = Frame::new(
            FrameHeader::new(3, hash2, 4),
            Bytes::from("test"),
        );

        let timeline = link_frames(vec![frame1, frame2, frame3]);

        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.gaps.len(), 0);
        assert_eq!(timeline.orphans.len(), 0);
    }

    #[test]
    fn test_link_with_gap() {
        // Create frames with a missing middle frame
        let frame1 = Frame::new(
            FrameHeader::new(1, [0u8; 32], 4),
            Bytes::from("test"),
        );

        let hash1 = frame1.compute_hash();

        // Frame 2 is missing!
        let fake_hash2 = [1u8; 32];

        let frame3 = Frame::new(
            FrameHeader::new(3, fake_hash2, 4),
            Bytes::from("test"),
        );

        let timeline = link_frames(vec![frame1, frame3]);

        assert_eq!(timeline.frames.len(), 2);
        assert_eq!(timeline.gaps.len(), 1);

        let gap = &timeline.gaps[0];
        assert_eq!(gap.before, 1);
        assert_eq!(gap.after, 3);
    }

    #[test]
    fn test_link_unordered_frames() {
        // Create frames and add them out of order
        let frame1 = Frame::new(
            FrameHeader::new(1, [0u8; 32], 4),
            Bytes::from("test"),
        );

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(
            FrameHeader::new(2, hash1, 4),
            Bytes::from("test"),
        );

        let hash2 = frame2.compute_hash();

        let frame3 = Frame::new(
            FrameHeader::new(3, hash2, 4),
            Bytes::from("test"),
        );

        // Add frames in reverse order
        let timeline = link_frames(vec![frame3, frame2, frame1]);

        // Should still be ordered correctly
        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.frames[0].header.frame_id, 1);
        assert_eq!(timeline.frames[1].header.frame_id, 2);
        assert_eq!(timeline.frames[2].header.frame_id, 3);
    }

    #[test]
    fn test_verify_backlinks() {
        let frame1 = Frame::new(
            FrameHeader::new(1, [0u8; 32], 4),
            Bytes::from("test"),
        );

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(
            FrameHeader::new(2, hash1, 4),
            Bytes::from("test"),
        );

        let timeline = Timeline {
            frames: vec![frame1, frame2],
            gaps: Vec::new(),
            orphans: Vec::new(),
        };

        let errors = verify_backlinks(&timeline);
        assert_eq!(errors.len(), 0);
    }
}

