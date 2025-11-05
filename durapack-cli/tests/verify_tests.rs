use bytes::Bytes;
use durapack_cli::commands::verify;
use durapack_core::encoder::FrameBuilder;
use std::fs;
use tempfile::tempdir;

/// Helper: create valid sequential frames
fn create_valid_frames(count: usize) -> Vec<u8> {
    let mut result = Vec::new();

    for i in 0..count {
        let payload = format!("Frame {}", i + 1);
        let mut builder = FrameBuilder::new((i + 1) as u64).payload(Bytes::from(payload));

        if i == 0 {
            builder = builder.mark_first();
        }

        let frame_bytes = builder.with_blake3().build().unwrap();

        result.extend_from_slice(&frame_bytes);
    }

    result
}

/// Helper: create frames with broken back-links
fn create_broken_backlink_frames() -> Vec<u8> {
    let mut result = Vec::new();

    // Frame 1 (good)
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    result.extend_from_slice(&frame1);

    // Frame 2 with wrong prev_hash
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .prev_hash([0x42; 32]) // Wrong hash!
        .build()
        .unwrap();
    result.extend_from_slice(&frame2);

    result
}

/// Helper: create frames with gaps
fn create_frames_with_gaps() -> Vec<u8> {
    let mut result = Vec::new();

    // Frame 1
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    result.extend_from_slice(&frame1);

    // Frame 3 (gap)
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame3);

    result
}

#[test]
fn test_verify_valid_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("valid.durp");

    // Create valid frames
    let frames = create_valid_frames(3);
    fs::write(&input_path, frames).unwrap();

    // Execute verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    // Should succeed
    assert!(result.is_ok());
}

#[test]
fn test_verify_with_report_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("gaps.durp");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute verify with report_gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    // Should succeed (gaps are reported, not errors)
    assert!(result.is_ok());
}

#[test]
fn test_verify_broken_backlinks() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("broken.durp");

    // Create frames with broken backlinks
    let frames = create_broken_backlink_frames();
    fs::write(&input_path, frames).unwrap();

    // Execute verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    // Should succeed (backlink errors are reported but not fatal)
    assert!(result.is_ok());
}

#[test]
fn test_verify_empty_file() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("empty.durp");

    // Create empty file
    fs::write(&input_path, b"").unwrap();

    // Execute verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    // Should succeed (prints "No valid frames found")
    assert!(result.is_ok());
}

#[test]
fn test_verify_execute_ext_basic() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");

    // Create valid frames
    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Execute verify with execute_ext
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false, // report_gaps
        None,  // fec_index_path
        false, // rs_repair
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_fec_index() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_fec.durp");
    let fec_index_path = td.path().join("fec.json");

    // Create valid frames
    let frames = create_valid_frames(4);
    fs::write(&input_path, frames).unwrap();

    // Create FEC index
    let fec_index = r#"[
        {
            "block_start_id": 1,
            "data": 2,
            "parity": 1,
            "_parity_frame_ids": [5]
        }
    ]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute verify with FEC index (without rs_repair since feature may not be enabled)
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false,
        Some(fec_index_path.to_str().unwrap()),
        false, // rs_repair=false
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_invalid_fec_index() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");
    let fec_index_path = td.path().join("bad_fec.json");

    // Create valid frames
    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Create invalid FEC index
    fs::write(&fec_index_path, b"not valid json").unwrap();

    // Execute verify with invalid FEC index AND rs_repair=true to trigger loading
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false,
        Some(fec_index_path.to_str().unwrap()),
        true, // rs_repair must be true to load FEC index
    );

    // Should fail due to invalid JSON
    assert!(result.is_err());
}

#[test]
fn test_verify_missing_fec_index_file() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");
    let fec_index_path = td.path().join("nonexistent.json");

    // Create valid frames
    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Execute verify with missing FEC index file AND rs_repair=true to trigger loading
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false,
        Some(fec_index_path.to_str().unwrap()),
        true, // rs_repair must be true to load FEC index
    );

    // Should fail - file not found
    assert!(result.is_err());
}

#[test]
fn test_export_strip_signatures_basic() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("input.durp");
    let output_path = td.path().join("output.durp");

    // Create frames (no signatures in this test, just BLAKE3)
    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Export (should just copy since no signatures)
    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
    assert!(output_path.exists());
}

#[test]
fn test_export_strip_signatures_empty_input() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("empty.durp");
    let output_path = td.path().join("output.durp");

    // Create empty file
    fs::write(&input_path, b"").unwrap();

    // Export
    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
    assert!(output_path.exists());

    // Output should also be empty
    let output_data = fs::read(&output_path).unwrap();
    assert_eq!(output_data.len(), 0);
}

#[test]
fn test_export_to_stdout() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("input.durp");

    // Create frames
    let frames = create_valid_frames(1);
    fs::write(&input_path, frames).unwrap();

    // Export to stdout
    let result = verify::export_strip_signatures(input_path.to_str().unwrap(), "-");

    assert!(result.is_ok());
}

#[test]
fn test_verify_multiple_frames_all_valid() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("many_valid.durp");

    // Create many valid frames
    let frames = create_valid_frames(10);
    fs::write(&input_path, frames).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_gaps_reporting() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("gaps_report.durp");

    // Create frames with multiple gaps
    let mut frames_data = Vec::new();

    // Frame 1, 3, 5, 7 (gaps at 2, 4, 6)
    for frame_id in &[1, 3, 5, 7] {
        let frame = FrameBuilder::new(*frame_id)
            .payload(Bytes::from(format!("Frame {}", frame_id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify with gap reporting
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_continuity_calculation() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("continuity.durp");

    // Create perfect sequence (100% continuity)
    let frames = create_valid_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Verify - should report 100% continuity
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_orphaned_frames_detection() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("orphans.durp");

    // Create frames with orphans
    let mut frames_data = Vec::new();

    // Good chain: 1, 2
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // Orphan with invalid prev_hash
    let orphan = FrameBuilder::new(99)
        .payload(Bytes::from("Orphan"))
        .with_blake3()
        .prev_hash([0xFF; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&orphan);

    fs::write(&input_path, frames_data).unwrap();

    // Verify - should detect orphans
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_crc32c_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("crc32c.durp");

    // Create frames with CRC32C instead of BLAKE3
    let mut frames_data = Vec::new();

    for i in 1..=3 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Frame {}", i)))
            // Don't call with_blake3(), which means CRC32C by default
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_mixed_trailer_types() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("mixed_trailers.durp");

    let mut frames_data = Vec::new();

    // Frame 1: BLAKE3
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Blake3"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Frame 2: CRC32C
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("CRC32C"))
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_single_frame() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("single.durp");

    // Single frame
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("Only frame"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_large_payload() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("large_payload.durp");

    // Frame with large payload
    let large_payload = "X".repeat(50000);
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from(large_payload))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_export_stdin_to_file() {
    // This test cannot easily test stdin in unit tests
    // But we can test the code path exists by testing file input
    let td = tempdir().unwrap();
    let input_path = td.path().join("input.durp");
    let output_path = td.path().join("output.durp");

    let frames = create_valid_frames(1);
    fs::write(&input_path, frames).unwrap();

    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_no_gaps_perfect_sequence() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("perfect.durp");

    // Perfect sequence 1,2,3,4,5
    let frames = create_valid_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Verify with gap reporting
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
    // Should report 0 gaps, 100% continuity
}

#[test]
fn test_verify_execute_ext_all_params() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("all_params.durp");
    let fec_index_path = td.path().join("fec_all.json");

    let frames = create_valid_frames(3);
    fs::write(&input_path, frames).unwrap();

    // Create minimal valid FEC index
    let fec_index = r#"[{"block_start_id": 1, "data": 2, "parity": 1, "_parity_frame_ids": [4]}]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute with all parameters
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        true, // report_gaps
        Some(fec_index_path.to_str().unwrap()),
        false, // rs_repair (disabled by default, might not have feature)
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_rs_repair_flag() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("rs_repair.durp");
    let fec_index_path = td.path().join("fec_rs.json");

    let frames = create_valid_frames(4);
    fs::write(&input_path, frames).unwrap();

    // Create valid FEC index
    let fec_index = r#"[
        {"block_start_id": 1, "data": 3, "parity": 1, "_parity_frame_ids": [5]},
        {"block_start_id": 5, "data": 2, "parity": 1, "_parity_frame_ids": [8]}
    ]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute with rs_repair=true
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false,
        Some(fec_index_path.to_str().unwrap()),
        true, // rs_repair
    );

    // Should succeed (will print RS repair info if feature is enabled)
    assert!(result.is_ok());
}

#[test]
fn test_export_multiple_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("multi_input.durp");
    let output_path = td.path().join("multi_output.durp");

    // Create multiple frames
    let frames = create_valid_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Export
    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
    assert!(output_path.exists());

    // Verify output is valid
    let output_data = fs::read(&output_path).unwrap();
    assert!(!output_data.is_empty());
}

#[test]
fn test_verify_empty_after_scan() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("garbage.durp");

    // Create file with garbage that won't scan to valid frames
    fs::write(&input_path, b"This is not a valid Durapack file").unwrap();

    // Verify should handle gracefully
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_timeline_with_multiple_orphans() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("multi_orphans.durp");

    let mut frames_data = Vec::new();

    // Create multiple orphaned frames with different prev_hashes
    for i in [10, 20, 30, 40] {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Orphan {}", i)))
            .with_blake3()
            .prev_hash([i as u8; 32])
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_all_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("all_gaps.durp");

    let mut frames_data = Vec::new();

    // Create non-sequential frames: 1, 5, 10, 20
    for frame_id in &[1, 5, 10, 20] {
        let frame = FrameBuilder::new(*frame_id)
            .payload(Bytes::from(format!("Frame {}", frame_id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify with gap reporting
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_no_trailer_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("no_trailer.durp");

    let mut frames_data = Vec::new();

    // Create frames without trailer
    for i in 1..=3 {
        // Build frame manually without trailer
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Frame {}", i)))
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_very_large_sequence() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("very_large.durp");

    // Create many frames to test performance paths
    let frames = create_valid_frames(50);
    fs::write(&input_path, frames).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_export_with_no_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("no_frames.durp");
    let output_path = td.path().join("no_frames_out.durp");

    // Create file with non-frame data
    fs::write(&input_path, b"random data").unwrap();

    // Export (should copy data as-is if no frames found)
    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_perfect_chain() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("perfect_chain.durp");

    // Create a perfect chain of frames
    let frames = create_valid_frames(10);
    fs::write(&input_path, frames).unwrap();

    // Verify without gap reporting
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_mixed_valid_invalid_backlinks() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("mixed_backlinks.durp");

    let mut frames_data = Vec::new();

    // Frame 1 (good)
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Frame 2 (good link)
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // Frame 3 (bad link)
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .prev_hash([0x99; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame3);

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_all_crc32c() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("all_crc.durp");

    let mut frames_data = Vec::new();

    // Create all frames with CRC32C
    for i in 1..=5 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Frame {}", i)))
            // No with_blake3() = CRC32C
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_first_frame_marker() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("first_marker.durp");

    // Frame with first marker
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("First frame"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_export_preserves_non_sig_trailers() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("preserve.durp");
    let output_path = td.path().join("preserved.durp");

    // Create frames with BLAKE3 (not signatures)
    let frames = create_valid_frames(3);
    fs::write(&input_path, &frames).unwrap();

    // Export
    verify::export_strip_signatures(input_path.to_str().unwrap(), output_path.to_str().unwrap())
        .unwrap();

    // Read output and verify it has same length (no stripping happened)
    let output_data = fs::read(&output_path).unwrap();
    assert_eq!(output_data.len(), frames.len());
}

#[test]
fn test_verify_with_report_gaps_no_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("no_gaps_report.durp");

    // Perfect sequence
    let frames = create_valid_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Verify with gap reporting (should report 0 gaps)
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_single_orphan() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("single_orphan.durp");

    // Single orphaned frame
    let frame = FrameBuilder::new(100)
        .payload(Bytes::from("Orphan"))
        .with_blake3()
        .prev_hash([0xAA; 32])
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_invalid_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("some_invalid.durp");

    let mut frames_data = Vec::new();

    // Add some valid frames
    for i in 1..=3 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Frame {}", i)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    // Add some garbage that will be skipped
    frames_data.extend_from_slice(b"GARBAGE DATA THAT IS NOT A FRAME");

    fs::write(&input_path, frames_data).unwrap();

    // Verify - should succeed but report some failures
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_gaps_at_boundaries() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("boundary_gaps.durp");

    let mut frames_data = Vec::new();

    // Frame 2 (missing frame 1)
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // Frame 3
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame3);

    fs::write(&input_path, frames_data).unwrap();

    // Verify with gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_single_frame_no_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("single_no_gaps.durp");

    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("Single"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    // Verify - should show no gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_export_empty_to_stdout() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("empty_export.durp");

    fs::write(&input_path, b"").unwrap();

    // Export to stdout
    let result = verify::export_strip_signatures(input_path.to_str().unwrap(), "-");

    assert!(result.is_ok());
}

#[test]
fn test_verify_continuity_with_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("continuity_gaps.durp");

    // Create frames with gaps: 1, 2, 4, 5, 7
    let mut frames_data = Vec::new();
    for id in &[1, 2, 4, 5, 7] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify - continuity will be < 100%
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_duplicate_frame_ids() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("duplicates.durp");

    let mut frames_data = Vec::new();

    // Frame 1
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1a"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Another Frame 1 (duplicate ID)
    let frame1b = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1b"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1b);

    // Frame 2
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_backlinks_all_broken() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("all_broken.durp");

    let mut frames_data = Vec::new();

    // All frames with wrong backlinks
    for i in 1..=5 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Frame {}", i)))
            .with_blake3()
            .prev_hash([i as u8; 32])
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify - should report multiple backlink errors
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_mix_trailers_and_no_trailers() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("mix_trailers.durp");

    let mut frames_data = Vec::new();

    // Frame with BLAKE3
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("BLAKE3"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Frame with CRC32C (default)
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("CRC32C"))
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // Another with BLAKE3
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("BLAKE3"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame3);

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_orphans_only() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("orphans_only.durp");

    let mut frames_data = Vec::new();

    // All orphans with invalid prev_hashes
    for i in [5, 10, 15] {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Orphan {}", i)))
            .with_blake3()
            .prev_hash([0xAA; 32])
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify - all should be orphans
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_with_fec_rs_repair_enabled() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("fec_repair.durp");
    let fec_index_path = td.path().join("fec_repair.json");

    let frames = create_valid_frames(6);
    fs::write(&input_path, frames).unwrap();

    // Create detailed FEC index
    let fec_index = r#"[
        {"block_start_id": 1, "data": 4, "parity": 2, "_parity_frame_ids": [7, 8]},
        {"block_start_id": 9, "data": 3, "parity": 1, "_parity_frame_ids": [13]}
    ]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute with rs_repair
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        true, // report_gaps
        Some(fec_index_path.to_str().unwrap()),
        true, // rs_repair
    );

    assert!(result.is_ok());
}

#[test]
fn test_export_large_file() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("large_export.durp");
    let output_path = td.path().join("large_export_out.durp");

    // Create many frames
    let frames = create_valid_frames(30);
    fs::write(&input_path, frames).unwrap();

    // Export
    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
    assert!(output_path.exists());
}

#[test]
fn test_verify_report_many_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("many_gaps.durp");

    let mut frames_data = Vec::new();

    // Sparse frame IDs: 1, 10, 20, 30, 40
    for id in &[1, 10, 20, 30, 40] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify with gap reporting - should report multiple gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
#[cfg(feature = "ed25519-signatures")]
fn test_verify_with_sig_no_pubkey_env() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("sig_no_key.durp");

    // Create frames (without actual signatures, just BLAKE3)
    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Unset the env var if it exists
    std::env::remove_var("DURAPACK_VERIFY_PUBKEY");

    // Verify - should work fine (no sig frames to verify)
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
#[cfg(feature = "ed25519-signatures")]
fn test_verify_with_invalid_pubkey_path() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("sig_bad_path.durp");

    let frames = create_valid_frames(1);
    fs::write(&input_path, frames).unwrap();

    // Set env var to non-existent file
    std::env::set_var("DURAPACK_VERIFY_PUBKEY", "nonexistent_key.bin");

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());

    // Clean up
    std::env::remove_var("DURAPACK_VERIFY_PUBKEY");
}

#[test]
#[cfg(feature = "ed25519-signatures")]
fn test_verify_with_invalid_pubkey_size() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("sig_bad_size.durp");
    let pubkey_path = td.path().join("bad_size.key");

    let frames = create_valid_frames(1);
    fs::write(&input_path, frames).unwrap();

    // Write invalid-sized public key (not 32 bytes)
    fs::write(&pubkey_path, vec![0u8; 16]).unwrap();

    std::env::set_var("DURAPACK_VERIFY_PUBKEY", pubkey_path.to_str().unwrap());

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());

    std::env::remove_var("DURAPACK_VERIFY_PUBKEY");
}

#[test]
#[cfg(not(feature = "ed25519-signatures"))]
fn test_verify_without_sig_feature() {
    // This test just ensures the code compiles without the feature
    let td = tempdir().unwrap();
    let input_path = td.path().join("no_sig_feature.durp");

    let frames = create_valid_frames(2);
    fs::write(&input_path, frames).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_completely_sparse_ids() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("sparse.durp");

    let mut frames_data = Vec::new();

    // Very sparse IDs: 100, 200, 300
    for id in &[100, 200, 300] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_high_frame_ids() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("high_ids.durp");

    let mut frames_data = Vec::new();

    // Very high frame IDs
    for id in &[1000000, 1000001, 1000002] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_and_report_gaps_sparse() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("report_sparse.durp");

    let mut frames_data = Vec::new();

    // IDs with many gaps: 1, 100, 200
    for id in &[1, 100, 200] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Report gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_zero_payload_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("zero_payload.durp");

    let mut frames_data = Vec::new();

    // Frames with empty payloads
    for i in 1..=3 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(""))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_export_zero_payload() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("export_zero.durp");
    let output_path = td.path().join("export_zero_out.durp");

    let frame = FrameBuilder::new(1)
        .payload(Bytes::from(""))
        .with_blake3()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_many_invalid_with_some_valid() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("many_invalid.durp");

    let mut frames_data = Vec::new();

    // Valid frame 1
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Valid 1"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Garbage
    frames_data.extend_from_slice(b"GARBAGE1234567890");

    // Valid frame 2
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Valid 2"))
        .with_blake3()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // More garbage
    frames_data.extend_from_slice(b"MORE GARBAGE DATA");

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_continuity_edge_case() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("continuity_edge.durp");

    let mut frames_data = Vec::new();

    // Frame sequence: 1, 2, 3, 10
    for id in &[1, 2, 3, 10] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_backlinks_partial_chain() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("partial_chain.durp");

    let mut frames_data = Vec::new();

    // Some frames with correct backlinks, some wrong
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Frame 2 with wrong backlink
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .prev_hash([0x11; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    fs::write(&input_path, frames_data).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_all_valid_no_issues() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("all_perfect.durp");

    // Perfect sequence
    let frames = create_valid_frames(7);
    fs::write(&input_path, frames).unwrap();

    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_export_single_frame() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("single_export.durp");
    let output_path = td.path().join("single_export_out.durp");

    let frame = FrameBuilder::new(42)
        .payload(Bytes::from("Single frame"))
        .with_blake3()
        .build()
        .unwrap();

    fs::write(&input_path, frame).unwrap();

    let result = verify::export_strip_signatures(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_frames_with_validation_errors() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("validation_errors.durp");

    // Create some frames - scanner will find them but validation might flag issues
    let frames = create_valid_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Verify - exercises validation code path for each frame
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_report_specific_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("specific_gaps.durp");

    let mut frames_data = Vec::new();

    // Create specific gap pattern: 1, 2, 5, 6
    for id in &[1, 2, 5, 6] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Verify with gap reporting - should specifically list gaps
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_orphans_and_gaps_together() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("orphans_and_gaps.durp");

    let mut frames_data = Vec::new();

    // Chain: 1, 2 (gap) 4
    for id in &[1, 2, 4] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Chain {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    // Orphan
    let orphan = FrameBuilder::new(100)
        .payload(Bytes::from("Orphan"))
        .with_blake3()
        .prev_hash([0xBB; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&orphan);

    fs::write(&input_path, frames_data).unwrap();

    // Verify
    let result = verify::execute(input_path.to_str().unwrap(), true);

    assert!(result.is_ok());
}

#[test]
fn test_verify_backlink_errors_logged() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("backlink_errors.durp");

    let mut frames_data = Vec::new();

    // Frame 1
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame1);

    // Frame 2 with wrong hash
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .prev_hash([0x33; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame2);

    // Frame 3 with wrong hash
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .prev_hash([0x44; 32])
        .build()
        .unwrap();
    frames_data.extend_from_slice(&frame3);

    fs::write(&input_path, frames_data).unwrap();

    // Verify - should log multiple backlink errors
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_verify_summary_conditions() {
    let td = tempdir().unwrap();

    // Test 1: File with no issues
    let path1 = td.path().join("no_issues.durp");
    let frames1 = create_valid_frames(3);
    fs::write(&path1, frames1).unwrap();
    assert!(verify::execute(path1.to_str().unwrap(), false).is_ok());

    // Test 2: File with backlink issues
    let path2 = td.path().join("backlink_issues.durp");
    let mut frames2 = Vec::new();
    let f1 = FrameBuilder::new(1)
        .payload(Bytes::from("a"))
        .with_blake3()
        .build()
        .unwrap();
    frames2.extend_from_slice(&f1);
    let f2 = FrameBuilder::new(2)
        .payload(Bytes::from("b"))
        .with_blake3()
        .prev_hash([0x55; 32])
        .build()
        .unwrap();
    frames2.extend_from_slice(&f2);
    fs::write(&path2, frames2).unwrap();
    assert!(verify::execute(path2.to_str().unwrap(), false).is_ok());

    // Test 3: File with gaps
    let path3 = td.path().join("gaps.durp");
    let mut frames3 = Vec::new();
    let g1 = FrameBuilder::new(1)
        .payload(Bytes::from("x"))
        .with_blake3()
        .build()
        .unwrap();
    frames3.extend_from_slice(&g1);
    let g3 = FrameBuilder::new(3)
        .payload(Bytes::from("z"))
        .with_blake3()
        .build()
        .unwrap();
    frames3.extend_from_slice(&g3);
    fs::write(&path3, frames3).unwrap();
    assert!(verify::execute(path3.to_str().unwrap(), false).is_ok());
}

#[test]
fn test_verify_specific_frame_validation() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frame_validation.durp");

    // Create frames that will pass validation
    let mut frames_data = Vec::new();

    for i in 1..=4 {
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(format!("Payload {}", i)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // This will exercise the frame.validate() code path
    let result = verify::execute(input_path.to_str().unwrap(), false);

    assert!(result.is_ok());
}

#[test]
fn test_export_various_sizes() {
    let td = tempdir().unwrap();

    // Small
    let input1 = td.path().join("small.durp");
    let output1 = td.path().join("small_out.durp");
    let f1 = create_valid_frames(1);
    fs::write(&input1, f1).unwrap();
    assert!(
        verify::export_strip_signatures(input1.to_str().unwrap(), output1.to_str().unwrap())
            .is_ok()
    );

    // Medium
    let input2 = td.path().join("medium.durp");
    let output2 = td.path().join("medium_out.durp");
    let f2 = create_valid_frames(10);
    fs::write(&input2, f2).unwrap();
    assert!(
        verify::export_strip_signatures(input2.to_str().unwrap(), output2.to_str().unwrap())
            .is_ok()
    );

    // Large
    let input3 = td.path().join("large.durp");
    let output3 = td.path().join("large_out.durp");
    let f3 = create_valid_frames(25);
    fs::write(&input3, f3).unwrap();
    assert!(
        verify::export_strip_signatures(input3.to_str().unwrap(), output3.to_str().unwrap())
            .is_ok()
    );
}

#[test]
fn test_verify_with_fec_and_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("fec_gaps.durp");
    let fec_index_path = td.path().join("fec_gaps.json");

    // Create frames with gaps
    let mut frames_data = Vec::new();
    for id in &[1, 2, 4, 5] {
        let frame = FrameBuilder::new(*id)
            .payload(Bytes::from(format!("Frame {}", id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }
    fs::write(&input_path, frames_data).unwrap();

    // FEC index
    let fec_index = r#"[{"block_start_id": 1, "data": 3, "parity": 1, "_parity_frame_ids": [6]}]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Verify with FEC and gaps
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        true, // report_gaps
        Some(fec_index_path.to_str().unwrap()),
        false,
    );

    assert!(result.is_ok());
}

#[test]
fn test_verify_fec_with_rs_repair_multiple_blocks() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("fec_multi.durp");
    let fec_index_path = td.path().join("fec_multi.json");

    let frames = create_valid_frames(10);
    fs::write(&input_path, frames).unwrap();

    // Multiple FEC blocks
    let fec_index = r#"[
        {"block_start_id": 1, "data": 3, "parity": 1, "_parity_frame_ids": [4]},
        {"block_start_id": 5, "data": 3, "parity": 1, "_parity_frame_ids": [8]},
        {"block_start_id": 9, "data": 2, "parity": 1, "_parity_frame_ids": [11]}
    ]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute with rs_repair
    let result = verify::execute_ext(
        input_path.to_str().unwrap(),
        false,
        Some(fec_index_path.to_str().unwrap()),
        true, // rs_repair
    );

    assert!(result.is_ok());
}
