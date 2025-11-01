use anyhow::{Context, Result};
use bytes::Bytes;
use durapack_core::encoder::FrameBuilder;
use serde_json::Value;
use std::fs;
use std::io::{self, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::ChunkStrategy;

#[allow(dead_code)]
pub fn execute(input: &str, output: &str, use_blake3: bool, start_id: u64) -> Result<()> {
    execute_ext(
        input,
        output,
        use_blake3,
        start_id,
        false,
        ChunkStrategy::Aggregate,
        None,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn execute_ext(
    input: &str,
    output: &str,
    use_blake3: bool,
    start_id: u64,
    jsonl: bool,
    chunk_strategy: ChunkStrategy,
    rate_limit: Option<u64>,
    progress: bool,
) -> Result<()> {
    info!("Packing data from {} to {}", input, output);

    // Read input
    let content = if input == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(input)
            .with_context(|| format!("Failed to read input file: {}", input))?
    };

    // Parse payloads according to mode
    let payloads: Vec<Value> = if jsonl || matches!(chunk_strategy, ChunkStrategy::Jsonl) {
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(serde_json::from_str::<Value>)
            .collect::<Result<_, _>>()
            .with_context(|| "Failed to parse JSONL input")?
    } else {
        serde_json::from_str::<Vec<Value>>(&content)
            .with_context(|| "Failed to parse JSON input (expected JSON array)")?
    };

    info!("Found {} payloads to pack", payloads.len());

    let mut output_data = Vec::new();
    let mut prev_hash = [0u8; 32];

    // Progress bar
    let pb = if progress {
        let pb = indicatif::ProgressBar::new(payloads.len() as u64);
        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "{spinner:.green} packing [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, est {eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let start_time = Instant::now();
    let mut bytes_written_total: u64 = 0;

    for (i, payload) in payloads.iter().enumerate() {
        let frame_id = start_id + i as u64;

        // Serialize payload to JSON bytes
        let payload_bytes = serde_json::to_vec(payload)
            .with_context(|| format!("Failed to serialize payload {}", frame_id))?;

        let mut builder = FrameBuilder::new(frame_id)
            .payload(Bytes::from(payload_bytes))
            .prev_hash(prev_hash);

        if i == 0 {
            builder = builder.mark_first();
        }

        if i == payloads.len() - 1 {
            builder = builder.mark_last();
        }

        if use_blake3 {
            builder = builder.with_blake3();
        } else {
            builder = builder.with_crc32c();
        }

        let frame_struct = builder
            .build_struct()
            .with_context(|| format!("Failed to build frame {}", frame_id))?;

        prev_hash = frame_struct.compute_hash();

        let encoded = durapack_core::encoder::encode_frame_struct(&frame_struct)
            .with_context(|| format!("Failed to encode frame {}", frame_id))?;

        output_data.extend_from_slice(&encoded);
        bytes_written_total += encoded.len() as u64;

        // Rate limiting (simple leaky bucket)
        if let Some(bps) = rate_limit {
            let elapsed = start_time.elapsed();
            let ideal = Duration::from_secs_f64(bytes_written_total as f64 / bps as f64);
            if ideal > elapsed {
                let sleep_dur = ideal - elapsed;
                debug!("rate-limit: sleeping {:?}", sleep_dur);
                sleep(sleep_dur);
            }
        }

        if let Some(pb) = &pb {
            pb.inc(1);
        }

        info!("Packed frame {} ({} bytes)", frame_id, encoded.len());
    }

    if let Some(pb) = &pb {
        pb.finish_with_message("done");
    }

    // Write output
    if output == "-" {
        io::stdout().write_all(&output_data)?;
        io::stdout().flush()?;
    } else {
        fs::write(output, &output_data)
            .with_context(|| format!("Failed to write output file: {}", output))?;
    }

    info!(
        "Successfully packed {} frames ({} bytes total)",
        payloads.len(),
        output_data.len()
    );

    Ok(())
}
