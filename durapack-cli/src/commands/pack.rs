use anyhow::{Context, Result};
use bytes::Bytes;
use durapack_core::encoder::FrameBuilder;
use serde_json::Value;
use std::fs;
use tracing::info;

pub fn execute(input: &str, output: &str, use_blake3: bool, start_id: u64) -> Result<()> {
    info!("Packing data from {} to {}", input, output);

    // Read input JSON
    let content = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input))?;

    let payloads: Vec<Value> = serde_json::from_str(&content)
        .with_context(|| "Failed to parse JSON input")?;

    info!("Found {} payloads to pack", payloads.len());

    let mut output_data = Vec::new();
    let mut prev_hash = [0u8; 32];

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

        let frame_struct = builder.build_struct()
            .with_context(|| format!("Failed to build frame {}", frame_id))?;

        prev_hash = frame_struct.compute_hash();

        let encoded = durapack_core::encoder::encode_frame_struct(&frame_struct)
            .with_context(|| format!("Failed to encode frame {}", frame_id))?;

        output_data.extend_from_slice(&encoded);

        info!("Packed frame {} ({} bytes)", frame_id, encoded.len());
    }

    // Write output file
    fs::write(output, &output_data)
        .with_context(|| format!("Failed to write output file: {}", output))?;

    info!(
        "Successfully packed {} frames ({} bytes total)",
        payloads.len(),
        output_data.len()
    );

    Ok(())
}

