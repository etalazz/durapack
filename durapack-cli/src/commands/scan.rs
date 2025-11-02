use anyhow::{Context, Result};
use durapack_core::linker::link_frames;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{self, Read, Write};
use tracing::info;
use bytes::Bytes;

#[derive(Serialize, Deserialize)]
struct RecoveredFrame {
    offset: usize,
    frame_id: u64,
    payload_len: u32,
    size: usize,
    payload: String,
    /// Optional confidence score [0.0, 1.0]
    confidence: f32,
}

#[derive(Serialize, Deserialize)]
struct GapRange {
    before: u64,
    after: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ScanRecord {
    Stats {
        bytes_scanned: usize,
        markers_found: usize,
        frames_found: usize,
        decode_failures: usize,
        bytes_recovered: usize,
        recovery_rate: f64,
    },
    Frame(RecoveredFrame),
    Gap(GapRange),
}

fn write_jsonl(mut out: impl Write, record: &ScanRecord) -> Result<()> {
    let line = serde_json::to_string(record)?;
    writeln!(out, "{}", line)?;
    Ok(())
}

#[allow(dead_code)]
pub fn execute(input: &str, output: Option<&str>, stats_only: bool) -> Result<()> {
    execute_ext(input, output, stats_only, false, None)
}

pub fn execute_ext(
    input: &str,
    output: Option<&str>,
    stats_only: bool,
    jsonl: bool,
    carve_payloads: Option<&str>,
) -> Result<()> {
    info!("Scanning: {}", input);

    // Read input (file or stdin)
    let data = if input == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        fs::read(input).with_context(|| format!("Failed to read input file: {}", input))?
    };

    info!("Input size: {} bytes", data.len());

    // Scan with statistics
    let (located_frames, stats) = if jsonl {
        // zero-copy scan still needs stats: compute using slice path for stats, emit frames from zero-copy path
        let zc = durapack_core::scanner::scan_stream_zero_copy(Bytes::from(data.clone()));
        let (_, st) = durapack_core::scanner::scan_stream_with_stats(&data);
        (zc, st)
    } else {
        durapack_core::scanner::scan_stream_with_stats(&data)
    };

    if jsonl {
        // Prepare writer (stdout or file)
        let mut writer: Box<dyn Write> = match output {
            Some(path) if path != "-" => Box::new(File::create(path)?),
            _ => Box::new(io::stdout()),
        };

        // Emit stats first
        write_jsonl(
            &mut writer,
            &ScanRecord::Stats {
                bytes_scanned: stats.bytes_scanned,
                markers_found: stats.markers_found,
                frames_found: stats.frames_found,
                decode_failures: stats.decode_failures,
                bytes_recovered: stats.bytes_recovered,
                recovery_rate: stats.recovery_rate(),
            },
        )?;

        // Compute gaps via timeline reconstruction
        let frames_only: Vec<_> = located_frames.iter().map(|lf| lf.frame.clone()).collect();
        let timeline = link_frames(frames_only);
        for gap in timeline.gaps {
            write_jsonl(
                &mut writer,
                &ScanRecord::Gap(GapRange {
                    before: gap.before,
                    after: gap.after,
                }),
            )?;
        }

        // Emit each frame as JSONL
        for lf in &located_frames {
            let payload_str = String::from_utf8_lossy(&lf.frame.payload).to_string();
            // Naive confidence heuristic: shorter frames and exact decodes get higher score
            let confidence = 1.0_f32;
            let rec = ScanRecord::Frame(RecoveredFrame {
                offset: lf.offset,
                frame_id: lf.frame.header.frame_id,
                payload_len: lf.frame.header.payload_len,
                size: lf.size,
                payload: payload_str,
                confidence,
            });
            write_jsonl(&mut writer, &rec)?;
        }

        // Carve payloads if requested
        if let Some(pattern) = carve_payloads {
            let stream_id = 0usize; // single-stream file
            for lf in &located_frames {
                let path = pattern
                    .replace("{stream}", &stream_id.to_string())
                    .replace("{frame}", &lf.frame.header.frame_id.to_string());
                fs::write(&path, &lf.frame.payload)
                    .with_context(|| format!("Failed to write carved payload: {}", path))?;
            }
        }

        return Ok(());
    }

    // Non-JSONL: print human-readable summary and optional JSON file
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
                confidence: 1.0,
            }
        })
        .collect();

    if let Some(output_path) = output {
        // Write to JSON file or stdout
        let json = serde_json::to_string_pretty(&recovered)
            .with_context(|| "Failed to serialize recovered frames")?;

        if output_path == "-" {
            println!("{}", json);
        } else {
            fs::write(output_path, json)
                .with_context(|| format!("Failed to write output file: {}", output_path))?;
            info!("Recovered frames written to: {}", output_path);
        }
    } else {
        // Print to stdout
        println!("=== Recovered Frames ===");
        for frame in &recovered {
            println!(
                "Frame {} @ offset {}: {} bytes",
                frame.frame_id, frame.offset, frame.size
            );
        }
    }

    // Carve payloads if requested (non-JSONL path)
    if let Some(pattern) = carve_payloads {
        let stream_id = 0usize; // single-stream file
        for lf in &located_frames {
            let path = pattern
                .replace("{stream}", &stream_id.to_string())
                .replace("{frame}", &lf.frame.header.frame_id.to_string());
            fs::write(&path, &lf.frame.payload)
                .with_context(|| format!("Failed to write carved payload: {}", path))?;
        }
    }

    Ok(())
}
