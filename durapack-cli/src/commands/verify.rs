use anyhow::{Context, Result};
use colored::*;
#[cfg(feature = "fec-rs")]
use durapack_core::fec::{RedundancyDecoder, RsDecoder};
use durapack_core::{
    linker::{link_frames, verify_backlinks},
    scanner::scan_stream,
};
use std::fs;
use std::io::{self, Read};
use tracing::{info, warn};

#[allow(dead_code)]
pub fn execute(input: &str, report_gaps: bool) -> Result<()> {
    execute_ext(input, report_gaps, None, false)
}

pub fn execute_ext(
    input: &str,
    report_gaps: bool,
    fec_index_path: Option<&str>,
    rs_repair: bool,
) -> Result<()> {
    info!("Verifying file: {}", input);

    // Read input file or stdin
    let data = if input == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        fs::read(input).with_context(|| format!("Failed to read input file: {}", input))?
    };

    // Scan for frames
    let located_frames = scan_stream(&data);

    if located_frames.is_empty() {
        println!("{} No valid frames found", "✗".red());
        return Ok(());
    }

    println!("\n=== Verification Results ===");
    println!("Total frames found: {}", located_frames.len());

    // Extract frames
    let frames: Vec<_> = located_frames.iter().map(|lf| lf.frame.clone()).collect();

    // Verify each frame individually
    let mut valid_frames = 0;
    let mut invalid_frames = 0;

    for frame in &frames {
        match frame.validate() {
            Ok(_) => valid_frames += 1,
            Err(e) => {
                invalid_frames += 1;
                warn!("Frame {} failed validation: {}", frame.header.frame_id, e);
            }
        }
    }

    println!("Valid frames:       {}", valid_frames.to_string().green());
    if invalid_frames > 0 {
        println!("Invalid frames:     {}", invalid_frames.to_string().red());
    } else {
        println!("Invalid frames:     {}", invalid_frames);
    }

    // Link frames and check back-links
    let timeline = link_frames(frames);

    println!("\n=== Timeline Analysis ===");
    println!("Ordered frames:     {}", timeline.frames.len());
    println!("Orphaned frames:    {}", timeline.orphans.len());
    println!("Detected gaps:      {}", timeline.gaps.len());

    let stats = timeline.stats();
    println!("Continuity:         {:.2}%", stats.continuity);

    // Verify back-links
    let backlink_errors = verify_backlinks(&timeline);

    println!("\n=== Back-link Verification ===");
    if backlink_errors.is_empty() {
        println!("{} All back-links valid", "✓".green());
    } else {
        println!(
            "{} {} back-link errors found",
            "✗".red(),
            backlink_errors.len()
        );
        for error in &backlink_errors {
            warn!("{}", error);
        }
    }

    // Optional: Load FEC index and attempt RS repair (report-only)
    if let (Some(path), true) = (fec_index_path, rs_repair) {
        #[derive(serde::Deserialize)]
        struct FecIndexEntry {
            block_start_id: u64,
            data: usize,
            parity: usize,
            _parity_frame_ids: Vec<u64>,
        }
        let idx_bytes =
            fs::read(path).with_context(|| format!("Failed to read FEC index: {}", path))?;
        let entries: Vec<FecIndexEntry> =
            serde_json::from_slice(&idx_bytes).with_context(|| "Invalid FEC index JSON")?;
        println!("\n=== FEC (RS) Repair Simulation ===");
        #[cfg(feature = "fec-rs")]
        {
            let dec = RsDecoder;
            for entry in entries {
                let have = entry.data; // assume we have all data frames here; a real flow would map IDs to present/missing
                let can = dec.can_reconstruct(have + entry.parity, entry.data);
                println!(
                    "Block start id {}: data={}, parity={} => reconstructable: {}",
                    entry.block_start_id,
                    entry.data,
                    entry.parity,
                    if can { "yes" } else { "no" }
                );
            }
        }
        #[cfg(not(feature = "fec-rs"))]
        {
            println!("RS repair requested, but durapack-core was built without `fec-rs` feature.");
        }
    }

    // Report gaps if requested
    if report_gaps && !timeline.gaps.is_empty() {
        println!("\n=== Detected Gaps ===");
        for gap in &timeline.gaps {
            println!("Gap between frame {} and frame {}", gap.before, gap.after);
        }
    }

    // Overall status
    println!("\n=== Summary ===");
    if invalid_frames == 0 && backlink_errors.is_empty() && timeline.gaps.is_empty() {
        println!("{} File is fully valid and complete", "✓".green());
    } else if invalid_frames > 0 {
        println!("{} File contains invalid frames", "✗".red());
    } else if !backlink_errors.is_empty() {
        println!("{} File has back-link integrity issues", "✗".red());
    } else {
        println!(
            "{} File is valid but has gaps in the sequence",
            "!".yellow()
        );
    }

    Ok(())
}
