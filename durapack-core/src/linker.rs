//! Bidirectional linking and timeline reconstruction

use crate::constants::BLAKE3_HASH_SIZE;
use crate::error::FrameError;
use crate::scanner::LocatedFrame;
use crate::types::Frame;
use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

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

/// Reason for an inferred gap
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GapReason {
    /// IDs skipped (e.g., 10 -> 13)
    MissingById,
    /// Next frame ID is sequential but backlink hash mismatches
    MissingByHash,
}

/// A gap with a reason code
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapDetail {
    /// The basic gap info
    pub gap: SequenceGap,
    /// Classified reason
    pub reason: GapReason,
}

/// Multiple successors reference the same predecessor (branch/conflict)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainConflict {
    /// Frame ID of the predecessor where branching occurs
    pub at: u64,
    /// Successor candidate frame IDs that reference the same predecessor
    pub contenders: Vec<u64>,
}

/// Connected set of orphan frames (by hash linkage among orphans)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanCluster {
    /// Frame IDs included in this cluster
    pub ids: Vec<u64>,
}

/// Human-friendly recovery hints for operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryRecipe {
    /// Suggest inserting a parity/redundancy frame in the gap
    InsertParityFrame {
        /// The pair of frame IDs surrounding the gap
        between: (u64, u64),
        /// Why this parity suggestion was emitted
        reason: String,
    },
    /// Suggest adjusting the byte offset near a frame by a signed delta
    RewindOffset {
        /// Frame ID near which the offset adjustment is suggested
        near_frame: u64,
        /// Signed number of bytes to move the read cursor to realign
        by_bytes: isize,
        /// Why this offset suggestion was emitted
        reason: String,
    },
}

/// Detailed report derived from a reconstructed timeline
#[derive(Debug, Clone)]
pub struct TimelineReport {
    /// The base timeline (ordered frames, gaps, orphans)
    pub timeline: Timeline,
    /// Gap classification
    pub gap_details: Vec<GapDetail>,
    /// Conflicts detected where multiple successors reference the same predecessor
    pub conflicts: Vec<ChainConflict>,
    /// Orphan clusters (connected components among orphans)
    pub orphan_clusters: Vec<OrphanCluster>,
    /// Recovery suggestions
    pub recipes: Vec<RecoveryRecipe>,
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
    tracing::debug!("Linking {} frames into timeline", frames.len());

    if frames.is_empty() {
        return Timeline {
            frames: Vec::new(),
            gaps: Vec::new(),
            orphans: Vec::new(),
        };
    }

    // Build lookup table
    let mut frame_map: BTreeMap<u64, Frame> = BTreeMap::new();
    for frame in frames {
        frame_map.insert(frame.header.frame_id, frame);
    }

    // Find first frame (prev_hash is all zeros)
    let first_frames: Vec<_> = frame_map.values().filter(|f| f.header.is_first()).collect();

    if first_frames.is_empty() {
        #[cfg(feature = "logging")]
        tracing::warn!("No first frame found (prev_hash = 0), attempting to reconstruct anyway");

        return reconstruct_without_first(frame_map);
    }

    if first_frames.len() > 1 {
        #[cfg(feature = "logging")]
        tracing::warn!("Multiple first frames found, using lowest frame_id");
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
    let mut visited: BTreeMap<u64, bool> = BTreeMap::new();
    visited.insert(first_frame.header.frame_id, true);

    let mut current_hash = first_frame.compute_hash();
    let mut current_id = first_frame.header.frame_id;

    // Follow the chain forward by looking for frames that reference the current frame
    loop {
        // Find the next frame (one that has prev_hash matching current_hash)
        let next_frame = frame_map.values().find(|f| {
            !visited.contains_key(&f.header.frame_id) && f.header.prev_hash == current_hash
        });

        match next_frame {
            Some(frame) => {
                #[cfg(feature = "logging")]
                tracing::debug!("Linked frame {} -> {}", current_id, frame.header.frame_id);

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
                    tracing::warn!(
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
    tracing::debug!(
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
fn reconstruct_without_first(frame_map: BTreeMap<u64, Frame>) -> Timeline {
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

/// Analyze a set of frames and produce a detailed report
pub fn analyze_timeline(frames: Vec<Frame>) -> TimelineReport {
    let timeline = link_frames(frames);
    build_report_from_timeline(&timeline, None)
}

/// Analyze located frames (with offsets) to include byte-offset recipes
pub fn analyze_located_frames(located_frames: Vec<LocatedFrame>) -> TimelineReport {
    let frames: Vec<Frame> = located_frames.iter().map(|lf| lf.frame.clone()).collect();
    let timeline = link_frames(frames);
    build_report_from_timeline(&timeline, Some(&located_frames))
}

fn build_report_from_timeline(
    timeline: &Timeline,
    located: Option<&[LocatedFrame]>,
) -> TimelineReport {
    // Build map: frame_id -> frame and hash -> frame_id
    let mut id_map: BTreeMap<u64, &Frame> = BTreeMap::new();
    let mut hash_to_id: BTreeMap<[u8; BLAKE3_HASH_SIZE], u64> = BTreeMap::new();

    for f in timeline.frames.iter().chain(timeline.orphans.iter()) {
        id_map.insert(f.header.frame_id, f);
        hash_to_id.insert(f.compute_hash(), f.header.frame_id);
    }

    // Classify gaps
    let mut gap_details = Vec::new();
    for g in &timeline.gaps {
        let reason = if g.after != g.before + 1 {
            GapReason::MissingById
        } else {
            // If both frames exist, check backlink
            match (id_map.get(&g.before), id_map.get(&g.after)) {
                (Some(prev), Some(next)) => {
                    let expected = prev.compute_hash();
                    if next.header.prev_hash == expected {
                        // Strictly contiguous by ID with matching hash shouldn't be a gap,
                        // but if it is in the list, default to MissingById for safety
                        GapReason::MissingById
                    } else {
                        GapReason::MissingByHash
                    }
                }
                _ => GapReason::MissingById,
            }
        };
        gap_details.push(GapDetail {
            gap: g.clone(),
            reason,
        });
    }

    // Conflicts: multiple frames that reference the same predecessor's hash
    let mut preds_to_successors: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for f in id_map.values() {
        if let Some(&pred_id) = hash_to_id.get(&f.header.prev_hash) {
            preds_to_successors
                .entry(pred_id)
                .or_default()
                .push(f.header.frame_id);
        }
    }
    let mut conflicts = Vec::new();
    for (pred, succs) in preds_to_successors {
        if succs.len() > 1 {
            let mut contenders = succs.clone();
            contenders.sort_unstable();
            conflicts.push(ChainConflict {
                at: pred,
                contenders,
            });
        }
    }

    // Orphan clusters: connected components among orphans linking by hash relationships
    let orphan_set: BTreeSet<u64> = timeline.orphans.iter().map(|f| f.header.frame_id).collect();
    let mut orphan_links: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for f in &timeline.orphans {
        let fid = f.header.frame_id;
        // Link to predecessor if it is also an orphan
        if let Some(&pred_id) = hash_to_id.get(&f.header.prev_hash) {
            if orphan_set.contains(&pred_id) {
                orphan_links.entry(fid).or_default().push(pred_id);
                orphan_links.entry(pred_id).or_default().push(fid);
            }
        }
        // Link to successors among orphans
        for (&h, &hid) in &hash_to_id {
            if orphan_set.contains(&hid) && f.header.prev_hash == h {
                // already handled by predecessor check above when roles swap
            }
        }
    }
    // BFS components
    let mut visited = BTreeSet::new();
    let mut orphan_clusters = Vec::new();
    for &fid in &orphan_set {
        if visited.contains(&fid) {
            continue;
        }
        let mut stack = vec![fid];
        let mut ids = Vec::new();
        visited.insert(fid);
        while let Some(u) = stack.pop() {
            ids.push(u);
            if let Some(neis) = orphan_links.get(&u) {
                for &v in neis {
                    if visited.insert(v) {
                        stack.push(v);
                    }
                }
            }
        }
        ids.sort_unstable();
        orphan_clusters.push(OrphanCluster { ids });
    }

    // Recovery recipes
    let mut recipes = Vec::new();
    // Insert parity frame suggestion for each gap
    for gd in &gap_details {
        let r = RecoveryRecipe::InsertParityFrame {
            between: (gd.gap.before, gd.gap.after),
            reason: format!("gap detected: {:?}", gd.reason),
        };
        recipes.push(r);
    }
    // Rewind/advance offsets if we have offsets
    if let Some(locs) = located {
        // Build offset map and size map by frame_id
        let mut off: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
        for lf in locs.iter() {
            off.insert(lf.frame.header.frame_id, (lf.offset, lf.size));
        }
        for gd in &gap_details {
            if let (Some((off_before, size_before)), Some((off_after, _))) =
                (off.get(&gd.gap.before), off.get(&gd.gap.after))
            {
                let expected_end = off_before + size_before;
                let actual_start = *off_after;
                let delta = actual_start as isize - expected_end as isize;
                let r = RecoveryRecipe::RewindOffset {
                    near_frame: gd.gap.after,
                    by_bytes: delta,
                    reason: String::from("non-contiguous offsets across gap"),
                };
                recipes.push(r);
            }
        }
    }

    TimelineReport {
        timeline: timeline.clone(),
        gap_details,
        conflicts,
        orphan_clusters,
        recipes,
    }
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

    /// Seek to a target frame ID using skip-list backlinks if available
    pub fn seek_with_skiplist(&self, target_id: u64) -> Option<&Frame> {
        // Build a map for quick lookup
        let mut map = alloc::collections::BTreeMap::new();
        for f in &self.frames {
            map.insert(f.header.frame_id, f);
        }
        // If exact exists
        if let Some(f) = map.get(&target_id) {
            return Some(f);
        }
        // Heuristic: start from the largest frame_id <= target
        let mut cursor = map.range(..=target_id).next_back().map(|(_, f)| *f)?;
        // Follow skip-links if present; otherwise walk linearly backward until found or start
        loop {
            if cursor.header.frame_id == target_id {
                return Some(cursor);
            }
            if let Some(links) = &cursor.skip_links {
                // pick the largest link not overshooting the target
                if let Some(best) = links
                    .iter()
                    .filter(|l| l.target_id <= target_id)
                    .max_by_key(|l| (l.level, l.target_id))
                {
                    if let Some(next) = map.get(&best.target_id) {
                        cursor = next;
                        continue;
                    }
                }
            }
            // Fallback: step back by one if possible
            if let Some(prev) = map
                .range(..cursor.header.frame_id)
                .next_back()
                .map(|(_, f)| *f)
            {
                cursor = prev;
            } else {
                break;
            }
        }
        None
    }
}

/// Render a TimelineReport as a Graphviz DOT string
pub fn report_to_dot(report: &TimelineReport) -> String {
    use core::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(&mut s, "digraph timeline {{");
    let _ = writeln!(&mut s, "  rankdir=LR;");

    // Nodes: ordered frames
    for f in &report.timeline.frames {
        let _ = writeln!(
            &mut s,
            "  {} [label=\"{}\"];",
            f.header.frame_id, f.header.frame_id
        );
    }
    // Orphan nodes grouped into clusters
    for (idx, cluster) in report.orphan_clusters.iter().enumerate() {
        let _ = writeln!(&mut s, "  subgraph cluster_orphans_{} {{", idx);
        let _ = writeln!(&mut s, "    label=\"orphan cluster #{}\";", idx);
        let _ = writeln!(&mut s, "    style=dashed; color=gray;");
        for id in &cluster.ids {
            let _ = writeln!(&mut s, "    {} [style=filled, fillcolor=lightgray];", id);
        }
        let _ = writeln!(&mut s, "  }}");
    }

    // Edges: ordered links
    for win in report.timeline.frames.windows(2) {
        let a = win[0].header.frame_id;
        let b = win[1].header.frame_id;
        let _ = writeln!(&mut s, "  {} -> {};", a, b);
    }

    // Gaps with reasons
    for gd in &report.gap_details {
        let label = match gd.reason {
            GapReason::MissingById => "gap: missing-by-id",
            GapReason::MissingByHash => "gap: missing-by-hash",
        };
        let _ = writeln!(
            &mut s,
            "  {} -> {} [style=dashed, color=red, label=\"{}\"];",
            gd.gap.before, gd.gap.after, label
        );
    }

    // Conflicts at a predecessor
    for c in &report.conflicts {
        for succ in &c.contenders {
            let _ = writeln!(
                &mut s,
                "  {} -> {} [style=dotted, color=orange, label=\"conflict\"];",
                c.at, succ
            );
        }
    }

    // Recovery recipes as notes
    for (i, r) in report.recipes.iter().enumerate() {
        match r {
            RecoveryRecipe::InsertParityFrame { between, reason } => {
                let _ = writeln!(
                    &mut s,
                    "  recipe_{} [shape=note, label=\"insert parity between {} and {}\\n{}\", color=blue];",
                    i, between.0, between.1, reason
                );
            }
            RecoveryRecipe::RewindOffset {
                near_frame,
                by_bytes,
                reason,
            } => {
                let _ = writeln!(
                    &mut s,
                    "  recipe_{} [shape=note, label=\"rewind offset near {} by {} bytes\\n{}\", color=blue];",
                    i, near_frame, by_bytes, reason
                );
            }
        }
    }

    let _ = writeln!(&mut s, "}}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FrameHeader;
    use bytes::Bytes;

    #[test]
    fn test_link_continuous_frames() {
        // Create a chain of frames
        let frame1 = Frame::new(FrameHeader::new(1, [0u8; 32], 4), Bytes::from("test"));

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(FrameHeader::new(2, hash1, 4), Bytes::from("test"));

        let hash2 = frame2.compute_hash();

        let frame3 = Frame::new(FrameHeader::new(3, hash2, 4), Bytes::from("test"));

        let timeline = link_frames(vec![frame1, frame2, frame3]);

        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.gaps.len(), 0);
        assert_eq!(timeline.orphans.len(), 0);
    }

    #[test]
    fn test_link_with_gap() {
        // Create frames with a missing middle frame
        let frame1 = Frame::new(FrameHeader::new(1, [0u8; 32], 4), Bytes::from("test"));

        let _hash1 = frame1.compute_hash();

        // Frame 2 is missing!
        let fake_hash2 = [1u8; 32];

        let frame3 = Frame::new(FrameHeader::new(3, fake_hash2, 4), Bytes::from("test"));

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
        let frame1 = Frame::new(FrameHeader::new(1, [0u8; 32], 4), Bytes::from("test"));

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(FrameHeader::new(2, hash1, 4), Bytes::from("test"));

        let hash2 = frame2.compute_hash();

        let frame3 = Frame::new(FrameHeader::new(3, hash2, 4), Bytes::from("test"));

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
        let frame1 = Frame::new(FrameHeader::new(1, [0u8; 32], 4), Bytes::from("test"));

        let hash1 = frame1.compute_hash();

        let frame2 = Frame::new(FrameHeader::new(2, hash1, 4), Bytes::from("test"));

        let timeline = Timeline {
            frames: vec![frame1, frame2],
            gaps: Vec::new(),
            orphans: Vec::new(),
        };

        let errors = verify_backlinks(&timeline);
        assert_eq!(errors.len(), 0);
    }
}
