# Durapack

[![CI](https://github.com/yourusername/durapack/workflows/CI/badge.svg)](https://github.com/yourusername/durapack/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Crates.io](https://img.shields.io/crates/v/durapack-core.svg)](https://crates.io/crates/durapack-core)

**Frames that survive what the link and the disk don't.**

Durapack is a Rust library for encoding telemetry, audit, or mission data so that it remains **recoverable even when the storage or link is damaged**. Each Durapack record ("frame") is **self-locating** (has a strong marker), **self-describing** (carries its own header/length), and **bidirectionally linkable** (can be re-threaded using IDs or hashes).

## Features

- **Self-synchronization**: Detect frame boundaries in noisy/damaged streams
- **Local decodability**: Parse a single frame without external schema files
- **Bidirectional reconstruction**: Reassemble timelines using forward/back references
- **FEC-ready layout**: Payload kept separable for erasure coding
- **Small, auditable core**: Minimal dependencies, pure Rust

## Use Cases

- **Space/satellite data**: Reconstruct telemetry from partial downlinks
- **Black-box forensics**: Recover data from damaged flight recorders
- **Tactical networks**: Stitch together partial captures from field units
- **Long-term archives**: Data that can survive bit rot and media degradation

## Quick Start

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

### Decoding Frames

```rust
use durapack_core::decoder::decode_frame_from_bytes;

let data = std::fs::read("data.durp")?;
let frame = decode_frame_from_bytes(&data)?;

println!("Frame {}: {:?}", frame.header.frame_id, frame.payload);
```

### Scanning Damaged Data

```rust
use durapack_core::scanner::scan_stream;

let damaged_data = std::fs::read("damaged.durp")?;

// Recovers all valid frames even if file is partially corrupted
let located_frames = scan_stream(&damaged_data);

println!("Recovered {} frames from damaged file", located_frames.len());
```

### Reconstructing Timelines

```rust
use durapack_core::linker::link_frames;

let frames = located_frames.into_iter()
    .map(|lf| lf.frame)
    .collect();

let timeline = link_frames(frames);

println!("Timeline: {} frames, {} gaps", 
    timeline.frames.len(),
    timeline.gaps.len()
);
```

## CLI Tool

Install the command-line tool:

```bash
cargo install durapack-cli
```

### Pack data into frames

```bash
durapack pack --input data.json --output data.durp --blake3
```

### Scan damaged file

```bash
durapack scan --input damaged.durp --output recovered.json
```

### Verify integrity

```bash
durapack verify --input data.durp --report-gaps
```

### Reconstruct timeline

```bash
durapack timeline --input data.durp --output timeline.json
```

## Architecture

Durapack is organized as a Rust workspace:

- **`durapack-core`**: Core library (encoding, decoding, scanning, linking)
- **`durapack-cli`**: Command-line tool
- **`durapack-fuzz`**: Fuzzing harness (optional)

## Frame Format

Each frame consists of:

```
┌──────────┬──────────┬─────────┬──────────┐
│  MARKER  │  HEADER  │ PAYLOAD │ TRAILER  │
└──────────┴──────────┴─────────┴──────────┘
   4 bytes   46 bytes   N bytes   0-32 bytes
```

- **Marker**: `"DURP"` - enables byte-by-byte scanning
- **Header**: version, frame_id, prev_hash, payload_len, flags
- **Payload**: Application data
- **Trailer**: Optional CRC32C or BLAKE3 hash

See [Frame Specification](docs/spec.md) for details.

## Performance

Benchmarks on an Intel i7-10700K:

| Operation | Size | Throughput |
|-----------|------|------------|
| Encode    | 1KB  | ~800 MB/s  |
| Decode    | 1KB  | ~850 MB/s  |
| Scan      | 10MB | ~600 MB/s  |

Run benchmarks:

```bash
cargo bench -p durapack-core
```

## Testing

### Unit & Integration Tests

```bash
cargo test --all
```

### Property-based Tests

```bash
cargo test --test proptest
```

### Fuzzing

```bash
cd durapack-fuzz
cargo test
```

## Documentation

- [Frame Specification](docs/spec.md)
- [API Documentation](https://docs.rs/durapack-core)
- [Examples](examples/)

## Design Goals

1. **Hostile media resilience**: Survive partial corruption, reordering, or loss
2. **Self-contained frames**: Each frame is independently parseable
3. **Forensic analysis**: Reconstruct timelines from incomplete captures
4. **Deterministic encoding**: Same input always produces same output
5. **No external dependencies**: Frame structure is self-describing

## Non-Goals

- **Transport protocol**: Durapack is storage-focused, not network-focused
- **Compression**: Apply externally to payloads
- **Encryption**: Apply externally to payloads
- **Real-time streaming**: Designed for durability, not latency

## Comparison

| Format | Self-sync | Damage recovery | Back-links | Use case |
|--------|-----------|-----------------|------------|----------|
| **Durapack** | ✓ | ✓ | ✓ | Hostile media |
| Log files | ✗ | ✗ | ✗ | Perfect storage |
| WARC | ✓ | Partial | ✗ | Web archives |
| CCSDS | ✓ | Partial | ✗ | Space packets |
| Protocol Buffers | ✗ | ✗ | ✗ | RPC |

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo fmt` and `cargo clippy` pass
5. Submit a pull request

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Citation

If you use Durapack in research, please cite:

```bibtex
@software{durapack2025,
  title={Durapack: Self-Locating Framing for Hostile Media},
  author={Durapack Contributors},
  year={2025},
  url={https://github.com/yourusername/durapack}
}
```

## Related Work

- [CCSDS Space Packet Protocol](https://public.ccsds.org/Pubs/133x0b2e1.pdf)
- [DTN Bundle Protocol](https://datatracker.ietf.org/doc/html/rfc9171)
- [WARC Format](https://iipc.github.io/warc-specifications/)
- [Fountain Codes](https://en.wikipedia.org/wiki/Fountain_code)

---

**Status**: Prototype / Research

This is a research prototype. Use in production systems at your own risk.

