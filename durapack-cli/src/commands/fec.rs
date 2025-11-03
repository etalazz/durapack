use anyhow::{bail, Context, Result};
use bytes::Bytes;
use std::fs;
use std::io::{self, Read, Write};
use tracing::info;

#[cfg(feature = "fec-rs")]
use durapack_core::{
    fec::{RedundancyEncoder, RsEncoder},
    linker::link_frames,
    scanner::scan_stream,
    types::Frame,
};

/// Post-facto parity injection:
/// - Reads an existing .durp file (or stdin)
/// - Scans frames, groups them into blocks of N, computes K parity frames per block (RS)
/// - Appends the parity frames to the original output (or a new file if --dry-run is used)
/// - Writes/updates a sidecar index JSON
pub fn inject_parity(
    input: &str,
    output: Option<&str>,
    n_data: usize,
    k_parity: usize,
    fec_index_out: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    #[cfg(not(feature = "fec-rs"))]
    {
        bail!("This build does not include RS FEC support. Rebuild with --features fec-rs");
    }

    #[cfg(feature = "fec-rs")]
    {
        info!(
            "Post-facto RS parity injection: N={}, K={} (input: {})",
            n_data, k_parity, input
        );

        let data = if input == "-" {
            let mut buf = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            buf
        } else {
            fs::read(input).with_context(|| format!("Failed to read input file: {}", input))?
        };

        let located = scan_stream(&data);
        if located.is_empty() {
            bail!("No frames found to protect");
        }
        // Build timeline to ensure order
        let frames: Vec<Frame> = located.into_iter().map(|lf| lf.frame).collect();
        let timeline = link_frames(frames);
        if timeline.frames.is_empty() {
            bail!("No ordered frames available");
        }

        // Sidecar entry
        #[derive(serde::Serialize)]
        struct FecIndexEntry {
            block_start_id: u64,
            data: usize,
            parity: usize,
            parity_frame_ids: Vec<u64>,
        }
        let mut fec_index: Vec<FecIndexEntry> = Vec::new();

        // Prepare output buffer (append mode if output omitted)
        let mut out_bytes: Vec<u8> = if dry_run {
            Vec::new()
        } else if let Some(out_path) = output {
            fs::read(out_path).unwrap_or_default()
        } else {
            data.clone()
        };

        let enc = RsEncoder::new(n_data, k_parity);
        let mut block: Vec<Frame> = Vec::with_capacity(n_data);
        let mut prev_hash = timeline
            .frames
            .last()
            .map(|f| f.compute_hash())
            .unwrap_or([0; 32]);
        let mut next_id = timeline
            .frames
            .last()
            .map(|f| f.header.frame_id + 1)
            .unwrap_or(1);

        for f in &timeline.frames {
            block.push(f.clone());
            if block.len() == n_data {
                let blocks = enc.encode_batch(&block, 0).with_context(|| {
                    format!(
                        "RS encode failed for block starting at {}",
                        block[0].header.frame_id
                    )
                })?;
                let parity_blocks = blocks.into_iter().skip(n_data);
                let mut parity_ids = Vec::new();
                for pb in parity_blocks {
                    // Build parity frame using the shard as payload
                    let mut b = durapack_core::encoder::FrameBuilder::new(next_id)
                        .prev_hash(prev_hash)
                        .payload(Bytes::from(pb.data));
                    // Use same trailer scheme as original frames best-effort: default CRC32C
                    b = b.with_crc32c();
                    let frame_struct = b.build_struct()?;
                    prev_hash = frame_struct.compute_hash();
                    let encoded = durapack_core::encoder::encode_frame_struct(&frame_struct)?;
                    if !dry_run {
                        out_bytes.extend_from_slice(&encoded);
                    }
                    parity_ids.push(next_id);
                    next_id += 1;
                }
                fec_index.push(FecIndexEntry {
                    block_start_id: block[0].header.frame_id,
                    data: n_data,
                    parity: k_parity,
                    parity_frame_ids: parity_ids,
                });
                block.clear();
            }
        }

        if !dry_run {
            // Write to output file (in-place append or new path)
            if let Some(out_path) = output {
                fs::write(out_path, &out_bytes)?;
                info!("Appended parity to: {}", out_path);
            } else if input != "-" {
                fs::write(input, &out_bytes)?;
                info!("Appended parity to input file in place");
            } else {
                io::stdout().write_all(&out_bytes)?;
            }
        }

        // Write sidecar index
        if let Some(sidecar_path) = fec_index_out {
            let json = serde_json::to_string_pretty(&fec_index)?;
            fs::write(sidecar_path, json)?;
            info!(
                "Wrote FEC index sidecar: {} (blocks: {})",
                sidecar_path,
                fec_index.len()
            );
        }

        Ok(())
    }
}
