//! Example demonstrating recovery from damaged data

use bytes::Bytes;
use durapack_core::{
    encoder::FrameBuilder,
    scanner::scan_stream_with_stats,
    linker::link_frames,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Durapack Damaged Data Recovery Example\n");

    // Step 1: Create a clean stream with 10 frames
    println!("Step 1: Creating 10 frames...");
    let mut stream = Vec::new();
    let mut prev_hash = [0u8; 32];

    for i in 1..=10 {
        let payload = format!("Important data from sensor {}", i);

        let frame_struct = FrameBuilder::new(i)
            .payload(Bytes::from(payload))
            .prev_hash(prev_hash)
            .with_crc32c()
            .build_struct()?;

        prev_hash = frame_struct.compute_hash();

        let encoded = durapack_core::encoder::encode_frame_struct(&frame_struct)?;
        stream.extend_from_slice(&encoded);
    }

    let original_size = stream.len();
    println!("Created clean stream: {} bytes\n", original_size);

    // Step 2: Simulate damage - corrupt random sections
    println!("Step 2: Simulating damage...");

    // Corrupt bytes 500-700
    if stream.len() > 700 {
        stream[500..700].fill(0xFF);
        println!("Corrupted bytes 500-700");
    }

    // Corrupt bytes 1200-1300
    if stream.len() > 1300 {
        stream[1200..1300].fill(0x00);
        println!("Corrupted bytes 1200-1300");
    }

    // Delete bytes 2000-2200 (simulating physical damage)
    if stream.len() > 2200 {
        stream.drain(2000..2200);
        println!("Deleted bytes 2000-2200");
    }

    println!("Damaged stream: {} bytes\n", stream.len());

    // Step 3: Scan and recover
    println!("Step 3: Scanning damaged stream...");
    let (located_frames, stats) = scan_stream_with_stats(&stream);

    println!("Scan Results:");
    println!("  Bytes scanned:     {}", stats.bytes_scanned);
    println!("  Markers found:     {}", stats.markers_found);
    println!("  Valid frames:      {}", stats.frames_found);
    println!("  Decode failures:   {}", stats.decode_failures);
    println!("  Recovery rate:     {:.1}%\n", stats.recovery_rate());

    // Step 4: Reconstruct timeline
    println!("Step 4: Reconstructing timeline...");
    let frames: Vec<_> = located_frames.into_iter()
        .map(|lf| lf.frame)
        .collect();

    let timeline = link_frames(frames);

    println!("Timeline Results:");
    println!("  Ordered frames:    {}", timeline.frames.len());
    println!("  Detected gaps:     {}", timeline.gaps.len());
    println!("  Orphaned frames:   {}", timeline.orphans.len());

    if !timeline.gaps.is_empty() {
        println!("\nDetected gaps:");
        for gap in &timeline.gaps {
            println!("  Gap between frame {} and {}", gap.before, gap.after);
        }
    }

    println!("\nRecovered frames:");
    for frame in &timeline.frames {
        let payload_str = String::from_utf8_lossy(&frame.payload);
        println!("  Frame {}: {}", frame.header.frame_id, payload_str);
    }

    println!("\n✓ Successfully recovered {}/{} frames despite damage!",
        timeline.frames.len(), 10);

    Ok(())
}

