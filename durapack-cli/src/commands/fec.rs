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

        // Prepare output buffer seeded with original data (append parity afterwards)
        let mut out_bytes: Vec<u8> = if dry_run { Vec::new() } else { data.clone() };

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

#[cfg(test)]
mod tests {
    use super::inject_parity;
    use std::fs;
    use tempfile::tempdir;

    // Helpers to build a small .durp buffer with N frames
    #[cfg(feature = "fec-rs")]
    fn make_durp_with_frames(count: u64) -> Vec<u8> {
        use bytes::Bytes;
        use durapack_core::encoder::{encode_frame_struct, FrameBuilder};
        let mut bytes_out = Vec::new();
        let mut prev_hash = [0u8; 32];
        for id in 1..=count {
            let mut b = FrameBuilder::new(id).payload(Bytes::from(format!("{{\"id\":{}}}", id)));
            if id == 1 {
                b = b.mark_first();
            }
            // default CRC32C to keep parity default consistent
            b = b.with_crc32c();
            b = b.prev_hash(prev_hash);
            let f = b.build_struct().unwrap();
            prev_hash = f.compute_hash();
            let enc = encode_frame_struct(&f).unwrap();
            bytes_out.extend_from_slice(&enc);
        }
        bytes_out
    }

    #[cfg(feature = "fec-rs")]
    #[test]
    fn test_inject_parity_dry_run_writes_sidecar_no_change() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input.durp");
        let sidecar_path = dir.path().join("input.durp.fec.json");

        let data = make_durp_with_frames(4);
        fs::write(&input_path, &data).unwrap();
        let orig_len = fs::metadata(&input_path).unwrap().len();

        // Dry-run should not change the input file
        inject_parity(
            input_path.to_str().unwrap(),
            None,
            2,
            1,
            Some(sidecar_path.to_str().unwrap()),
            true,
        )
        .unwrap();

        let new_len = fs::metadata(&input_path).unwrap().len();
        assert_eq!(orig_len, new_len, "dry-run must not modify input file");

        // Sidecar exists and has 2 blocks (frames 1..2 and 3..4), 1 parity each
        let side = fs::read_to_string(&sidecar_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&side).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["data"], serde_json::json!(2));
        assert_eq!(arr[0]["parity"], serde_json::json!(1));
        assert_eq!(arr[1]["data"], serde_json::json!(2));
        assert_eq!(arr[1]["parity"], serde_json::json!(1));
        assert_eq!(arr[0]["block_start_id"], serde_json::json!(1));
        assert_eq!(arr[1]["block_start_id"], serde_json::json!(3));
        assert_eq!(arr[0]["parity_frame_ids"].as_array().unwrap().len(), 1);
        assert_eq!(arr[1]["parity_frame_ids"].as_array().unwrap().len(), 1);
    }

    #[cfg(feature = "fec-rs")]
    #[test]
    fn test_inject_parity_appends_to_new_output_file() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input2.durp");
        let out_path = dir.path().join("out_with_parity.durp");
        let sidecar_path = dir.path().join("out_with_parity.durp.fec.json");

        let data = make_durp_with_frames(6);
        fs::write(&input_path, &data).unwrap();
        let orig_len = fs::metadata(&input_path).unwrap().len();

        inject_parity(
            input_path.to_str().unwrap(),
            Some(out_path.to_str().unwrap()),
            2,
            1,
            Some(sidecar_path.to_str().unwrap()),
            false,
        )
        .unwrap();

        let new_len = fs::metadata(&out_path).unwrap().len();
        assert!(
            new_len > orig_len,
            "output should include appended parity frames"
        );

        // 6 frames -> 3 blocks of 2 with 1 parity each
        let side = fs::read_to_string(&sidecar_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&side).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["block_start_id"], serde_json::json!(1));
        assert_eq!(arr[1]["block_start_id"], serde_json::json!(3));
        assert_eq!(arr[2]["block_start_id"], serde_json::json!(5));
        for e in arr {
            assert_eq!(e["parity_frame_ids"].as_array().unwrap().len(), 1);
        }
    }

    #[cfg(feature = "fec-rs")]
    #[test]
    fn test_inject_parity_appends_in_place() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("input3.durp");
        let data = make_durp_with_frames(2);
        fs::write(&input_path, &data).unwrap();
        let orig_len = fs::metadata(&input_path).unwrap().len();

        inject_parity(input_path.to_str().unwrap(), None, 2, 1, None, false).unwrap();

        let new_len = fs::metadata(&input_path).unwrap().len();
        assert!(
            new_len > orig_len,
            "input should grow after in-place append"
        );
    }

    #[cfg(feature = "fec-rs")]
    #[test]
    fn test_inject_parity_ignores_leftover_frames() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("leftover.durp");
        let sidecar_path = dir.path().join("leftover.durp.fec.json");

        // 3 frames with N=2 yields 1 protected block, 1 leftover frame ignored
        let data = make_durp_with_frames(3);
        fs::write(&input_path, &data).unwrap();

        inject_parity(
            input_path.to_str().unwrap(),
            None,
            2,
            1,
            Some(sidecar_path.to_str().unwrap()),
            true,
        )
        .unwrap();

        let side = fs::read_to_string(&sidecar_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&side).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1, "only full N-sized blocks receive parity");
        assert_eq!(arr[0]["block_start_id"], serde_json::json!(1));
    }

    #[cfg(feature = "fec-rs")]
    #[test]
    fn test_inject_parity_no_frames_error() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("empty.durp");
        fs::write(&input_path, &[] as &[u8]).unwrap();
        let err = inject_parity(input_path.to_str().unwrap(), None, 2, 1, None, true)
            .expect_err("expected error for empty input");
        let msg = format!("{}", err);
        assert!(msg.contains("No frames found to protect"));
    }

    #[cfg(not(feature = "fec-rs"))]
    #[test]
    fn test_inject_parity_requires_feature() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("noop.durp");
        fs::write(&input_path, &[] as &[u8]).unwrap();
        let err = inject_parity(input_path.to_str().unwrap(), None, 2, 1, None, true)
            .expect_err("expected feature-gated error");
        let msg = format!("{}", err);
        assert!(msg.contains("Rebuild with --features fec-rs"));
    }
}
