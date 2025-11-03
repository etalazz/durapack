use anyhow::{bail, Context, Result};
use durapack_core::{
    linker::{analyze_located_frames, link_frames, report_to_dot, RecoveryRecipe},
    scanner::scan_stream,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    analysis: Option<AnalysisExtras>,
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

#[derive(Serialize, Deserialize)]
struct GapReasonJson {
    before: u64,
    after: u64,
    reason: String,
}

#[derive(Serialize, Deserialize)]
struct ConflictJson {
    at: u64,
    contenders: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
struct OrphanClusterJson {
    ids: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum RecipeJson {
    InsertParityFrame {
        between: (u64, u64),
        reason: String,
    },
    RewindOffset {
        near_frame: u64,
        by_bytes: isize,
        reason: String,
    },
}

#[derive(Serialize, Deserialize)]
struct AnalysisExtras {
    gap_reasons: Vec<GapReasonJson>,
    conflicts: Vec<ConflictJson>,
    orphan_clusters: Vec<OrphanClusterJson>,
    recipes: Vec<RecipeJson>,
}

#[allow(dead_code)]
pub fn execute(input: &str, output: &str, include_orphans: bool) -> Result<()> {
    execute_ext(input, output, include_orphans, false, false, None)
}

pub fn execute_ext(
    input: &str,
    output: &str,
    include_orphans: bool,
    dot: bool,
    analyze: bool,
    fec_index_path: Option<&str>,
) -> Result<()> {
    info!("Reconstructing timeline from: {}", input);

    // Read input ("-" for stdin)
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
        bail!("No valid frames found in input");
    }

    info!("Found {} frames", located_frames.len());

    // Optional FEC index
    #[derive(Serialize, Deserialize, Clone)]
    struct FecIndexEntry {
        block_start_id: u64,
        data: usize,
        parity: usize,
        parity_frame_ids: Vec<u64>,
    }
    let mut fec_index: Option<Vec<FecIndexEntry>> = None;
    if let Some(path) = fec_index_path {
        let idx = fs::read(path).with_context(|| format!("Failed to read FEC index: {}", path))?;
        let entries: Vec<FecIndexEntry> =
            serde_json::from_slice(&idx).with_context(|| "Invalid FEC index JSON")?;
        fec_index = Some(entries);
    }

    // Extract frames and link (basic timeline always available)
    let frames: Vec<_> = located_frames.iter().map(|lf| lf.frame.clone()).collect();
    let timeline = link_frames(frames);

    info!(
        "Timeline: {} ordered, {} gaps, {} orphans",
        timeline.frames.len(),
        timeline.gaps.len(),
        timeline.orphans.len()
    );

    if dot {
        // Emit Graphviz DOT representation; if analyze, use richer report
        let mut out: Box<dyn Write> = if output == "-" {
            Box::new(io::stdout())
        } else {
            Box::new(fs::File::create(output)?)
        };

        if analyze {
            let report = analyze_located_frames(located_frames);
            let dot_str = report_to_dot(&report);
            write!(&mut out, "{}", dot_str)?;
        } else {
            // Basic DOT (backwards compatible)
            writeln!(&mut out, "digraph timeline {{")?;
            writeln!(&mut out, "  rankdir=LR;")?;
            for f in &timeline.frames {
                writeln!(
                    &mut out,
                    "  {} [label=\"{}\"];",
                    f.header.frame_id, f.header.frame_id
                )?;
            }
            for win in timeline.frames.windows(2) {
                let a = win[0].header.frame_id;
                let b = win[1].header.frame_id;
                writeln!(&mut out, "  {} -> {};", a, b)?;
            }
            for g in &timeline.gaps {
                writeln!(
                    &mut out,
                    "  {} -> {} [style=dashed, color=red, label=\"gap\"];",
                    g.before, g.after
                )?;
            }
            if let Some(idx) = &fec_index {
                writeln!(&mut out, "  // FEC parity annotations")?;
                for e in idx {
                    writeln!(
                        &mut out,
                        "  subgraph cluster_fec_{} {{ label=\"RS {}+{}\"; style=dotted; }}",
                        e.block_start_id, e.data, e.parity
                    )?;
                }
            }
            writeln!(&mut out, "}}")?;
        }

        return Ok(());
    }

    // JSON output path
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

    let mut analysis = if analyze {
        let report = analyze_located_frames(located_frames);
        let gap_reasons: Vec<GapReasonJson> = report
            .gap_details
            .iter()
            .map(|gd| GapReasonJson {
                before: gd.gap.before,
                after: gd.gap.after,
                reason: format!("{:?}", gd.reason),
            })
            .collect();
        let conflicts: Vec<ConflictJson> = report
            .conflicts
            .iter()
            .map(|c| ConflictJson {
                at: c.at,
                contenders: c.contenders.clone(),
            })
            .collect();
        let orphan_clusters: Vec<OrphanClusterJson> = report
            .orphan_clusters
            .iter()
            .map(|c| OrphanClusterJson { ids: c.ids.clone() })
            .collect();
        let recipes: Vec<RecipeJson> = report
            .recipes
            .iter()
            .map(|r| match r {
                RecoveryRecipe::InsertParityFrame { between, reason } => {
                    RecipeJson::InsertParityFrame {
                        between: *between,
                        reason: reason.clone(),
                    }
                }
                RecoveryRecipe::RewindOffset {
                    near_frame,
                    by_bytes,
                    reason,
                } => RecipeJson::RewindOffset {
                    near_frame: *near_frame,
                    by_bytes: *by_bytes,
                    reason: reason.clone(),
                },
            })
            .collect();
        Some(AnalysisExtras {
            gap_reasons,
            conflicts,
            orphan_clusters,
            recipes,
        })
    } else {
        None
    };

    if analysis.is_none() {
        if let Some(_idx) = fec_index {
            // Attach a minimal analysis object to carry FEC info
            analysis = Some(AnalysisExtras {
                gap_reasons: Vec::new(),
                conflicts: Vec::new(),
                orphan_clusters: Vec::new(),
                recipes: Vec::new(),
            });
            // We don't embed the full index here to keep payload small; DOT path annotates clusters.
        }
    }

    let output_obj = TimelineOutput {
        frames: frames_output,
        gaps: gaps_output,
        orphans: orphans_output,
        stats: stats_output,
        analysis,
    };

    // Write JSON output
    if output == "-" {
        let out = serde_json::to_string_pretty(&output_obj)?;
        io::stdout().write_all(out.as_bytes())?;
    } else {
        let out = serde_json::to_string_pretty(&output_obj)?;
        fs::write(output, out)?;
    }

    println!("\n=== Timeline Reconstruction ===");
    println!("Ordered frames:  {}", output_obj.frames.len());
    println!("Gaps detected:   {}", output_obj.gaps.len());
    println!("Orphaned frames: {}", output_obj.orphans.len());
    println!("Continuity:      {:.2}%", output_obj.stats.continuity);
    if output != "-" {
        println!("\nTimeline written to: {}", output);
    }

    Ok(())
}
