//! Test vectors for Durapack specification
//!
//! This module generates and validates test vectors for all corruption scenarios
//! defined in the formal specification.

use durapack_core::{
    decoder::decode_frame_from_bytes,
    encoder::FrameBuilder,
    scanner::{scan_stream, scan_stream_with_stats},
    linker::link_frames,
    types::Frame,
};
use bytes::Bytes;
use std::fs;
use std::path::Path;

/// Test vector directory
const TEST_VECTOR_DIR: &str = "test_vectors";

/// Generate all test vectors
pub fn generate_all_test_vectors() -> std::io::Result<()> {
    let dir = Path::new(TEST_VECTOR_DIR);
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }

    // Generate each test vector
    generate_minimal_frame()?;
    generate_frame_with_crc32c()?;
    generate_frame_with_blake3()?;
    generate_linked_sequence()?;
    generate_truncated_frame()?;
    generate_bit_flip_error()?;
    generate_burst_error()?;
    generate_inserted_garbage()?;
    generate_deleted_bytes()?;
    generate_swapped_frames()?;
    generate_wrong_checksum()?;
    generate_duplicate_frames()?;
    generate_reordered_frames()?;

    println!("✓ All test vectors generated in {}/", TEST_VECTOR_DIR);
    Ok(())
}

/// 1. Minimal Frame (No Trailer)
fn generate_minimal_frame() -> std::io::Result<()> {
    let frame = FrameBuilder::new(1)
        .payload(Bytes::new())
        .mark_first()
        .build()
        .unwrap();

    fs::write(
        format!("{}/01_minimal_frame.durp", TEST_VECTOR_DIR),
        &frame,
    )?;

    // Generate documentation
    let doc = format!(
        "# Test Vector 1: Minimal Frame

**File:** 01_minimal_frame.durp
**Size:** {} bytes
**Description:** Smallest valid frame with no payload and no trailer

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: Empty (0 bytes)
- Flags: 0x04 (IS_FIRST)
- Trailer: None

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- Timeline: Single frame, no gaps

## Hex Dump
```
{}
```
",
        frame.len(),
        hex::encode(&frame)
    );

    fs::write(
        format!("{}/01_minimal_frame.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 2. Frame with CRC32C
fn generate_frame_with_crc32c() -> std::io::Result<()> {
    let payload = Bytes::from("Hello, Durapack!");
    let frame = FrameBuilder::new(1)
        .payload(payload.clone())
        .mark_first()
        .with_crc32c()
        .build()
        .unwrap();

    fs::write(
        format!("{}/02_frame_with_crc32c.durp", TEST_VECTOR_DIR),
        &frame,
    )?;

    let doc = format!(
        "# Test Vector 2: Frame with CRC32C

**File:** 02_frame_with_crc32c.durp
**Size:** {} bytes
**Description:** Frame with short payload and CRC32C integrity check

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: \"{}\" ({} bytes)
- Flags: 0x05 (IS_FIRST | HAS_CRC32C)
- Trailer: CRC32C (4 bytes)

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- CRC verification: MUST pass

## Hex Dump
```
{}
```
",
        frame.len(),
        String::from_utf8_lossy(&payload),
        payload.len(),
        hex::encode(&frame)
    );

    fs::write(
        format!("{}/02_frame_with_crc32c.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 3. Frame with BLAKE3
fn generate_frame_with_blake3() -> std::io::Result<()> {
    let payload = Bytes::from("Frame with BLAKE3 hash");
    let frame = FrameBuilder::new(1)
        .payload(payload.clone())
        .mark_first()
        .with_blake3()
        .build()
        .unwrap();

    fs::write(
        format!("{}/03_frame_with_blake3.durp", TEST_VECTOR_DIR),
        &frame,
    )?;

    let doc = format!(
        "# Test Vector 3: Frame with BLAKE3

**File:** 03_frame_with_blake3.durp
**Size:** {} bytes
**Description:** Frame with payload and BLAKE3 cryptographic hash

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: \"{}\" ({} bytes)
- Flags: 0x06 (IS_FIRST | HAS_BLAKE3)
- Trailer: BLAKE3 (32 bytes)

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- BLAKE3 verification: MUST pass

## Hex Dump
```
{}
```
",
        frame.len(),
        String::from_utf8_lossy(&payload),
        payload.len(),
        hex::encode(&frame)
    );

    fs::write(
        format!("{}/03_frame_with_blake3.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 4. Linked Sequence (3 frames)
fn generate_linked_sequence() -> std::io::Result<()> {
    let frame1_struct = FrameBuilder::new(1)
        .payload(Bytes::from("First frame"))
        .mark_first()
        .with_blake3()
        .build_struct()
        .unwrap();

    let hash1 = frame1_struct.compute_hash();

    let frame2_struct = FrameBuilder::new(2)
        .payload(Bytes::from("Second frame"))
        .prev_hash(hash1)
        .with_blake3()
        .build_struct()
        .unwrap();

    let hash2 = frame2_struct.compute_hash();

    let frame3_struct = FrameBuilder::new(3)
        .payload(Bytes::from("Third frame"))
        .prev_hash(hash2)
        .mark_last()
        .with_blake3()
        .build_struct()
        .unwrap();

    let frame1 = durapack_core::encoder::encode_frame_struct(&frame1_struct).unwrap();
    let frame2 = durapack_core::encoder::encode_frame_struct(&frame2_struct).unwrap();
    let frame3 = durapack_core::encoder::encode_frame_struct(&frame3_struct).unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(&frame2);
    stream.extend_from_slice(&frame3);

    fs::write(
        format!("{}/04_linked_sequence.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 4: Linked Sequence

**File:** 04_linked_sequence.durp
**Size:** {} bytes
**Description:** Three frames linked via BLAKE3 hash chain

## Frame Details
### Frame 1
- Frame ID: 1
- Prev Hash: All zeros
- Payload: \"First frame\" (11 bytes)
- Flags: 0x06 (IS_FIRST | HAS_BLAKE3)

### Frame 2
- Frame ID: 2
- Prev Hash: BLAKE3(Frame 1)
- Payload: \"Second frame\" (12 bytes)
- Flags: 0x02 (HAS_BLAKE3)

### Frame 3
- Frame ID: 3
- Prev Hash: BLAKE3(Frame 2)
- Payload: \"Third frame\" (11 bytes)
- Flags: 0x0A (IS_LAST | HAS_BLAKE3)

## Expected Behavior
- Scanner: MUST find exactly 3 frames
- Timeline: Complete chain 1 → 2 → 3
- No gaps or orphans
- All back-links MUST verify

## Hex Dump
```
{}
```
",
        stream.len(),
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/04_linked_sequence.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 5. Truncated Frame
fn generate_truncated_frame() -> std::io::Result<()> {
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("This frame will be truncated"))
        .with_crc32c()
        .build()
        .unwrap();

    // Truncate after 30 bytes (middle of header)
    let truncated = &frame[..30];

    fs::write(
        format!("{}/05_truncated_frame.durp", TEST_VECTOR_DIR),
        truncated,
    )?;

    let doc = format!(
        "# Test Vector 5: Truncated Frame

**File:** 05_truncated_frame.durp
**Size:** {} bytes (truncated from {} bytes)
**Description:** Frame truncated in the middle of the header

## Corruption Details
- Original size: {} bytes
- Truncated at: byte 30
- Corruption type: TRUNCATION
- Severity: Frame unrecoverable

## Expected Behavior
- Scanner: MUST detect incomplete frame
- Decoder: MUST reject (insufficient data)
- Recovery: No valid frames found

## Hex Dump
```
{}
```
",
        truncated.len(),
        frame.len(),
        frame.len(),
        hex::encode(truncated)
    );

    fs::write(
        format!("{}/05_truncated_frame.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 6. Bit Flip Error
fn generate_bit_flip_error() -> std::io::Result<()> {
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("Data with single bit error"))
        .with_crc32c()
        .build()
        .unwrap();

    let mut corrupted = frame.to_vec();
    // Flip a bit in the payload (byte 60)
    corrupted[60] ^= 0x01;

    fs::write(
        format!("{}/06_bit_flip_error.durp", TEST_VECTOR_DIR),
        &corrupted,
    )?;

    let doc = format!(
        "# Test Vector 6: Bit Flip Error

**File:** 06_bit_flip_error.durp
**Size:** {} bytes
**Description:** Single bit flipped in payload

## Corruption Details
- Corruption type: BIT FLIP
- Location: Byte 60 (in payload)
- Bit flipped: 0x01 (LSB)
- Severity: Frame detectable but invalid

## Expected Behavior
- Scanner: MUST find frame via marker
- Decoder: MUST reject (CRC32C mismatch)
- Error: ChecksumMismatch
- Recovery: Frame lost

## Hex Dump
```
{}
```

## Diff from Clean
Original byte 60: 0x{:02X}
Corrupted byte 60: 0x{:02X}
",
        corrupted.len(),
        hex::encode(&corrupted),
        frame[60],
        corrupted[60]
    );

    fs::write(
        format!("{}/06_bit_flip_error.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 7. Burst Error
fn generate_burst_error() -> std::io::Result<()> {
    // Create 3 frames
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame before burst"))
        .with_blake3()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame in burst zone"))
        .with_blake3()
        .build()
        .unwrap();

    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame after burst"))
        .with_blake3()
        .build()
        .unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    let burst_start = stream.len();
    stream.extend_from_slice(&frame2);
    let burst_end = stream.len();
    stream.extend_from_slice(&frame3);

    // Corrupt 50 bytes in the middle (destroying frame2)
    for i in burst_start..(burst_start + 50).min(burst_end) {
        stream[i] = 0xFF;
    }

    fs::write(
        format!("{}/07_burst_error.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 7: Burst Error

**File:** 07_burst_error.durp
**Size:** {} bytes
**Description:** 50-byte burst error destroying middle frame

## Corruption Details
- Corruption type: BURST ERROR
- Location: Bytes {} to {}
- Length: 50 bytes
- Affected: Frame 2 (completely destroyed)
- Severity: One frame lost

## Expected Behavior
- Scanner: MUST find 2 valid frames (1 and 3)
- Frame 2: Unrecoverable
- Timeline: Gap detected between frame 1 and 3
- Recovery rate: 66.7% (2/3 frames)

## Hex Dump
```
{}
```
",
        stream.len(),
        burst_start,
        burst_start + 50,
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/07_burst_error.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 8. Inserted Garbage
fn generate_inserted_garbage() -> std::io::Result<()> {
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame before garbage"))
        .with_crc32c()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame after garbage"))
        .with_crc32c()
        .build()
        .unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);

    // Insert 100 bytes of garbage
    let garbage = vec![0xAA; 100];
    stream.extend_from_slice(&garbage);

    stream.extend_from_slice(&frame2);

    fs::write(
        format!("{}/08_inserted_garbage.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 8: Inserted Garbage

**File:** 08_inserted_garbage.durp
**Size:** {} bytes
**Description:** 100 bytes of garbage inserted between frames

## Corruption Details
- Corruption type: INSERTION
- Location: Between frame 1 and frame 2
- Inserted: 100 bytes (0xAA repeated)
- Severity: Minimal (frames still recoverable)

## Expected Behavior
- Scanner: MUST find 2 valid frames
- Scanner: MUST skip garbage via marker search
- Timeline: 2 frames, may detect gap (depending on IDs)
- Recovery rate: 100%

## Hex Dump (first 200 bytes)
```
{}
```
",
        stream.len(),
        hex::encode(&stream[..200.min(stream.len())])
    );

    fs::write(
        format!("{}/08_inserted_garbage.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 9. Deleted Bytes
fn generate_deleted_bytes() -> std::io::Result<()> {
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("Frame before deletion"))
        .with_blake3()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Frame with deletion"))
        .with_blake3()
        .build()
        .unwrap();

    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Frame after deletion"))
        .with_blake3()
        .build()
        .unwrap();

    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    let delete_start = stream.len() + 10; // Delete 30 bytes from frame2
    stream.extend_from_slice(&frame2);
    stream.extend_from_slice(&frame3);

    // Delete 30 bytes from frame2
    stream.drain(delete_start..(delete_start + 30));

    fs::write(
        format!("{}/09_deleted_bytes.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 9: Deleted Bytes

**File:** 09_deleted_bytes.durp
**Size:** {} bytes
**Description:** 30 bytes deleted from middle frame

## Corruption Details
- Corruption type: DELETION
- Location: Frame 2 (bytes 10-40 removed)
- Deleted: 30 bytes
- Severity: One frame lost, desynchronization

## Expected Behavior
- Scanner: MUST resynchronize at next marker
- Frame 2: Unrecoverable (damaged)
- Frame 3: Recoverable after resync
- Recovery rate: 66.7% (2/3 frames)

## Hex Dump
```
{}
```
",
        stream.len(),
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/09_deleted_bytes.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 10. Swapped Frames
fn generate_swapped_frames() -> std::io::Result<()> {
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("First frame"))
        .mark_first()
        .with_blake3()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Second frame"))
        .with_blake3()
        .build()
        .unwrap();

    let frame3 = FrameBuilder::new(3)
        .payload(Bytes::from("Third frame"))
        .mark_last()
        .with_blake3()
        .build()
        .unwrap();

    // Write in wrong order: 3, 1, 2
    let mut stream = Vec::new();
    stream.extend_from_slice(&frame3);
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(&frame2);

    fs::write(
        format!("{}/10_swapped_frames.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 10: Swapped Frames

**File:** 10_swapped_frames.durp
**Size:** {} bytes
**Description:** Frames stored in wrong order (3, 1, 2 instead of 1, 2, 3)

## Corruption Details
- Corruption type: REORDERING
- Physical order: Frame 3, Frame 1, Frame 2
- Logical order: Frame 1, Frame 2, Frame 3
- Severity: None (design feature)

## Expected Behavior
- Scanner: MUST find all 3 frames
- Timeline: MUST reconstruct correct order (1 → 2 → 3)
- No data loss
- Recovery rate: 100%

## Hex Dump
```
{}
```
",
        stream.len(),
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/10_swapped_frames.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 11. Wrong Checksum
fn generate_wrong_checksum() -> std::io::Result<()> {
    let frame = FrameBuilder::new(1)
        .payload(Bytes::from("Frame with wrong checksum"))
        .with_crc32c()
        .build()
        .unwrap();

    let mut corrupted = frame.to_vec();
    // Corrupt the CRC32C trailer (last 4 bytes)
    let len = corrupted.len();
    corrupted[len - 4] ^= 0xFF;
    corrupted[len - 3] ^= 0xFF;

    fs::write(
        format!("{}/11_wrong_checksum.durp", TEST_VECTOR_DIR),
        &corrupted,
    )?;

    let doc = format!(
        "# Test Vector 11: Wrong Checksum

**File:** 11_wrong_checksum.durp
**Size:** {} bytes
**Description:** Frame with intentionally corrupted CRC32C trailer

## Corruption Details
- Corruption type: WRONG CHECKSUM
- Location: CRC32C trailer (last 4 bytes)
- Corruption: XOR with 0xFFFF0000
- Severity: Frame detectable but invalid

## Expected Behavior
- Scanner: MUST find frame via marker
- Decoder: MUST reject (ChecksumMismatch)
- Error message: MUST indicate expected vs actual checksum
- Recovery: Frame lost

## Hex Dump
```
{}
```
",
        corrupted.len(),
        hex::encode(&corrupted)
    );

    fs::write(
        format!("{}/11_wrong_checksum.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 12. Duplicate Frames
fn generate_duplicate_frames() -> std::io::Result<()> {
    let frame1 = FrameBuilder::new(1)
        .payload(Bytes::from("First frame"))
        .with_crc32c()
        .build()
        .unwrap();

    let frame2 = FrameBuilder::new(2)
        .payload(Bytes::from("Second frame"))
        .with_crc32c()
        .build()
        .unwrap();

    // Write: frame1, frame2, frame1 (duplicate)
    let mut stream = Vec::new();
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(&frame2);
    stream.extend_from_slice(&frame1); // Duplicate

    fs::write(
        format!("{}/12_duplicate_frames.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 12: Duplicate Frames

**File:** 12_duplicate_frames.durp
**Size:** {} bytes
**Description:** Frame 1 appears twice in the stream

## Corruption Details
- Corruption type: DUPLICATION
- Duplicated: Frame 1
- Occurrences: 2 (byte 0 and byte {})
- Severity: Minor (deduplication required)

## Expected Behavior
- Scanner: MUST find 3 frame instances
- Timeline: SHOULD keep first occurrence of frame 1
- Warning: Duplicate frame ID detected
- Effective frames: 2 (frame 1 and frame 2)

## Hex Dump
```
{}
```
",
        stream.len(),
        frame1.len() + frame2.len(),
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/12_duplicate_frames.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

/// 13. Reordered Frames
fn generate_reordered_frames() -> std::io::Result<()> {
    let frame1_struct = FrameBuilder::new(1)
        .payload(Bytes::from("First frame"))
        .mark_first()
        .with_blake3()
        .build_struct()
        .unwrap();

    let hash1 = frame1_struct.compute_hash();

    let frame2_struct = FrameBuilder::new(2)
        .payload(Bytes::from("Second frame"))
        .prev_hash(hash1)
        .with_blake3()
        .build_struct()
        .unwrap();

    let hash2 = frame2_struct.compute_hash();

    let frame3_struct = FrameBuilder::new(3)
        .payload(Bytes::from("Third frame"))
        .prev_hash(hash2)
        .with_blake3()
        .build_struct()
        .unwrap();

    let hash3 = frame3_struct.compute_hash();

    let frame4_struct = FrameBuilder::new(4)
        .payload(Bytes::from("Fourth frame"))
        .prev_hash(hash3)
        .mark_last()
        .with_blake3()
        .build_struct()
        .unwrap();

    let frame1 = durapack_core::encoder::encode_frame_struct(&frame1_struct).unwrap();
    let frame2 = durapack_core::encoder::encode_frame_struct(&frame2_struct).unwrap();
    let frame3 = durapack_core::encoder::encode_frame_struct(&frame3_struct).unwrap();
    let frame4 = durapack_core::encoder::encode_frame_struct(&frame4_struct).unwrap();

    // Write in scrambled order: 3, 1, 4, 2
    let mut stream = Vec::new();
    stream.extend_from_slice(&frame3);
    stream.extend_from_slice(&frame1);
    stream.extend_from_slice(&frame4);
    stream.extend_from_slice(&frame2);

    fs::write(
        format!("{}/13_reordered_frames.durp", TEST_VECTOR_DIR),
        &stream,
    )?;

    let doc = format!(
        "# Test Vector 13: Reordered Frames with Hash Links

**File:** 13_reordered_frames.durp
**Size:** {} bytes
**Description:** 4 linked frames stored in scrambled order

## Frame Details
- Logical order: 1 → 2 → 3 → 4
- Physical order: 3, 1, 4, 2
- All frames have prev_hash links

## Corruption Details
- Corruption type: REORDERING (intentional)
- Severity: None (timeline reconstruction handles this)

## Expected Behavior
- Scanner: MUST find all 4 frames
- Timeline: MUST reconstruct correct order via hash links
- Result: 1 → 2 → 3 → 4
- No gaps, no orphans
- Recovery rate: 100%

## Hex Dump
```
{}
```
",
        stream.len(),
        hex::encode(&stream)
    );

    fs::write(
        format!("{}/13_reordered_frames.md", TEST_VECTOR_DIR),
        doc,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_all_vectors() {
        generate_all_test_vectors().unwrap();
    }

    #[test]
    fn test_validate_minimal_frame() {
        let data = fs::read("test_vectors/01_minimal_frame.durp").unwrap();
        let frame = decode_frame_from_bytes(&data).unwrap();
        assert_eq!(frame.header.frame_id, 1);
        assert_eq!(frame.payload.len(), 0);
    }

    #[test]
    fn test_validate_linked_sequence() {
        let data = fs::read("test_vectors/04_linked_sequence.durp").unwrap();
        let located = scan_stream(&data);
        assert_eq!(located.len(), 3);

        let frames: Vec<_> = located.into_iter().map(|lf| lf.frame).collect();
        let timeline = link_frames(frames);

        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.gaps.len(), 0);
        assert_eq!(timeline.orphans.len(), 0);
    }

    #[test]
    fn test_validate_truncated_frame() {
        let data = fs::read("test_vectors/05_truncated_frame.durp").unwrap();
        let result = decode_frame_from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_bit_flip_error() {
        let data = fs::read("test_vectors/06_bit_flip_error.durp").unwrap();
        let result = decode_frame_from_bytes(&data);
        assert!(result.is_err());
        // Should be ChecksumMismatch error
    }

    #[test]
    fn test_validate_burst_error() {
        let data = fs::read("test_vectors/07_burst_error.durp").unwrap();
        let located = scan_stream(&data);
        // Should find 2 frames (1 and 3), frame 2 is destroyed
        assert_eq!(located.len(), 2);
    }

    #[test]
    fn test_validate_inserted_garbage() {
        let data = fs::read("test_vectors/08_inserted_garbage.durp").unwrap();
        let located = scan_stream(&data);
        // Should find both frames despite garbage
        assert_eq!(located.len(), 2);
    }

    #[test]
    fn test_validate_swapped_frames() {
        let data = fs::read("test_vectors/10_swapped_frames.durp").unwrap();
        let located = scan_stream(&data);
        assert_eq!(located.len(), 3);

        // Frames should be found in physical order: 3, 1, 2
        assert_eq!(located[0].frame.header.frame_id, 3);
        assert_eq!(located[1].frame.header.frame_id, 1);
        assert_eq!(located[2].frame.header.frame_id, 2);
    }

    #[test]
    fn test_validate_reordered_with_links() {
        let data = fs::read("test_vectors/13_reordered_frames.durp").unwrap();
        let located = scan_stream(&data);
        assert_eq!(located.len(), 4);

        let frames: Vec<_> = located.into_iter().map(|lf| lf.frame).collect();
        let timeline = link_frames(frames);

        // Timeline should reconstruct correct order
        assert_eq!(timeline.frames.len(), 4);
        assert_eq!(timeline.frames[0].header.frame_id, 1);
        assert_eq!(timeline.frames[1].header.frame_id, 2);
        assert_eq!(timeline.frames[2].header.frame_id, 3);
        assert_eq!(timeline.frames[3].header.frame_id, 4);
    }
}

