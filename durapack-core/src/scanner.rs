//! Stream scanner for damaged or noisy input

use crate::constants::{FRAME_MARKER, MAX_FRAME_SIZE, MIN_HEADER_SIZE};
use crate::decoder::decode_frame_from_bytes;
use crate::types::Frame;
use bytes::Bytes;

#[cfg(feature = "logging")]
use tracing::{debug, warn};

/// A frame found at a specific offset in the stream
#[derive(Debug, Clone)]
pub struct LocatedFrame {
    /// Byte offset where the frame marker was found
    pub offset: usize,

    /// The decoded frame
    pub frame: Frame,

    /// Total size of the frame in bytes
    pub size: usize,
}

/// Scan a byte stream for valid frames, even if the stream is damaged
///
/// This function:
/// 1. Searches byte-by-byte for the frame marker
/// 2. Attempts to decode a frame at each potential position
/// 3. Validates the frame and collects successfully decoded frames
/// 4. Continues scanning after each frame (or failed attempt)
///
/// This allows recovery of valid frames even when:
/// - The start of the stream is corrupted
/// - There are gaps or corruption between frames
/// - Frames are missing or damaged
pub fn scan_stream(data: &[u8]) -> Vec<LocatedFrame> {
    let mut results = Vec::new();
    let mut pos = 0;

    #[cfg(feature = "logging")]
    debug!("Starting stream scan of {} bytes", data.len());

    while pos < data.len() {
        // Look for frame marker
        if let Some(marker_pos) = find_marker(&data[pos..]) {
            let absolute_pos = pos + marker_pos;

            #[cfg(feature = "logging")]
            debug!("Found potential marker at offset {}", absolute_pos);

            // Try to decode frame starting at this position
            match try_decode_at_offset(data, absolute_pos) {
                Ok(located_frame) => {
                    #[cfg(feature = "logging")]
                    debug!(
                        "Successfully decoded frame {} at offset {} (size: {} bytes)",
                        located_frame.frame.header.frame_id,
                        located_frame.offset,
                        located_frame.size
                    );

                    // Jump past this frame
                    pos = absolute_pos + located_frame.size;
                    results.push(located_frame);
                }
                Err(e) => {
                    #[cfg(feature = "logging")]
                    warn!("Failed to decode frame at offset {}: {}", absolute_pos, e);

                    // Move past this marker and continue searching
                    pos = absolute_pos + FRAME_MARKER.len();
                }
            }
        } else {
            // No more markers found
            break;
        }
    }

    #[cfg(feature = "logging")]
    debug!(
        "Scan complete: found {} valid frames out of {} bytes scanned",
        results.len(),
        data.len()
    );

    results
}

/// Find the next occurrence of the frame marker
fn find_marker(data: &[u8]) -> Option<usize> {
    // Fast substring search; memchr dispatches to optimized backends (SSE2/AVX2/NEON)
    if data.len() >= FRAME_MARKER.len() {
        if let Some(pos) = memchr::memmem::find(data, FRAME_MARKER) {
            return Some(pos);
        }
    }
    // Fallback: naive window scan
    data.windows(FRAME_MARKER.len())
        .position(|window| window == FRAME_MARKER)
}

/// Try to decode a frame at a specific offset
fn try_decode_at_offset(
    data: &[u8],
    offset: usize,
) -> Result<LocatedFrame, crate::error::FrameError> {
    // Need at least minimum header size
    if offset + MIN_HEADER_SIZE > data.len() {
        return Err(crate::error::FrameError::IncompleteFrame {
            expected: MIN_HEADER_SIZE,
            actual: data.len() - offset,
        });
    }

    // Read payload length from header to determine total frame size
    let payload_len_offset = offset + 4 + 1 + 8 + 32; // marker + version + frame_id + prev_hash
    let payload_len = u32::from_be_bytes([
        data[payload_len_offset],
        data[payload_len_offset + 1],
        data[payload_len_offset + 2],
        data[payload_len_offset + 3],
    ]);

    // Read flags to determine trailer size
    let flags_offset = payload_len_offset + 4;
    let flags = crate::constants::FrameFlags::new(data[flags_offset]);
    let trailer_size = flags.trailer_type().size();

    // Calculate total frame size
    let total_size = MIN_HEADER_SIZE + payload_len as usize + trailer_size;

    // Sanity check: frame size must be reasonable
    if total_size > MAX_FRAME_SIZE as usize {
        return Err(crate::error::FrameError::FrameTooLarge(
            total_size as u32,
            MAX_FRAME_SIZE,
        ));
    }

    // Check if we have enough data
    if offset + total_size > data.len() {
        return Err(crate::error::FrameError::IncompleteFrame {
            expected: total_size,
            actual: data.len() - offset,
        });
    }

    // Try to decode the frame
    let frame_data = &data[offset..offset + total_size];
    let frame = decode_frame_from_bytes(frame_data)?;

    Ok(LocatedFrame {
        offset,
        frame,
        size: total_size,
    })
}

/// Scan statistics
#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    /// Total bytes scanned
    pub bytes_scanned: usize,

    /// Number of valid frames found
    pub frames_found: usize,

    /// Number of potential markers found
    pub markers_found: usize,

    /// Number of decode failures
    pub decode_failures: usize,

    /// Total bytes recovered (sum of all valid frame sizes)
    pub bytes_recovered: usize,
}

impl ScanStats {
    /// Calculate recovery rate as a percentage
    pub fn recovery_rate(&self) -> f64 {
        if self.bytes_scanned == 0 {
            0.0
        } else {
            (self.bytes_recovered as f64 / self.bytes_scanned as f64) * 100.0
        }
    }
}

/// Scan stream with statistics
pub fn scan_stream_with_stats(data: &[u8]) -> (Vec<LocatedFrame>, ScanStats) {
    let mut stats = ScanStats {
        bytes_scanned: data.len(),
        ..Default::default()
    };

    let mut results = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        if let Some(marker_pos) = find_marker(&data[pos..]) {
            let absolute_pos = pos + marker_pos;
            stats.markers_found += 1;

            match try_decode_at_offset(data, absolute_pos) {
                Ok(located_frame) => {
                    stats.bytes_recovered += located_frame.size;
                    pos = absolute_pos + located_frame.size;
                    results.push(located_frame);
                }
                Err(_) => {
                    stats.decode_failures += 1;
                    pos = absolute_pos + FRAME_MARKER.len();
                }
            }
        } else {
            break;
        }
    }

    stats.frames_found = results.len();

    (results, stats)
}

/// Scan a byte buffer (Bytes) and return zero-copy frames by slicing
pub fn scan_stream_zero_copy(buf: Bytes) -> Vec<LocatedFrame> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        if let Some(rel) = find_marker(&buf[pos..]) {
            let at = pos + rel;
            // Fast path: compute total size to slice just once
            match try_decode_at_offset(&buf, at) {
                Ok(loc) => {
                    let start = at;
                    let end = at + loc.size;
                    let slice = buf.slice(start..end);
                    if let Ok(frame) = crate::decoder::decode_frame_from_bytes_zero_copy(slice) {
                        results.push(LocatedFrame {
                            offset: at,
                            size: loc.size,
                            frame,
                        });
                        pos = end;
                        continue;
                    }
                    // Fallback to advancing by marker if zero-copy decode failed unexpectedly
                    pos = at + FRAME_MARKER.len();
                }
                Err(_) => pos = at + FRAME_MARKER.len(),
            }
        } else {
            break;
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::FrameBuilder;
    use bytes::Bytes;

    #[test]
    fn test_scan_clean_stream() {
        // Create a clean stream with multiple frames
        let frame1 = FrameBuilder::new(1)
            .payload(Bytes::from("frame 1"))
            .mark_first()
            .build()
            .unwrap();

        let frame2 = FrameBuilder::new(2)
            .payload(Bytes::from("frame 2"))
            .build()
            .unwrap();

        let frame3 = FrameBuilder::new(3)
            .payload(Bytes::from("frame 3"))
            .mark_last()
            .build()
            .unwrap();

        let mut stream = Vec::new();
        stream.extend_from_slice(&frame1);
        stream.extend_from_slice(&frame2);
        stream.extend_from_slice(&frame3);

        let results = scan_stream(&stream);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].frame.header.frame_id, 1);
        assert_eq!(results[1].frame.header.frame_id, 2);
        assert_eq!(results[2].frame.header.frame_id, 3);
    }

    #[test]
    fn test_scan_with_corruption() {
        // Create frames with corruption between them
        let frame1 = FrameBuilder::new(1)
            .payload(Bytes::from("frame 1"))
            .with_crc32c()
            .build()
            .unwrap();

        let frame2 = FrameBuilder::new(2)
            .payload(Bytes::from("frame 2"))
            .with_crc32c()
            .build()
            .unwrap();

        let mut stream = Vec::new();
        stream.extend_from_slice(&frame1);
        stream.extend_from_slice(b"GARBAGE DATA HERE!!!"); // Corruption
        stream.extend_from_slice(&frame2);

        let results = scan_stream(&stream);

        // Should find both valid frames
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].frame.header.frame_id, 1);
        assert_eq!(results[1].frame.header.frame_id, 2);
    }

    #[test]
    fn test_scan_missing_start() {
        // Create a stream missing the first part
        let frame1 = FrameBuilder::new(1)
            .payload(Bytes::from("frame 1"))
            .build()
            .unwrap();

        let frame2 = FrameBuilder::new(2)
            .payload(Bytes::from("frame 2"))
            .build()
            .unwrap();

        let mut stream = Vec::new();
        stream.extend_from_slice(&frame1);
        stream.extend_from_slice(&frame2);

        // Skip the first 20 bytes (corrupt the start)
        let damaged_stream = &stream[20..];

        let results = scan_stream(damaged_stream);

        // Should still find at least one frame
        assert!(!results.is_empty());
    }

    #[test]
    fn test_scan_stats() {
        let frame1 = FrameBuilder::new(1)
            .payload(Bytes::from("test"))
            .build()
            .unwrap();

        let (results, stats) = scan_stream_with_stats(&frame1);

        assert_eq!(results.len(), 1);
        assert_eq!(stats.frames_found, 1);
        assert_eq!(stats.bytes_scanned, frame1.len());
        assert!(stats.recovery_rate() > 99.0); // Should be close to 100%
    }
}
