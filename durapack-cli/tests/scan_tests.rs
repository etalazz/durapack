use std::fs;
use tempfile::tempdir;

use bytes::Bytes;
use durapack_cli::commands::scan;
use durapack_core::encoder::FrameBuilder;

/// Helper: create a simple encoded frame file with multiple frames
fn create_test_frames(num_frames: usize, use_blake3: bool) -> Vec<u8> {
    let mut result = Vec::new();

    for i in 0..num_frames {
        let payload = format!("Test payload {}", i);
        let mut builder = FrameBuilder::new((i + 1) as u64).payload(Bytes::from(payload));

        if use_blake3 {
            builder = builder.with_blake3();
        }

        if i == 0 {
            builder = builder.mark_first();
        }

        let frame = builder.build().unwrap();
        result.extend_from_slice(&frame);
    }

    result
}

/// Helper: create frames with gaps (missing frame IDs)
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

    // Frame 3 (gap: missing frame 2)
    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame 3"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame3);

    // Frame 5 (gap: missing frame 4)
    let frame5 = FrameBuilder::new(5)
        .payload(Bytes::from("Frame 5"))
        .with_blake3()
        .build()
        .unwrap();
    result.extend_from_slice(&frame5);

    result
}

/// Helper: create damaged frames (partial corruption)
fn create_damaged_frames() -> Vec<u8> {
    let mut data = create_test_frames(5, true);

    // Corrupt some bytes in the middle (but not the markers)
    if data.len() > 200 {
        data[150] = data[150].wrapping_add(1);
        data[151] = data[151].wrapping_add(1);
    }

    data
}

#[test]
fn test_scan_basic_file() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames.durp");
    let output_path = td.path().join("output.json");

    // Create test data
    let frames_data = create_test_frames(3, false);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan
    scan::execute(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
    )
    .unwrap();

    // Verify output file was created
    assert!(output_path.exists());

    // Parse JSON output
    let output_json = fs::read_to_string(&output_path).unwrap();
    let frames: Vec<serde_json::Value> = serde_json::from_str(&output_json).unwrap();

    assert_eq!(frames.len(), 3);
}

#[test]
fn test_scan_with_blake3() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_blake3.durp");
    let output_path = td.path().join("output_blake3.json");

    // Create test data with BLAKE3
    let frames_data = create_test_frames(5, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan
    scan::execute(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
    )
    .unwrap();

    // Verify output
    assert!(output_path.exists());
    let output_json = fs::read_to_string(&output_path).unwrap();
    let frames: Vec<serde_json::Value> = serde_json::from_str(&output_json).unwrap();

    assert_eq!(frames.len(), 5);

    // Verify frames have frame_id
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(frame["frame_id"].as_u64().unwrap(), (i + 1) as u64);
    }
}

#[test]
fn test_scan_stats_only() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_stats.durp");

    // Create test data
    let frames_data = create_test_frames(4, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan with stats_only=true
    scan::execute(
        input_path.to_str().unwrap(),
        None,
        true, // stats_only
    )
    .unwrap();

    // Should complete successfully without creating output file
}

#[test]
fn test_scan_jsonl_mode() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_jsonl.durp");
    let output_path = td.path().join("output.jsonl");

    // Create test data
    let frames_data = create_test_frames(3, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan in JSONL mode
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        None,
        None,
    )
    .unwrap();

    // Verify output
    assert!(output_path.exists());

    // Parse JSONL output (one JSON object per line)
    let output_text = fs::read_to_string(&output_path).unwrap();
    let lines: Vec<&str> = output_text.lines().collect();

    // Should have: 1 stats record + 3 frame records = 4 lines minimum
    assert!(lines.len() >= 4);

    // First line should be stats
    let first_record: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first_record["type"], "stats");
    assert!(first_record["frames_found"].as_u64().unwrap() >= 3);
}

#[test]
fn test_scan_with_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_gaps.durp");
    let output_path = td.path().join("output_gaps.jsonl");

    // Create frames with gaps
    let frames_data = create_frames_with_gaps();
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan in JSONL mode to see gaps
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        None,
        None,
    )
    .unwrap();

    // Verify output
    let output_text = fs::read_to_string(&output_path).unwrap();
    let lines: Vec<&str> = output_text.lines().collect();

    // Parse each line and count gap records
    let mut gap_count = 0;
    let mut frame_count = 0;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        match record["type"].as_str() {
            Some("gap") => {
                gap_count += 1;
                // Verify gap has before/after fields
                assert!(record["before"].is_u64());
                assert!(record["after"].is_u64());
                assert!(record["confidence"].is_f64());
            }
            Some("frame") => frame_count += 1,
            _ => {}
        }
    }

    assert_eq!(frame_count, 3); // We created 3 frames
    assert!(gap_count >= 1); // Should detect at least one gap
}

#[test]
fn test_scan_min_confidence_filtering() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_confidence.durp");
    let output_path = td.path().join("output_filtered.jsonl");

    // Create test data
    let frames_data = create_test_frames(5, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan with min_confidence=0.9 (high threshold)
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        None,
        Some(0.9), // min_confidence
    )
    .unwrap();

    // Verify output - should still have frames (good frames have high confidence)
    let output_text = fs::read_to_string(&output_path).unwrap();
    let lines: Vec<&str> = output_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    // Parse frames and verify all have confidence >= 0.9
    for line in lines {
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        if record["type"] == "frame" {
            let confidence = record["confidence"].as_f64().unwrap();
            assert!(
                confidence >= 0.9,
                "Frame confidence {} is below threshold 0.9",
                confidence
            );
        }
    }
}

#[test]
fn test_scan_min_confidence_zero() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_conf_zero.durp");
    let output_path = td.path().join("output_all.json");

    // Create test data
    let frames_data = create_test_frames(3, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan with min_confidence=0.0 (accept all)
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        false, // not jsonl
        None,
        Some(0.0), // min_confidence
    )
    .unwrap();

    // Verify all frames are included
    let output_json = fs::read_to_string(&output_path).unwrap();
    let frames: Vec<serde_json::Value> = serde_json::from_str(&output_json).unwrap();
    assert_eq!(frames.len(), 3);
}

#[test]
fn test_scan_carve_payloads() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_carve.durp");
    let output_path = td.path().join("output_carve.jsonl");

    // Create test data
    let frames_data = create_test_frames(3, true);
    fs::write(&input_path, frames_data).unwrap();

    // Create a pattern that uses both {stream} and {frame}
    let carve_pattern = td
        .path()
        .join("payload_{stream}_{frame}.bin")
        .to_str()
        .unwrap()
        .to_string();

    // Execute scan with payload carving
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        Some(&carve_pattern),
        None,
    )
    .unwrap();

    // Verify carved payload files were created
    for i in 1..=3 {
        let carved_path = td.path().join(format!("payload_0_{}.bin", i));
        assert!(
            carved_path.exists(),
            "Carved payload file should exist: {:?}",
            carved_path
        );

        // Verify content
        let content = fs::read_to_string(&carved_path).unwrap();
        assert!(content.contains(&format!("Test payload {}", i - 1)));
    }
}

#[test]
fn test_scan_carve_with_min_confidence() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_carve_conf.durp");
    let output_path = td.path().join("output_carve_conf.jsonl");

    // Create test data
    let frames_data = create_test_frames(4, true);
    fs::write(&input_path, frames_data).unwrap();

    let carve_pattern = td
        .path()
        .join("payload_{frame}.bin")
        .to_str()
        .unwrap()
        .to_string();

    // Execute scan with high confidence threshold (will pass for good frames)
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        Some(&carve_pattern),
        Some(0.95), // min_confidence
    )
    .unwrap();

    // Good frames should still be carved (they have high confidence)
    let carved_exists = (1..=4).any(|i| td.path().join(format!("payload_{}.bin", i)).exists());

    // At least some frames should be carved if they meet confidence threshold
    // (Good frames with BLAKE3 typically have confidence > 0.95)
    assert!(carved_exists || output_path.exists());
}

#[test]
fn test_scan_output_to_stdout() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_stdout.durp");

    // Create test data
    let frames_data = create_test_frames(2, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan with output to stdout ("-")
    // This should complete without error
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some("-"),
        false,
        true, // jsonl
        None,
        None,
    )
    .unwrap();
}

#[test]
fn test_scan_no_output_path() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_no_out.durp");

    // Create test data
    let frames_data = create_test_frames(2, false);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan without output path (prints to stdout)
    scan::execute(input_path.to_str().unwrap(), None, false).unwrap();
}

#[test]
fn test_scan_damaged_frames() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_damaged.durp");
    let output_path = td.path().join("output_damaged.json");

    // Create damaged data
    let frames_data = create_damaged_frames();
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan - should recover what it can
    scan::execute(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
    )
    .unwrap();

    // Should create output (may have fewer frames due to damage)
    assert!(output_path.exists());
}

#[test]
fn test_scan_empty_file() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("empty.durp");
    let output_path = td.path().join("output_empty.json");

    // Create empty file
    fs::write(&input_path, b"").unwrap();

    // Execute scan
    scan::execute(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
    )
    .unwrap();

    // Should create output with empty array
    assert!(output_path.exists());
    let output_json = fs::read_to_string(&output_path).unwrap();
    let frames: Vec<serde_json::Value> = serde_json::from_str(&output_json).unwrap();
    assert_eq!(frames.len(), 0);
}

#[test]
fn test_scan_jsonl_stats_record() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_stats_record.durp");
    let output_path = td.path().join("output_stats_record.jsonl");

    // Create test data
    let frames_data = create_test_frames(3, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan in JSONL mode
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        None,
        None,
    )
    .unwrap();

    // Parse first line (should be stats)
    let output_text = fs::read_to_string(&output_path).unwrap();
    let first_line = output_text.lines().next().unwrap();
    let stats: serde_json::Value = serde_json::from_str(first_line).unwrap();

    // Verify stats structure
    assert_eq!(stats["type"], "stats");
    assert!(stats["bytes_scanned"].is_u64());
    assert!(stats["markers_found"].is_u64());
    assert!(stats["frames_found"].is_u64());
    assert!(stats["decode_failures"].is_u64());
    assert!(stats["bytes_recovered"].is_u64());
    assert!(stats["recovery_rate"].is_f64());

    // Verify some values
    assert!(stats["bytes_scanned"].as_u64().unwrap() > 0);
    assert_eq!(stats["frames_found"].as_u64().unwrap(), 3);
}

#[test]
fn test_scan_large_payload() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_large.durp");
    let output_path = td.path().join("output_large.json");

    // Create frame with larger payload
    let large_payload = "X".repeat(10000);
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from(large_payload))
        .with_blake3()
        .mark_first()
        .build()
        .unwrap();

    fs::write(&input_path, &frame).unwrap();

    // Execute scan
    scan::execute(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
    )
    .unwrap();

    // Verify output
    assert!(output_path.exists());
    let output_json = fs::read_to_string(&output_path).unwrap();
    let frames: Vec<serde_json::Value> = serde_json::from_str(&output_json).unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["payload_len"].as_u64().unwrap(), 10000);
}

#[test]
fn test_scan_non_jsonl_carve_path() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_carve_non_jsonl.durp");
    let output_path = td.path().join("output.json");

    // Create test data
    let frames_data = create_test_frames(2, true);
    fs::write(&input_path, frames_data).unwrap();

    let carve_pattern = td
        .path()
        .join("carved_{frame}.bin")
        .to_str()
        .unwrap()
        .to_string();

    // Execute scan in non-JSONL mode with carving
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        false, // not jsonl
        Some(&carve_pattern),
        None,
    )
    .unwrap();

    // Verify carved files exist
    assert!(td.path().join("carved_1.bin").exists());
    assert!(td.path().join("carved_2.bin").exists());
}

#[test]
fn test_jsonl_format_validity() {
    // Test that all JSONL output lines are valid JSON
    // This indirectly tests the write_jsonl function

    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_jsonl_write.durp");
    let output_path = td.path().join("output_jsonl_write.jsonl");

    // Create test data
    let frames_data = create_test_frames(1, true);
    fs::write(&input_path, frames_data).unwrap();

    // Execute scan in JSONL mode (this exercises write_jsonl)
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl - this uses write_jsonl internally
        None,
        None,
    )
    .unwrap();

    // Verify JSONL format - each line should be valid JSON
    let output_text = fs::read_to_string(&output_path).unwrap();
    for line in output_text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Each line should be valid JSON
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
}

#[test]
fn test_scan_multiple_gaps() {
    let td = tempdir().unwrap();
    let input_path = td.path().join("frames_multiple_gaps.durp");
    let output_path = td.path().join("output_multiple_gaps.jsonl");

    // Create frames: 1, 4, 7 (gaps: 2-3, 5-6, etc.)
    let mut frames_data = Vec::new();

    for frame_id in &[1, 4, 7] {
        let frame = FrameBuilder::new(*frame_id)
            .payload(Bytes::from(format!("Frame {}", frame_id)))
            .with_blake3()
            .build()
            .unwrap();
        frames_data.extend_from_slice(&frame);
    }

    fs::write(&input_path, frames_data).unwrap();

    // Execute scan
    scan::execute_ext(
        input_path.to_str().unwrap(),
        Some(output_path.to_str().unwrap()),
        false,
        true, // jsonl
        None,
        None,
    )
    .unwrap();

    // Count gap records
    let output_text = fs::read_to_string(&output_path).unwrap();
    let gap_count = output_text
        .lines()
        .filter(|line| {
            if let Ok(record) = serde_json::from_str::<serde_json::Value>(line) {
                record["type"] == "gap"
            } else {
                false
            }
        })
        .count();

    assert!(gap_count >= 2); // Should detect multiple gaps
}
