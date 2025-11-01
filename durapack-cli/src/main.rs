mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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
        /// Input JSON file (array of payloads)
        #[arg(short, long)]
        input: String,

        /// Output file for packed frames
        #[arg(short, long)]
        output: String,

        /// Use BLAKE3 trailer instead of CRC32C
        #[arg(long)]
        blake3: bool,

        /// Starting frame ID
        #[arg(long, default_value = "1")]
        start_id: u64,
    },

    /// Scan damaged file and recover frames
    Scan {
        /// Input file to scan
        #[arg(short, long)]
        input: String,

        /// Output JSON file for recovered frames
        #[arg(short, long)]
        output: Option<String>,

        /// Show statistics only
        #[arg(long)]
        stats_only: bool,
    },

    /// Verify frame integrity and back-links
    Verify {
        /// Input file to verify
        #[arg(short, long)]
        input: String,

        /// Report gaps
        #[arg(long)]
        report_gaps: bool,
    },

    /// Reconstruct timeline from frames
    Timeline {
        /// Input file with frames
        #[arg(short, long)]
        input: String,

        /// Output JSON file for timeline
        #[arg(short, long)]
        output: String,

        /// Include orphaned frames
        #[arg(long)]
        include_orphans: bool,
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
        } => commands::pack::execute(&input, &output, blake3, start_id),

        Commands::Scan {
            input,
            output,
            stats_only,
        } => commands::scan::execute(&input, output.as_deref(), stats_only),

        Commands::Verify { input, report_gaps } => commands::verify::execute(&input, report_gaps),

        Commands::Timeline {
            input,
            output,
            include_orphans,
        } => commands::timeline::execute(&input, &output, include_orphans),
    }
}
