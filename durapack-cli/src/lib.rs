//! Library entry for durapack-cli used by integration tests and embedding.

pub mod commands;

// Re-export commands for convenience
pub use commands::*;

/// Shared chunking strategy used by pack
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ChunkStrategy {
    /// One JSON value per line
    Jsonl,
    /// Aggregate all inputs into a single JSON array
    Aggregate,
}

// Re-export commonly used items
pub use crate::commands::pack;

#[cfg(feature = "ed25519-signatures")]
pub use ed25519_dalek;
