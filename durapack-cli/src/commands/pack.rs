use anyhow::{Context, Result};
use bytes::Bytes;
#[cfg(feature = "ed25519-signatures")]
use durapack_core::encoder::encode_frame_struct_signed;
use durapack_core::encoder::FrameBuilder;
#[cfg(feature = "fec-rs")]
use durapack_core::fec::{FecBlock, RedundancyEncoder, RsEncoder};
#[cfg(feature = "ed25519-signatures")]
use ed25519_dalek::SigningKey;
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
        None,
        None,
        None,
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
    fec_rs: Option<(usize, usize)>,
    fec_index_out: Option<&str>,
    sign_key_path: Option<&str>,
) -> Result<()> {
    info!("Packing data from {} to {}", input, output);

    #[cfg(feature = "ed25519-signatures")]
    let signing_key: Option<SigningKey> = if let Some(path) = sign_key_path {
        let bytes = fs::read(path).with_context(|| format!("Failed to read key: {}", path))?;
        let sk = SigningKey::from_bytes(
            bytes
                .as_slice()
                .try_into()
                .context("Key must be 32 bytes")?,
        );
        Some(sk)
    } else {
        None
    };
    #[cfg(not(feature = "ed25519-signatures"))]
    let _ = sign_key_path; // suppress unused

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

    // FEC sidecar structure
    #[derive(serde::Serialize)]
    struct FecIndexEntry {
        block_start_id: u64,
        data: usize,
        parity: usize,
        parity_frame_ids: Vec<u64>,
    }
    let mut fec_index: Vec<FecIndexEntry> = Vec::new();

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

    // Buffer frames in a block if FEC enabled
    let mut block_frames: Vec<durapack_core::types::Frame> = Vec::new();
    let mut next_frame_id = start_id;

    for payload in payloads.iter() {
        let frame_id = next_frame_id;

        // Serialize payload to JSON bytes
        let payload_bytes = serde_json::to_vec(payload)
            .with_context(|| format!("Failed to serialize payload {}", frame_id))?;

        let mut builder = FrameBuilder::new(frame_id)
            .payload(Bytes::from(payload_bytes))
            .prev_hash(prev_hash);

        if frame_id == start_id {
            builder = builder.mark_first();
        }

        let use_sig = sign_key_path.is_some();
        if use_sig {
            builder = builder.with_blake3_signature();
        } else if use_blake3 {
            builder = builder.with_blake3();
        } else {
            builder = builder.with_crc32c();
        }

        let frame_struct = builder
            .build_struct()
            .with_context(|| format!("Failed to build frame {}", frame_id))?;

        prev_hash = frame_struct.compute_hash();

        let encoded = {
            #[cfg(feature = "ed25519-signatures")]
            {
                if let Some(sk) = &signing_key {
                    encode_frame_struct_signed(&frame_struct, sk)
                        .with_context(|| format!("Failed to encode+sign frame {}", frame_id))?
                } else {
                    durapack_core::encoder::encode_frame_struct(&frame_struct)
                        .with_context(|| format!("Failed to encode frame {}", frame_id))?
                }
            }
            #[cfg(not(feature = "ed25519-signatures"))]
            {
                durapack_core::encoder::encode_frame_struct(&frame_struct)
                    .with_context(|| format!("Failed to encode frame {}", frame_id))?
            }
        };

        output_data.extend_from_slice(&encoded);
        bytes_written_total += encoded.len() as u64;

        // FEC block accumulation
        if let Some((n, k)) = fec_rs {
            block_frames.push(frame_struct.clone());
            if block_frames.len() == n {
                // Emit parity frames for this block
                #[cfg(feature = "fec-rs")]
                {
                    let enc = RsEncoder::new(n, k);
                    let blocks = enc.encode_batch(&block_frames, 0).context(format!(
                        "RS encode failed for block starting at {}",
                        frame_id + 1 - n as u64
                    ))?;
                    let parity_blocks: Vec<FecBlock> = blocks.into_iter().skip(n).collect();
                    let mut parity_ids = Vec::new();
                    for pb in parity_blocks {
                        // Wrap parity shard into a frame with IS_SUPERFRAME flag off; mark trailer
                        let mut b = FrameBuilder::new(next_frame_id + 1)
                            .payload(Bytes::from(pb.data))
                            .prev_hash(prev_hash);
                        if use_sig {
                            b = b.with_blake3_signature();
                        } else if use_blake3 {
                            b = b.with_blake3();
                        } else {
                            b = b.with_crc32c();
                        }
                        let parity_frame = b.build_struct()?;
                        prev_hash = parity_frame.compute_hash();
                        let enc_bytes = {
                            #[cfg(feature = "ed25519-signatures")]
                            {
                                if let Some(sk) = &signing_key {
                                    encode_frame_struct_signed(&parity_frame, sk)?
                                } else {
                                    durapack_core::encoder::encode_frame_struct(&parity_frame)?
                                }
                            }
                            #[cfg(not(feature = "ed25519-signatures"))]
                            {
                                durapack_core::encoder::encode_frame_struct(&parity_frame)?
                            }
                        };
                        output_data.extend_from_slice(&enc_bytes);
                        bytes_written_total += enc_bytes.len() as u64;
                        next_frame_id += 1;
                        parity_ids.push(parity_frame.header.frame_id);
                    }
                    fec_index.push(FecIndexEntry {
                        block_start_id: frame_id + 1 - n as u64,
                        data: n,
                        parity: k,
                        parity_frame_ids: parity_ids,
                    });
                }
                block_frames.clear();
            }
        }

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
        next_frame_id += 1;
    }

    // If there are leftover frames in a partial block, you can choose to emit parity or skip.
    // We skip parity for partial blocks by default.

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

    // Write FEC index sidecar if requested or implied
    if let Some((n, k)) = fec_rs {
        let sidecar_path = fec_index_out
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}.fec.json", output));
        let json = serde_json::to_string_pretty(&fec_index)?;
        fs::write(&sidecar_path, json)
            .with_context(|| format!("Failed to write FEC index: {}", sidecar_path))?;
        info!(
            "Wrote FEC index sidecar: {} (blocks: {})",
            sidecar_path,
            fec_index.len()
        );
        let _ = (n, k); // silence unused if compiled without feature
    }

    info!(
        "Successfully packed {} frames ({} bytes total)",
        payloads.len(),
        output_data.len()
    );

    Ok(())
}
