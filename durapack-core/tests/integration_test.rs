//! Integration tests for the complete encode → corrupt → scan → rethread flow

use bytes::Bytes;
use durapack_core::{
    encoder::FrameBuilder, linker::link_frames, scanner::scan_stream, types::Frame,
};

#[test]
fn test_full_workflow_clean() {
    // Step 1: Create frames with proper linking
    let frame1_struct = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1 data"))
        .mark_first()
        .with_crc32c()
        .build_struct()
        .unwrap();

    let hash1 = frame1_struct.compute_hash();

    let frame2_struct = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2 data"))
        .prev_hash(hash1)
        .with_crc32c()
        .build_struct()
        .unwrap();

    let hash2 = frame2_struct.compute_hash();

    let frame3_struct = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3 data"))
        .prev_hash(hash2)
        .mark_last()
        .with_crc32c()
        .build_struct()
        .unwrap();

    // Step 2: Encode and concatenate into stream
    let frame1 = durapack_core::encoder::encode_frame_struct(&frame1_struct).unwrap();
    let frame2 = durapack_core::encoder::encode_frame_struct(&frame2_struct).unwrap();
    let frame3 = durapack_core::encoder::encode_frame_struct(&frame3_struct).unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(&frame2);
    stream.extend_from_slice(&frame3);

    // Step 3: Scan stream
    let located_frames = scan_stream(&stream);

    assert_eq!(located_frames.len(), 3);

    // Step 4: Rethread timeline
    let frames: Vec<_> = located_frames.into_iter().map(|lf| lf.frame).collect();
    let timeline = link_frames(frames);

    assert_eq!(timeline.frames.len(), 3);
    assert_eq!(timeline.gaps.len(), 0);
    assert_eq!(timeline.orphans.len(), 0);
}

#[test]
fn test_workflow_with_corruption() {
    // Create clean stream
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Data 1"))
        .mark_first()
        .with_blake3()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Data 2"))
        .with_blake3()
        .build()
        .unwrap();

    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Data 3"))
        .with_blake3()
        .build()
        .unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(b"CORRUPT GARBAGE DATA HERE!!!"); // Corruption!
    stream.extend_from_slice(&frame2);
    stream.extend_from_slice(&frame3);

    // Scan should recover all valid frames
    let located_frames = scan_stream(&stream);

    assert!(
        located_frames.len() >= 2,
        "Should recover at least 2 frames despite corruption"
    );
}

#[test]
fn test_workflow_missing_middle_frame() {
    // Create frames but skip frame 2
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .mark_first()
        .with_crc32c()
        .build()
        .unwrap();

    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_crc32c()
        .build()
        .unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    // Frame 2 missing!
    stream.extend_from_slice(&frame3);

    let located_frames = scan_stream(&stream);
    assert_eq!(located_frames.len(), 2);

    let frames: Vec<_> = located_frames.into_iter().map(|lf| lf.frame).collect();
    let timeline = link_frames(frames);

    // Should detect a gap
    assert!(!timeline.gaps.is_empty());
}

#[test]
fn test_workflow_shuffled_frames() {
    // Create frames
    let frame1 = create_test_frame(1, [0u8; 32], "Frame 1");
    let hash1 = frame1.compute_hash();

    let frame2 = create_test_frame(2, hash1, "Frame 2");
    let hash2 = frame2.compute_hash();

    let frame3 = create_test_frame(3, hash2, "Frame 3");

    // Encode and shuffle
    let enc1 = durapack_core::encoder::encode_frame_struct(&frame1).unwrap();
    let enc2 = durapack_core::encoder::encode_frame_struct(&frame2).unwrap();
    let enc3 = durapack_core::encoder::encode_frame_struct(&frame3).unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&enc3); // Out of order!
    stream.extend_from_slice(&enc1);
    stream.extend_from_slice(&enc2);

    let located_frames = scan_stream(&stream);
    let frames: Vec<_> = located_frames.into_iter().map(|lf| lf.frame).collect();
    let timeline = link_frames(frames);

    // Should reorder correctly
    assert_eq!(timeline.frames[0].header.frame_id, 1);
    assert_eq!(timeline.frames[1].header.frame_id, 2);
    assert_eq!(timeline.frames[2].header.frame_id, 3);
}

fn create_test_frame(id: u64, prev_hash: [u8; 32], payload: &str) -> Frame {
    Frame::new(
        durapack_core::types::FrameHeader::new(id, prev_hash, payload.len() as u32),
        Bytes::from(payload.to_string()),
    )
}
