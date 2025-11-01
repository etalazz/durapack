use anyhow::{Context, Result};
use durapack_core::scanner::{scan_stream_with_stats, LocatedFrame};
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::info;

#[derive(Serialize, Deserialize)]
struct RecoveredFrame {
    offset: usize,
    frame_id: u64,
    payload_len: u32,
    size: usize,
    payload: String,
}

pub fn execute(input: &str, output: Option<&str>, stats_only: bool) -> Result<()> {
    info!("Scanning file: {}", input);

    // Read input file
    let data = fs::read(input)
        .with_context(|| format!("Failed to read input file: {}", input))?;

    info!("File size: {} bytes", data.len());

    // Scan the stream
    let (located_frames, stats) = scan_stream_with_stats(&data);

    // Print statistics
    println!("\n=== Scan Results ===");
    println!("Bytes scanned:     {} bytes", stats.bytes_scanned);
    println!("Markers found:     {}", stats.markers_found);
    println!("Valid frames:      {}", stats.frames_found);
    println!("Decode failures:   {}", stats.decode_failures);
    println!("Bytes recovered:   {} bytes", stats.bytes_recovered);
    println!("Recovery rate:     {:.2}%", stats.recovery_rate());
    println!();

    if stats_only {
        return Ok(());
    }

    // Convert to JSON-friendly format
    let recovered: Vec<RecoveredFrame> = located_frames
        .iter()
        .map(|lf| {
            let payload_str = String::from_utf8_lossy(&lf.frame.payload).to_string();
            RecoveredFrame {
                offset: lf.offset,
                frame_id: lf.frame.header.frame_id,
                payload_len: lf.frame.header.payload_len,
                size: lf.size,
                payload: payload_str,
            }
        })
        .collect();

    if let Some(output_path) = output {
        // Write to JSON file
        let json = serde_json::to_string_pretty(&recovered)
            .with_context(|| "Failed to serialize recovered frames")?;

        fs::write(output_path, json)
            .with_context(|| format!("Failed to write output file: {}", output_path))?;

        info!("Recovered frames written to: {}", output_path);
    } else {
        // Print to stdout
        println!("=== Recovered Frames ===");
        for frame in &recovered {
            println!("Frame {} @ offset {}: {} bytes",
                frame.frame_id, frame.offset, frame.size);
        }
    }

    Ok(())
}

