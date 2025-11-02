//! # Durapack Core
//!
//! A self-locating, bidirectionally-linkable framing format for hostile or partially damaged media.
//!
//! ## Modules
//!
//! - `constants`: Frame format constants and limits
//! - `types`: Core types (Frame, FrameHeader, FrameError)
//! - `encoder`: Frame encoding
//! - `decoder`: Strict frame decoding
//! - `scanner`: Damaged stream scanning and recovery
//! - `linker`: Bidirectional timeline reconstruction
//! - `fec`: Forward Error Correction traits (interface only)

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod constants;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod fec;
pub mod linker;
pub mod scanner;
pub mod types;

// Re-export commonly used types
pub use error::FrameError;
pub use types::{DurapackSerializable, Frame, FrameHeader};

/// Result type alias for Durapack operations
pub type Result<T> = core::result::Result<T, FrameError>;
