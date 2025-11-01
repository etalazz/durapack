use anyhow::{Context, Result};
use durapack_core::{linker::link_frames, scanner::scan_stream};
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::info;

#[derive(Serialize, Deserialize)]
struct TimelineFrame {
    frame_id: u64,
    prev_hash: String,
    payload: String,
}

#[derive(Serialize, Deserialize)]
struct TimelineOutput {
    frames: Vec<TimelineFrame>,
    gaps: Vec<TimelineGap>,
    orphans: Vec<TimelineFrame>,
    stats: TimelineStats,
}

#[derive(Serialize, Deserialize)]
struct TimelineGap {
    before: u64,
    after: u64,
}

#[derive(Serialize, Deserialize)]
struct TimelineStats {
    total_frames: usize,
    gaps: usize,
    orphans: usize,
    continuity: f64,
}

pub fn execute(input: &str, output: &str, include_orphans: bool) -> Result<()> {
    info!("Reconstructing timeline from: {}", input);

    // Read input file
    let data = fs::read(input).with_context(|| format!("Failed to read input file: {}", input))?;

    // Scan for frames
    let located_frames = scan_stream(&data);

    if located_frames.is_empty() {
        anyhow::bail!("No valid frames found in input file");
    }

    info!("Found {} frames", located_frames.len());

    // Extract frames
    let frames: Vec<_> = located_frames.into_iter().map(|lf| lf.frame).collect();

    // Link frames into timeline
    let timeline = link_frames(frames);

    info!(
        "Timeline: {} ordered, {} gaps, {} orphans",
        timeline.frames.len(),
        timeline.gaps.len(),
        timeline.orphans.len()
    );

    // Convert to output format
    let frames_output: Vec<TimelineFrame> = timeline
        .frames
        .iter()
        .map(|f| TimelineFrame {
            frame_id: f.header.frame_id,
            prev_hash: hex::encode(f.header.prev_hash),
            payload: String::from_utf8_lossy(&f.payload).to_string(),
        })
        .collect();

    let orphans_output: Vec<TimelineFrame> = if include_orphans {
        timeline
            .orphans
            .iter()
            .map(|f| TimelineFrame {
                frame_id: f.header.frame_id,
                prev_hash: hex::encode(f.header.prev_hash),
                payload: String::from_utf8_lossy(&f.payload).to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };

    let gaps_output: Vec<TimelineGap> = timeline
        .gaps
        .iter()
        .map(|g| TimelineGap {
            before: g.before,
            after: g.after,
        })
        .collect();

    let stats = timeline.stats();
    let stats_output = TimelineStats {
        total_frames: stats.total_frames,
        gaps: stats.gaps,
        orphans: stats.orphans,
        continuity: stats.continuity,
    };

    let output_data = TimelineOutput {
        frames: frames_output,
        gaps: gaps_output,
        orphans: orphans_output,
        stats: stats_output,
    };

    // Write output
    let json = serde_json::to_string_pretty(&output_data)
        .with_context(|| "Failed to serialize timeline")?;

    fs::write(output, json).with_context(|| format!("Failed to write output file: {}", output))?;

    println!("\n=== Timeline Reconstruction ===");
    println!("Ordered frames:  {}", output_data.frames.len());
    println!("Gaps detected:   {}", output_data.gaps.len());
    println!("Orphaned frames: {}", output_data.orphans.len());
    println!("Continuity:      {:.2}%", output_data.stats.continuity);
    println!("\nTimeline written to: {}", output);

    Ok(())
}
