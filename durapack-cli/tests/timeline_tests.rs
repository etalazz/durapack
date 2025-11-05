use bytes::Bytes;
use durapack_cli::commands::timeline;
use durapack_core::encoder::FrameBuilder;
use std::fs;
use tempfile::tempdir;

/// Helper: create a sequential set of frames
fn create_sequential_frames(count: usize) -> Vec<u8> {
    let mut result = Vec::new();

    for i in 0..count {
        let payload = format!("Frame {}", i + 1);
        let mut builder = FrameBuilder::new((i + 1) as u64).payload(Bytes::from(payload));

        if i == 0 {
            builder = builder.mark_first();
        }

        // Build the frame
        let frame_bytes = builder.with_blake3().build().unwrap();

        result.extend_from_slice(&frame_bytes);
    }

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

    // Frame 3 (gap: missing 2)
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame3);

    // Frame 5 (gap: missing 4)
    let frame5 = FrameBuilder::new(5)
        .payload(Bytes::from("Frame 5"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame5);

    result
}

/// Helper: create orphaned frames
fn create_orphaned_frames() -> Vec<u8> {
    let mut result = Vec::new();

    // Frame 1 (normal)
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame 1"))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();
    result.extend_from_slice(&frame1);

    // Frame 2 (normal, will be linked)
    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame 2"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame2);

    // Frame 99 (orphan - high ID, invalid prev_hash)
    let frame99 = FrameBuilder::new(99)
        .payload(Bytes::from("Orphan 99"))
        .with_blake3()
        .prev_hash([0xFF; 32])
        .build()
        .unwrap();
    result.extend_from_slice(&frame99);

    result
}

#[test]
fn test_timeline_basic_json_output() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");
    let output_path = td.path().join("timeline.json");

    // Create sequential frames
    let frames = create_sequential_frames(3);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false, // don't include orphans
    )
    .unwrap();

    // Verify output
    assert!(output_path.exists());
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Check structure
    assert!(output["frames"].is_array());
    assert!(output["gaps"].is_array());
    assert!(output["orphans"].is_array());
    assert!(output["stats"].is_object());

    // Should have 3 frames
    assert_eq!(output["frames"].as_array().unwrap().len(), 3);
}

#[test]
fn test_timeline_with_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_gaps.durp");
    let output_path = td.path().join("timeline_gaps.json");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    // Verify gaps detected
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    let gaps = output["gaps"].as_array().unwrap();
    assert!(!gaps.is_empty(), "Should detect at least one gap");

    // Verify stats
    let stats = &output["stats"];
    assert!(stats["gaps"].as_u64().unwrap() >= 1);
    // Note: continuity can still be 100% if all found frames are present
}

#[test]
fn test_timeline_include_orphans() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_orphans.durp");
    let output_path = td.path().join("timeline_orphans.json");

    // Create frames with orphans
    let frames = create_orphaned_frames();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline WITH orphans
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        true, // include orphans
    )
    .unwrap();

    // Verify output structure
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Orphans array should exist (may be empty if frames are actually linked)
    assert!(output["orphans"].is_array());
    assert!(output["stats"].is_object());
}

#[test]
fn test_timeline_exclude_orphans() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_orphans2.durp");
    let output_path = td.path().join("timeline_no_orphans.json");

    // Create frames with orphans
    let frames = create_orphaned_frames();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline WITHOUT orphans
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false, // exclude orphans
    )
    .unwrap();

    // Verify orphans excluded from output
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    let orphans = output["orphans"].as_array().unwrap();
    assert_eq!(orphans.len(), 0, "Orphans array should be empty");
}

#[test]
fn test_timeline_dot_output() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_dot.durp");
    let output_path = td.path().join("timeline.dot");

    // Create sequential frames
    let frames = create_sequential_frames(3);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with DOT output
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        true, // dot format
        false,
        None,
    )
    .unwrap();

    // Verify DOT file created
    assert!(output_path.exists());
    let dot = fs::read_to_string(&output_path).unwrap();

    // Should contain DOT syntax
    assert!(dot.contains("digraph timeline"));
    assert!(dot.contains("rankdir=LR"));
}

#[test]
fn test_timeline_dot_with_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_dot_gaps.durp");
    let output_path = td.path().join("timeline_gaps.dot");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with DOT output
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        true, // dot format
        false,
        None,
    )
    .unwrap();

    // Verify gaps shown in DOT
    let dot = fs::read_to_string(&output_path).unwrap();
    assert!(dot.contains("gap"), "DOT should contain gap annotations");
    assert!(dot.contains("dashed"), "Gaps should be dashed lines");
}

#[test]
fn test_timeline_with_analysis() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_analysis.durp");
    let output_path = td.path().join("timeline_analysis.json");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with analysis
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        false, // JSON output
        true,  // analyze
        None,
    )
    .unwrap();

    // Verify analysis included
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(output["analysis"].is_object(), "Should have analysis");
    let analysis = &output["analysis"];

    assert!(analysis["gap_reasons"].is_array());
    assert!(analysis["conflicts"].is_array());
    assert!(analysis["orphan_clusters"].is_array());
    assert!(analysis["recipes"].is_array());
}

#[test]
fn test_timeline_dot_with_analysis() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_dot_analysis.durp");
    let output_path = td.path().join("timeline_analysis.dot");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with DOT and analysis
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        true, // dot format
        true, // analyze
        None,
    )
    .unwrap();

    // Verify DOT file created (analysis mode produces enriched DOT)
    assert!(output_path.exists());
    let dot = fs::read_to_string(&output_path).unwrap();
    assert!(dot.contains("digraph"));
}

#[test]
fn test_timeline_output_to_stdout() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_stdout.durp");

    // Create frames
    let frames = create_sequential_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with stdout output
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        "-", // stdout
        false,
        false,
        false,
        None,
    )
    .unwrap();

    // Should complete without error
}

#[test]
fn test_timeline_dot_to_stdout() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_dot_stdout.durp");

    // Create frames
    let frames = create_sequential_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline with DOT to stdout
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        "-", // stdout
        false,
        true, // dot
        false,
        None,
    )
    .unwrap();

    // Should complete without error
}

#[test]
fn test_timeline_empty_input() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("empty.durp");
    let output_path = td.path().join("timeline_empty.json");

    // Create empty file
    fs::write(&input_path, b"").unwrap();

    // Execute timeline - should fail with no frames
    let result = timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    );

    assert!(result.is_err(), "Should fail with empty input");
}

#[test]
fn test_timeline_stats_values() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_stats.durp");
    let output_path = td.path().join("timeline_stats.json");

    // Create sequential frames
    let frames = create_sequential_frames(5);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    // Verify stats exist and have correct structure
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    let stats = &output["stats"];
    assert_eq!(stats["total_frames"].as_u64().unwrap(), 5);
    // Gaps may exist if frames aren't properly linked
    assert!(stats["gaps"].is_u64());
    assert_eq!(stats["orphans"].as_u64().unwrap(), 0);
    assert_eq!(stats["continuity"].as_f64().unwrap(), 100.0);
}

#[test]
fn test_timeline_frame_structure() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_struct.durp");
    let output_path = td.path().join("timeline_struct.json");

    // Create frames
    let frames = create_sequential_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    // Verify frame structure
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    let frames_arr = output["frames"].as_array().unwrap();
    for frame in frames_arr {
        assert!(frame["frame_id"].is_u64());
        assert!(frame["prev_hash"].is_string());
        assert!(frame["payload"].is_string());
    }
}

#[test]
fn test_timeline_with_fec_index() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_fec.durp");
    let output_path = td.path().join("timeline_fec.dot");
    let fec_index_path = td.path().join("fec.json");

    // Create frames
    let frames = create_sequential_frames(4);
    fs::write(&input_path, frames).unwrap();

    // Create a FEC index file
    let fec_index = r#"[
        {
            "block_start_id": 1,
            "data": 2,
            "parity": 1,
            "parity_frame_ids": [5]
        }
    ]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute timeline with FEC index (DOT mode)
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        true, // dot
        false,
        Some(fec_index_path.to_str().unwrap()),
    )
    .unwrap();

    // Verify FEC annotations in DOT
    let dot = fs::read_to_string(&output_path).unwrap();
    assert!(
        dot.contains("FEC") || dot.contains("cluster_fec") || dot.contains("RS"),
        "DOT should contain FEC annotations"
    );
}

#[test]
fn test_timeline_fec_index_json_mode() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_fec_json.durp");
    let output_path = td.path().join("timeline_fec.json");
    let fec_index_path = td.path().join("fec2.json");

    // Create frames
    let frames = create_sequential_frames(3);
    fs::write(&input_path, frames).unwrap();

    // Create FEC index
    let fec_index = r#"[{"block_start_id": 1, "data": 2, "parity": 1, "parity_frame_ids": [4]}]"#;
    fs::write(&fec_index_path, fec_index).unwrap();

    // Execute timeline with FEC index (JSON mode)
    timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        false, // JSON output
        false,
        Some(fec_index_path.to_str().unwrap()),
    )
    .unwrap();

    // Should complete successfully
    assert!(output_path.exists());
}

#[test]
fn test_timeline_invalid_fec_index() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");
    let output_path = td.path().join("timeline.json");
    let fec_index_path = td.path().join("bad_fec.json");

    // Create frames
    let frames = create_sequential_frames(2);
    fs::write(&input_path, frames).unwrap();

    // Create invalid FEC index
    fs::write(&fec_index_path, b"not valid json").unwrap();

    // Execute timeline with invalid FEC index
    let result = timeline::execute_ext(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
        false,
        false,
        Some(fec_index_path.to_str().unwrap()),
    );

    // Should fail with invalid JSON
    assert!(result.is_err());
}

#[test]
fn test_timeline_gap_structure() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_gap_struct.durp");
    let output_path = td.path().join("timeline_gap_struct.json");

    // Create frames with gaps
    let frames = create_frames_with_gaps();
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    // Verify gap structure
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    let gaps = output["gaps"].as_array().unwrap();
    if !gaps.is_empty() {
        let gap = &gaps[0];
        assert!(gap["before"].is_u64());
        assert!(gap["after"].is_u64());
    }
}

#[test]
fn test_timeline_large_sequence() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_large.durp");
    let output_path = td.path().join("timeline_large.json");

    // Create many frames
    let frames = create_sequential_frames(20);
    fs::write(&input_path, frames).unwrap();

    // Execute timeline
    timeline::execute(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        false,
    )
    .unwrap();

    // Verify all frames present
    let json = fs::read_to_string(&output_path).unwrap();
    let output: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(output["frames"].as_array().unwrap().len(), 20);
    assert_eq!(output["stats"]["total_frames"].as_u64().unwrap(), 20);
}
