<div align="center">
  <img src="durapack_logo.png" alt="Durapack Logo" width="150">
  <h1>Durapack</h1>
  <p>
    <strong>Frames that survive what the link and the disk don't.</strong>
  </p>
  <p>
    <a href="https://github.com/etalazz/durapack/actions/workflows/ci.yml"><img src="https://github.com/etalazz/durapack/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
    <a href="https://github.com/etalazz/durapack/actions/workflows/windows-cli-smoke.yml"><img src="https://github.com/etalazz/durapack/actions/workflows/windows-cli-smoke.yml/badge.svg?branch=main" alt="Windows CLI Smoke"></a>
    <a href="https://crates.io/crates/durapack-core"><img src="https://img.shields.io/crates/v/durapack-core.svg" alt="Crates.io"></a>
    <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
    <a href="https://github.com/sponsors/etalazz"><img src="https://img.shields.io/badge/sponsor-%E2%9D%A4-red" alt="Sponsor"></a>
  </p>
</div>

---

Durapack is a Rust library for encoding telemetry, audit, or mission data so that it remains **recoverable even when the storage or link is damaged**. Each Durapack record ("frame") is **self-locating** (has a strong marker), **self-describing** (carries its own header/length), and **bidirectionally linkable** (can be re-threaded using IDs or hashes).

### 🛡️ A Note on Security and Scope

Durapack is a general-purpose framing and data repair library. It **does not provide encryption**. If you need to protect data at rest or in transit, you should encrypt your payload *before* passing it to Durapack.

## 📖 Table of Contents

- [✨ Features](#-features)
- [🎯 Use Cases](#-use-cases)
- [🚀 Quick Start](#-quick-start)
- [🛠️ CLI Tool](#cli-tool)
- [🏗️ Architecture](#architecture)
- [📦 Frame Format](#-frame-format)
- [⏱️ Performance](#-performance)
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
- **Zero-copy core paths**: Bytes/BytesMut across encoder/decoder/scanner to avoid extra copies.
- **SIMD-accelerated scanner**: memchr/memmem-backed marker search (auto-uses SSE2/AVX2/NEON).
- **no_std + alloc**: `durapack-core` builds without `std`; enable `std` feature for I/O convenience.
- **Optional robust sync**: Preamble + low-autocorrelation sync word with bounded-Hamming fallback in scanner.
- **Burst-error mitigation helpers**: Interleave/deinterleave utilities to spread bursts across frames.

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

### Build features and no_std

- Default build (with `std`): includes convenient I/O helpers and richer error Display via `thiserror`.
- Embedded/firmware (`no_std + alloc`): core works without `std`.

Commands:

```bat
:: Build core without std
cargo build -p durapack-core --no-default-features

:: Build core with std explicitly
cargo build -p durapack-core --features std
```

---

## 🛠️ CLI Tool

Install the command-line tool:

```bash
cargo install durapack-cli
```

### Commands

- **`pack`**: Read JSON/JSONL → frames → file.
- **`scan`**: Scan damaged file → JSON/JSONL records of recovered frames.
- **`verify`**: Check links, hashes, and report gaps.
- **`timeline`**: Rethread and export ordered result (JSON or Graphviz DOT).
- **`fec`**: Post-facto parity injection (requires build with `--features fec-rs`).

### New CLI ergonomics

- Stream-friendly I/O:
  - All commands accept `-` for stdin and can write to stdout (e.g., `-o -`).
  - `scan --jsonl` streams one JSON record per line: a Stats record, Gap records (with confidence), then Frame records (with confidence).
- Packing flexibility:
  - `pack --jsonl` reads one JSON object per line; `--chunk-strategy {jsonl|aggregate}` controls parsing.
  - `pack --rate-limit <bytes/sec>` throttles output; `--progress` shows a progress bar.
- Carving payloads:
  - `scan --carve-payloads "payload_{stream}_{frame}.bin"` writes each recovered payload to disk. Combine with `--min-confidence <0.0-1.0>` to filter lower-confidence hits.
- Visualizing timelines:
  - `timeline --dot -o -` emits Graphviz DOT; add `--analyze` for labeled gaps, conflicts, clusters, and recovery notes; pipe to `dot` to render.

### Examples (Windows cmd)

```bat
:: Pack JSONL from stdin to a file with BLAKE3 and a progress bar
type data.jsonl | durapack pack -i - -o out.durp --blake3 --jsonl --progress

:: Scan a file to JSON Lines on stdout and carve payloads
Durapack scan -i out.durp --jsonl --carve-payloads "payload_{stream}_{frame}.bin" -o -

:: Verify from stdin with colored output
type out.durp | durapack verify -i - --report-gaps

:: Timeline to DOT and render with Graphviz
Durapack timeline -i out.durp --dot -o - | dot -Tpng -o timeline.png
```

#### Timeline with analysis (JSON)

```bat
:: Emit timeline JSON to stdout including gap reasons, conflicts, clusters, and recipes
Durapack timeline -i out.durp -o - --analyze
```

Example output (truncated):

```json
{
  "frames": [
    { "frame_id": 10, "prev_hash": "…", "payload": "…" },
    { "frame_id": 12, "prev_hash": "…", "payload": "…" }
  ],
  "gaps": [ { "before": 10, "after": 12 } ],
  "stats": { "total_frames": 15, "gaps": 1, "orphans": 0, "continuity": 93.33 },
  "analysis": {
    "gap_reasons": [ { "before": 10, "after": 12, "reason": "missing-by-id" } ],
    "conflicts": [],
    "orphan_clusters": [ { "ids": [42, 43] } ],
    "recipes": [
      { "type": "InsertParityFrame", "between": [10, 12], "reason": "gap detected: MissingById" },
      { "type": "RewindOffset", "near_frame": 12, "by_bytes": 128, "reason": "non-contiguous offsets across gap" }
    ]
  }
}
```

### CLI reference (--help)

 Global options:

 - -v, --verbose  Enable verbose logging

 Subcommands and options:

 - pack
   - -i, --input <FILE|->
     Input JSON or JSONL file. Use "-" to read from stdin.
   - -o, --output <FILE|->
     Output file for packed frames. Use "-" to write to stdout.
   - --blake3
     Use BLAKE3 trailer instead of CRC32C.
   - --start-id <u64> (default: 1)
     Starting frame ID for the first frame.
   - --jsonl (default: false)
     Interpret input as JSON Lines (one JSON object per line).
   - --chunk-strategy <jsonl|aggregate> (default: aggregate)
     Parsing strategy when reading stdin/JSONL.
   - --rate-limit <bytes/sec>
     Throttle output to approximately this rate.
   - --progress (default: false)
     Show a progress bar during packing.
  - --fec-rs-data <N> and --fec-rs-parity <K> (requires building with `--features fec-rs`)
    Emit K parity frames after each N data frames; sidecar index written via `--fec-index-out` or defaults to `<output>.fec.json`.
  - --fec-index-out <path>
    Path to write the FEC sidecar index (JSON).

 - scan
   - -i, --input <FILE|->
     Input file to scan. Use "-" to read from stdin.
   - -o, --output <FILE|->
     Output file. With --jsonl, emits JSON Lines; otherwise pretty JSON. Use "-" to write to stdout.
   - --stats-only
     Print statistics only and exit.
   - --jsonl (default: false)
     Stream results as JSON Lines (records: Stats, Gap, Frame).
   - --min-confidence <float>
     Minimum confidence threshold [0.0-1.0] to report/carve frames.
   - --carve-payloads <pattern>
     Write payloads to files; pattern may include {stream} and {frame}.

 - verify
   - -i, --input <FILE|->
     Input file to verify. Use "-" to read from stdin.
   - --report-gaps
     Also list detected gaps in the sequence.
  - --fec-index <path>
    Load FEC sidecar for parity block metadata.
  - --rs-repair
    Simulate RS reconstructability per block (report-only; requires build with `--features fec-rs`).

 - timeline
   - -i, --input <FILE|->
     Input file with frames. Use "-" to read from stdin.
   - -o, --output <FILE|->
     Output JSON file for the timeline, or "-" for stdout.
   - --include-orphans
     Include orphaned frames in the JSON output.
   - --dot (default: false)
     Emit a Graphviz DOT graph instead of JSON (to file or stdout).
   - --analyze (default: false)
     Include detailed analysis in outputs. JSON gains `analysis` with `gap_reasons`, `conflicts`, `orphan_clusters`, and `recipes`. With `--dot`, the graph includes labeled gaps, conflict edges, orphan clusters, and note-shaped recipe hints.
  - --fec-index <path>
    Annotate DOT with RS clusters (N+K) if a sidecar is provided.
+
+- fec (post-facto parity injection; requires build with `--features fec-rs`)
+  - -i, --input <FILE|->
+    Input .durp file to protect.
+  - -o, --output <FILE>
+    Output file; if omitted, appends parity to the input file.
+  - --n-data <N>
+    RS data shard count.
+  - --k-parity <K>
+    RS parity shard count.
+  - --fec-index-out <path>
+    Write/update sidecar JSON mapping blocks and parity frame IDs.
+  - --dry-run (default: false)
+    Compute parity without writing frames; still emits sidecar if requested.
+
+Example:
+
+```bat
+cargo run -p durapack-cli --features fec-rs -- fec ^
+  --input out.durp ^
+  --n-data 8 ^
+  --k-parity 2 ^
+  --fec-index-out out.durp.fec.json
+```

 Quick help

 - durapack --help
 - durapack pack --help
 - durapack scan --help
 - durapack verify --help
 - durapack timeline --help

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

> For a deep dive, see the [**Formal Specification (NEW)**](docs/FORMAL_SPEC.md) and [Frame Specification](docs/spec.md).

### Optional Forward Error Correction (FEC)

Durapack can be paired with pluggable, optional FEC at the application layer:

- Reed–Solomon parity frames (feature: `fec-rs`): for every N payload frames, emit K parity frames capable of repairing up to K losses within the block.
- Interleaved RS: combine with `interleave_bytes` to spread data across frames and RS across stripes for burst-damage media.
- Research PoC features (behind flags, no implementation): `fec-raptorq`, `fec-ldpc`.

Enabling RS in core (Cargo features):

```bat
:: Build with Reed–Solomon support
env RUSTFLAGS= cargo build -p durapack-core --features fec-rs
```

API sketch (library):

```rust
use durapack_core::fec::{RedundancyEncoder, RedundancyDecoder};
#[cfg(feature = "fec-rs")] use durapack_core::fec::{RsEncoder, RsDecoder};

#[cfg(feature = "fec-rs")]
{
    // N data frames, K parity frames
    let enc = RsEncoder::new(N, K);
    let blocks = enc.encode_batch(&frames, 0)?; // returns N+K blocks

    // drop some blocks ... then recover
    let dec = RsDecoder;
    let recovered = dec.decode_batch(&available_blocks, N)?;
}
```

Export-control note: Some advanced FEC schemes (e.g., certain LDPC/Raptor variants) may be subject to additional export restrictions. This repository ships only RS by default; research flags are stub-only. You are responsible for compliance with applicable laws.

---

## ⏱️ Performance

- Zero-copy encoder/decoder/scanner paths using Bytes/BytesMut.
- SIMD-accelerated marker search via memchr::memmem (auto-dispatch to SSE2/AVX2/NEON).
- Criterion benches with realistic corpora (scanner + encoding).
- Burst-error mitigation helpers: `durapack_core::interleave::{interleave_bytes, deinterleave_bytes}`.

Run benchmarks:

```bat
cargo bench -p durapack-core --bench scanner
cargo bench -p durapack-core --bench encoding
:: RS FEC benches (requires building with fec-rs)
cargo bench -p durapack-core --features fec-rs --bench fec
```

### Scanning confidence model

The scanner assigns a confidence score [0.0, 1.0] to each recovered frame based on:

- Marker quality (exact vs. bounded-Hamming match)
- Presence of robust sync/preamble before the marker
- Trailer validation strength (BLAKE3 > CRC32C > none)
- Size sanity and neighbor consistency (backlinks, contiguous spacing)

You can filter outputs and carving by `--min-confidence`.

### Burst-error mitigation (interleaving)

Writer-side guidance:
- Choose a stripe `group` (number of consecutive frames to spread data across) and `shard_len` (bytes per stripe per round).
- Use `interleave_bytes(&data, InterleaveParams { group, shard_len })` to split payload across upcoming frames.
- Include the parameters in your metadata or superframe index so readers can reassemble.

Reader-side guidance:
- Collect stripes for the same content (e.g., from consecutive frames) and call `deinterleave_bytes(&stripes, params)` to reconstruct the original.

---

## 🔐 FEC sidecar format (pack)

When `pack` is invoked with `--fec-rs-data N --fec-rs-parity K`, the tool writes a sidecar JSON (default `<output>.fec.json`) containing entries like:

```json
[
  { "block_start_id": 1, "data": 8, "parity": 2, "parity_frame_ids": [9, 10] },
  { "block_start_id": 11, "data": 8, "parity": 2, "parity_frame_ids": [19, 20] }
]
```

- `block_start_id`: the first frame ID in the RS block (data frames)
- `data`: N data frames
- `parity`: K parity frames
- `parity_frame_ids`: IDs of emitted parity frames for that block

The sidecar lets downstream tools annotate timelines and, in future, attempt automated repairs. Today, `verify --rs-repair` reports whether RS blocks are theoretically reconstructable given N and K.

---

## 📚 Documentation

- [**Formal Specification (NEW)**](docs/FORMAL_SPEC.md) - Complete on-disk format specification
- [Frame Specification](docs/spec.md) - Original specification document
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

---

### Continuous Integration

- Linux CI runs formatting, clippy, and tests for the whole workspace.
- Windows CI runs a PowerShell smoke test of all CLI commands.
  - Workflow: `.github/workflows/windows-cli-smoke.yml`
  - Local run: `pwsh -NoProfile -File scripts/test-cli.ps1`

---

## 🔒 Robust sync/marker (format robustness)

Durapack can optionally harden synchronization to improve recovery in noisy/damaged streams:

- Robust sync word before the 4-byte marker: `ROBUST_SYNC_WORD` (low autocorrelation)
- Optional preamble: `PREAMBLE_PATTERN` repeated for at least `MIN_PREAMBLE_LEN` bytes
- Scanner tolerance: bounded Hamming distance on `FRAME_MARKER` via `MAX_MARKER_HAMMING`

How the scanner searches:
1) Exact `FRAME_MARKER` via `memchr::memmem` (fast path)
2) If present: look for `ROBUST_SYNC_WORD` and attempt marker immediately after
3) If present: detect a short preamble run then attempt marker
4) Fallback: bounded-Hamming match of the 4-byte marker to tolerate 1-bit flips

Enable preamble/sync when encoding (optional):

```rust
use durapack_core::{constants::FrameFlags, encoder::FrameBuilder};
use bytes::Bytes;

let payload = Bytes::from("payload");
let encoded = FrameBuilder::new(1)
    .payload(payload)
    // Set flags via FrameHeader in build_struct if you need full control.
    // For raw encode_frame usage, set header.flags to include these bits:
    // FrameFlags::HAS_PREAMBLE | FrameFlags::HAS_SYNC_PREFIX
    .build()?;
```

Notes:
- Defaults remain unchanged; no extra bytes are added unless you enable the flags.
- The scanner automatically benefits from sync/preamble if present.

---

## 📇 Superframes and skip lists (optional)

For very large streams, you can accelerate resync and seeking by periodically inserting superframes and optional skip-list backlinks:

- Superframes summarize a recent range (IDs, offsets, checksums) to enable bounded binary search mid-stream.
- Skip-list backlinks add logarithmic (2^k) pointers in payload so `Timeline::seek_with_skiplist` can locate targets in ~O(log n) when present.

Enable on the builder (payload should carry the index/backlinks your app defines):

```rust
use durapack_core::encoder::FrameBuilder;
use bytes::Bytes;

let encoded = FrameBuilder::new(1024)
    .payload(Bytes::from("index-or-summary"))
    .as_superframe()
    .with_skiplist()
    .build()?;
```

Note: These features are optional and backward-compatible; readers that don’t use them will still decode frames normally.
