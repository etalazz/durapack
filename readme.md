<div align="center">
  <img src="durapack_logo.png" alt="Durapack Logo" width="150">
  <h1>Durapack</h1>
  <p>
    <strong>Frames that survive what the link and the disk don't.</strong>
  </p>
  <p>
    <a href="https://github.com/etalazz/durapack/actions"><img src="https://github.com/etalazz/durapack/workflows/CI/badge.svg" alt="CI Status"></a>
    <a href="https://crates.io/crates/durapack-core"><img src="https://img.shields.io/crates/v/durapack-core.svg" alt="Crates.io"></a>
    <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
  </p>
</div>

---

Durapack is a Rust library for encoding telemetry, audit, or mission data so that it remains **recoverable even when the storage or link is damaged**. Each Durapack record ("frame") is **self-locating** (has a strong marker), **self-describing** (carries its own header/length), and **bidirectionally linkable** (can be re-threaded using IDs or hashes).

## 📖 Table of Contents

- [✨ Features](#-features)
- [🎯 Use Cases](#-use-cases)
- [🚀 Quick Start](#-quick-start)
- [🛠️ CLI Tool](#️-cli-tool)
- [🏗️ Architecture](#️-architecture)
- [📦 Frame Format](#-frame-format)
- [⏱️ Performance](#️-performance)
- [✅ Testing](#-testing)
- [📚 Documentation](#-documentation)
- [🏆 Why Durapack is Better](#-why-durapack-is-better)
- [🤝 Contributing](#-contributing)
- [📜 License](#-license)

---

## ✨ Features

- **Self-synchronization**: Detect frame boundaries in noisy/damaged streams.
- **Local decodability**: Parse a single frame without external schema files.
- **Bidirectional reconstruction**: Reassemble timelines using forward/back references.
- **FEC-ready layout**: Payload kept separable for erasure coding.
- **Small, auditable core**: Minimal dependencies, pure Rust.

## 🎯 Use Cases

- **🛰️ Space/satellite data**: Reconstruct telemetry from partial downlinks.
- **✈️ Black-box forensics**: Recover data from damaged flight recorders.
- **📡 Tactical networks**: Stitch together partial captures from field units.
- **🗄️ Long-term archives**: Data that can survive bit rot and media degradation.

## 🚀 Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
durapack-core = "0.1"
```

### Encoding Frames

```rust
use durapack_core::encoder::FrameBuilder;
use bytes::Bytes;

let payload = Bytes::from("Critical telemetry data");

let frame = FrameBuilder::new(1)
    .payload(payload)
    .with_blake3()      // Add integrity check
    .mark_first()       // Mark as first in sequence
    .build()?;

// Write to file/network
std::fs::write("data.durp", frame)?;
```

### Scanning Damaged Data

```rust
use durapack_core::scanner::scan_stream;

let damaged_data = std::fs::read("damaged.durp")?;

// Recovers all valid frames even if file is partially corrupted
let located_frames = scan_stream(&damaged_data);

println!("Recovered {} frames from damaged file", located_frames.len());
```

---

## 🛠️ CLI Tool

Install the command-line tool:

```bash
cargo install durapack-cli
```

### Commands

- **`pack`**: Read JSON/CBOR → frames → file.
- **`scan`**: Scan damaged file → print recovered frames as JSON.
- **`verify`**: Check links, hashes, and report gaps.
- **`timeline`**: Rethread and export ordered result.

---

## 🏗️ Architecture

Durapack is organized as a Rust workspace:

- **`durapack-core`**: Core library (encoding, decoding, scanning, linking).
- **`durapack-cli`**: Command-line tool.
- **`durapack-fuzz`**: Fuzzing harness (optional).

## 📦 Frame Format

Each frame consists of:

```
┌──────────┬──────────┬─────────┬──────────┐
│  MARKER  │  HEADER  │ PAYLOAD │ TRAILER  │
└──────────┴──────────┴─────────┴──────────┘
   4 bytes   46 bytes   N bytes   0-32 bytes
```

- **Marker**: `"DURP"` - enables byte-by-byte scanning.
- **Header**: version, frame_id, prev_hash, payload_len, flags.
- **Payload**: Application data.
- **Trailer**: Optional CRC32C or BLAKE3 hash.

> For a deep dive, see the [**Frame Specification**](docs/spec.md).

---

## ⏱️ Performance

Benchmarks on an Intel i7-10700K:

| Operation | Size | Throughput |
|-----------|------|------------|
| Encode    | 1KB  | ~800 MB/s  |
| Decode    | 1KB  | ~850 MB/s  |
| Scan      | 10MB | ~600 MB/s  |

Run benchmarks: `cargo bench -p durapack-core`

## ✅ Testing

- **Unit & Integration Tests**: `cargo test --all`
- **Property-based Tests**: `cargo test --test proptest`
- **Fuzzing**: `cd durapack-fuzz && cargo test`

---

## 📚 Documentation

- [Frame Specification](docs/spec.md)
- [Quick Start Guide](QUICKSTART.md)
- [FAQ - Frequently Asked Questions](FAQ.md)
- [API Documentation](https://docs.rs/durapack-core)
- [Examples](examples/)

---

## 🏆 Why Durapack is Better

Durapack is not just another container format; it's a complete system for robust data transport and archival, designed from the ground up to be observable, resilient, and performant in unreliable environments.

### 1. Superior Resilience to Corruption

Most standard formats like TAR, ZIP, or simple line-delimited JSON are brittle. A single corrupted byte can render the rest of the file unreadable.

*   **Durapack's Advantage**: It is designed to survive damage. The `scan` command uses a 4-byte `DURP` marker to find and validate individual frames even within a corrupted file. It can skip over damaged sections and salvage all remaining intact data.

### 2. Advanced Timeline Reconstruction

When data is recovered, understanding the original sequence is crucial.

*   **Durapack's Advantage**: Each frame is cryptographically linked to the previous one using a BLAKE3 hash. The `timeline` command uses these links to reconstruct the original order of frames. Crucially, it also explicitly identifies `gaps` (where frames are missing) and `orphans` (valid frames that can't be placed in the main sequence), providing a complete diagnostic picture.

### 3. High-Performance Integrity and Linking

The choice of hashing algorithm impacts both security and speed.

*   **Durapack's Advantage**: It uses **BLAKE3** for integrity checks and linking. BLAKE3 is a modern cryptographic hash function that is significantly faster than older standards like SHA-2 or MD5, making it ideal for high-throughput applications without compromising on security.

### 4. Rich, Actionable Diagnostics

Most tools simply report success or failure.

*   **Durapack's Advantage**: It provides detailed, structured (JSON) reports on the state of the data. The `timeline` command calculates a `continuity` percentage and lists every gap, giving you a precise measure of data loss. This is invaluable for monitoring and diagnostics.

### Comparison Summary

| Feature | Standard Formats (e.g., TAR, JSONL) | Durapack |
| :--- | :--- | :--- |
| **Corruption Handling** | Often fails on first error. | Scans and recovers all intact frames from a damaged stream. |
| **Data Ordering** | Relies on file order; lost if corrupted. | Reconstructs the timeline using cryptographic links. |
| **Gap Detection** | No built-in mechanism. | Explicitly reports gaps and orphaned frames. |
| **Integrity** | Basic checksums (like CRC32) or none. | Modern, high-speed BLAKE3 hashing for strong integrity. |
| **Diagnostics** | Binary pass/fail. | Rich JSON output with continuity stats, gaps, and orphans. |

---

## 🤝 Contributing

Contributions are welcome! Please fork the repository, create a feature branch, and submit a pull request. Ensure that `cargo fmt` and `cargo clippy` pass.

## 📜 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

---

**Status**: Prototype / Research. Use in production systems at your own risk.

---

## ⚖️ Export Control

This software is subject to U.S. export laws and regulations. By downloading or using this software, you agree to comply with all applicable export laws and regulations.

