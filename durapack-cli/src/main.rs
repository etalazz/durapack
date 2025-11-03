mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ChunkStrategy {
    /// One JSON value per line
    Jsonl,
    /// Aggregate all inputs into a single JSON array
    Aggregate,
}

#[derive(Parser)]
#[command(name = "durapack")]
#[command(about = "Durapack - Self-locating framing format for hostile media", long_about = None)]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack data into Durapack frames
    Pack {
        /// Input JSON/JSONL file ("-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output file for packed frames ("-" for stdout)
        #[arg(short, long)]
        output: String,

        /// Use BLAKE3 trailer instead of CRC32C
        #[arg(long)]
        blake3: bool,

        /// Starting frame ID
        #[arg(long, default_value = "1")]
        start_id: u64,

        /// Interpret input as JSONL stream
        #[arg(long, default_value_t = false)]
        jsonl: bool,

        /// Chunking strategy when reading stdin/JSONL
        #[arg(long, value_enum, default_value_t = ChunkStrategy::Aggregate)]
        chunk_strategy: ChunkStrategy,

        /// Apply rate limit while writing (bytes/sec)
        #[arg(long)]
        rate_limit: Option<u64>,

        /// Show a progress bar while packing
        #[arg(long, default_value_t = false)]
        progress: bool,

        /// FEC: number of data frames per block (Reed–Solomon). Requires durapack-core with `fec-rs` feature.
        #[arg(long, requires = "fec_rs_parity")]
        fec_rs_data: Option<usize>,

        /// FEC: number of parity frames per block (Reed–Solomon). Requires durapack-core with `fec-rs` feature.
        #[arg(long, requires = "fec_rs_data")]
        fec_rs_parity: Option<usize>,

        /// Path to write FEC sidecar index (JSON). Defaults to <output>.fec.json when FEC is enabled.
        #[arg(long)]
        fec_index_out: Option<String>,
    },

    /// Post-facto parity injection: compute RS parity over existing file and append parity frames
    Fec {
        /// Input .durp file ("-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output file to write (defaults to appending to input if omitted)
        #[arg(short, long)]
        output: Option<String>,

        /// RS data shard count (N). Requires build with `--features fec-rs`.
        #[arg(long, requires = "k_parity")]
        n_data: usize,

        /// RS parity shard count (K). Requires build with `--features fec-rs`.
        #[arg(long, requires = "n_data")]
        k_parity: usize,

        /// Write/update FEC sidecar index JSON at this path
        #[arg(long)]
        fec_index_out: Option<String>,

        /// Dry-run (compute but do not write frames); still writes sidecar if requested
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Scan damaged file and recover frames
    Scan {
        /// Input file to scan ("-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output JSON file ("-" for stdout). With --jsonl, emits JSON Lines.
        #[arg(short, long)]
        output: Option<String>,

        /// Show statistics only
        #[arg(long)]
        stats_only: bool,

        /// Emit scan results as JSON Lines (JSONL)
        #[arg(long, default_value_t = false)]
        jsonl: bool,

        /// Carve payloads to files; pattern may include {stream} and {frame}
        #[arg(long)]
        carve_payloads: Option<String>,

        /// Minimum confidence [0.0-1.0] for reporting/carving frames
        #[arg(long)]
        min_confidence: Option<f32>,
    },

    /// Verify frame integrity and back-links
    Verify {
        /// Input file to verify ("-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Report gaps
        #[arg(long)]
        report_gaps: bool,

        /// Optional FEC sidecar index (JSON) to identify parity blocks
        #[arg(long)]
        fec_index: Option<String>,

        /// Attempt Reed–Solomon repair using FEC sidecar (report-only)
        #[arg(long, default_value_t = false)]
        rs_repair: bool,
    },

    /// Reconstruct timeline from frames
    Timeline {
        /// Input file with frames ("-" for stdin)
        #[arg(short, long)]
        input: String,

        /// Output JSON file for timeline ("-" for stdout)
        #[arg(short, long)]
        output: String,

        /// Include orphaned frames
        #[arg(long)]
        include_orphans: bool,

        /// Emit Graphviz DOT instead of JSON
        #[arg(long, default_value_t = false)]
        dot: bool,

        /// Include detailed analysis (reasons/conflicts/recipes) in JSON or DOT
        #[arg(long, default_value_t = false)]
        analyze: bool,

        /// Optional FEC sidecar index (JSON) to annotate parity frames in stats
        #[arg(long)]
        fec_index: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Execute command
    match cli.command {
        Commands::Pack {
            input,
            output,
            blake3,
            start_id,
            jsonl,
            chunk_strategy,
            rate_limit,
            progress,
            fec_rs_data,
            fec_rs_parity,
            fec_index_out,
        } => commands::pack::execute_ext(
            &input,
            &output,
            blake3,
            start_id,
            jsonl,
            chunk_strategy,
            rate_limit,
            progress,
            fec_rs_data.zip(fec_rs_parity),
            fec_index_out.as_deref(),
        ),

        Commands::Fec {
            input,
            output,
            n_data,
            k_parity,
            fec_index_out,
            dry_run,
        } => commands::fec::inject_parity(
            &input,
            output.as_deref(),
            n_data,
            k_parity,
            fec_index_out.as_deref(),
            dry_run,
        ),

        Commands::Scan {
            input,
            output,
            stats_only,
            jsonl,
            carve_payloads,
            min_confidence,
        } => commands::scan::execute_ext(
            &input,
            output.as_deref(),
            stats_only,
            jsonl,
            carve_payloads.as_deref(),
            min_confidence,
        ),

        Commands::Verify {
            input,
            report_gaps,
            fec_index,
            rs_repair,
        } => commands::verify::execute_ext(&input, report_gaps, fec_index.as_deref(), rs_repair),

        Commands::Timeline {
            input,
            output,
            include_orphans,
            dot,
            analyze,
            fec_index,
        } => commands::timeline::execute_ext(
            &input,
            &output,
            include_orphans,
            dot,
            analyze,
            fec_index.as_deref(),
        ),
    }
}
