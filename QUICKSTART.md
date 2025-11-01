# Durapack Quick Reference

## Installation

```bash
# Clone repository
git clone https://github.com/yourusername/durapack
cd durapack

# Build
cargo build --release

# Run tests
cargo test --all

# Install CLI tool
cargo install --path durapack-cli
```

## Library Usage

### Add to Cargo.toml
```toml
[dependencies]
durapack-core = "0.1"
```

### Encode a Frame
```rust
use durapack_core::encoder::FrameBuilder;
use bytes::Bytes;

let frame = FrameBuilder::new(1)
    .payload(Bytes::from("data"))
    .with_blake3()
    .mark_first()
    .build()?;
```

### Decode a Frame
```rust
use durapack_core::decoder::decode_frame_from_bytes;

let frame = decode_frame_from_bytes(&data)?;
println!("Frame {}: {:?}", frame.header.frame_id, frame.payload);
```

### Scan Damaged Stream
```rust
use durapack_core::scanner::scan_stream;

let frames = scan_stream(&damaged_data);
println!("Recovered {} frames", frames.len());
```

### Reconstruct Timeline
```rust
use durapack_core::linker::link_frames;

let timeline = link_frames(frames);
println!("Ordered: {}, Gaps: {}", 
    timeline.frames.len(), 
    timeline.gaps.len()
);
```

## CLI Commands

### Pack
```bash
# Create frames from JSON
durapack pack --input data.json --output data.durp --blake3

# With custom start ID
durapack pack -i data.json -o data.durp --start-id 1000
```

### Scan
```bash
# Scan damaged file
durapack scan --input damaged.durp --output recovered.json

# Show statistics only
durapack scan -i damaged.durp --stats-only

# Verbose output
durapack -v scan -i damaged.durp
```

### Verify
```bash
# Verify integrity
durapack verify --input data.durp

# Show gaps
durapack verify -i data.durp --report-gaps
```

### Timeline
```bash
# Reconstruct timeline
durapack timeline --input data.durp --output timeline.json

# Include orphaned frames
durapack timeline -i data.durp -o timeline.json --include-orphans
```

## Frame Format

```
┌──────────┬─────────┬───────────┬─────────┬──────────┐
│  Marker  │ Version │ Frame ID  │Prev Hash│Payload..│
│  "DURP"  │   1     │  8 bytes  │32 bytes │         │
└──────────┴─────────┴───────────┴─────────┴─────────┘
     4B        1B         8B         32B       N bytes

┌─────────────┬───────┬──────────────┐
│ Payload Len │ Flags │   Payload    │
│   4 bytes   │  1B   │   N bytes    │
└─────────────┴───────┴──────────────┘

Optional Trailer:
- CRC32C: 4 bytes
- BLAKE3: 32 bytes
```

## Flags

| Bit | Mask | Name       | Description           |
|-----|------|------------|-----------------------|
| 0   | 0x01 | HAS_CRC32C | CRC32C trailer        |
| 1   | 0x02 | HAS_BLAKE3 | BLAKE3 trailer        |
| 2   | 0x04 | IS_FIRST   | First frame           |
| 3   | 0x08 | IS_LAST    | Last frame            |

## Error Handling

```rust
use durapack_core::{FrameError, Result};

match decode_frame_from_bytes(data) {
    Ok(frame) => { /* process frame */ },
    Err(FrameError::BadMarker(_)) => { /* not a frame */ },
    Err(FrameError::ChecksumMismatch { .. }) => { /* corrupted */ },
    Err(e) => { /* other error */ },
}
```

## Performance Tips

1. **Use BLAKE3 for large payloads**: Better performance than CRC32C for >1KB
2. **Batch encoding**: Encode multiple frames before writing
3. **Preallocate buffers**: Use `Vec::with_capacity()` when building streams
4. **Disable logging in production**: Use `default-features = false`

## Common Patterns

### Create Linked Sequence
```rust
let mut frames = Vec::new();
let mut prev_hash = [0u8; 32];

for i in 0..10 {
    let frame_struct = FrameBuilder::new(i)
        .payload(Bytes::from(format!("data {}", i)))
        .prev_hash(prev_hash)
        .with_crc32c()
        .build_struct()?;
    
    prev_hash = frame_struct.compute_hash();
    frames.push(encode_frame_struct(&frame_struct)?);
}
```

### Scan and Recover
```rust
use durapack_core::scanner::scan_stream_with_stats;

let (frames, stats) = scan_stream_with_stats(&data);
println!("Recovery rate: {:.1}%", stats.recovery_rate());

for lf in frames {
    println!("Frame {} at offset {}", 
        lf.frame.header.frame_id,
        lf.offset
    );
}
```

### Detect and Report Gaps
```rust
use durapack_core::linker::{link_frames, verify_backlinks};

let timeline = link_frames(frames);

for gap in &timeline.gaps {
    println!("Missing frames between {} and {}", 
        gap.before, gap.after);
}

let errors = verify_backlinks(&timeline);
if errors.is_empty() {
    println!("All back-links valid");
}
```

## Limits

- **Max frame size**: 16 MB
- **Max payload size**: ~16 MB - 1 KB
- **Protocol version**: 1 (current)
- **Hash size**: 32 bytes (BLAKE3)
- **Frame ID**: 64-bit unsigned integer

## Testing

```bash
# Run all tests
cargo test --all

# Run with verbose output
cargo test --all -- --nocapture

# Run specific test
cargo test test_scan_with_corruption

# Run property tests
cargo test --test proptest

# Run benchmarks
cargo bench -p durapack-core
```

## Feature Flags

```toml
[dependencies]
durapack-core = { version = "0.1", default-features = false }
```

Available features:
- `logging` (default): Enable tracing integration

## Troubleshooting

### "Frame too large" error
- Check payload size < MAX_PAYLOAD_SIZE (16 MB - 1 KB)
- Split large payloads into multiple frames

### "Bad marker" error
- Data is not a valid Durapack frame
- Try using `scan_stream()` for damaged data

### "Checksum mismatch" error
- Frame is corrupted
- Use scanner to find valid frames

### Build errors
- Ensure Rust 1.70+ installed
- Run `cargo clean && cargo build`

## Resources

- **Specification**: `docs/spec.md`
- **Examples**: `examples/`
- **API Docs**: Run `cargo doc --open`
- **GitHub**: https://github.com/etalazz/durapack

## License

Dual-licensed under MIT or Apache 2.0

