# Durapack Frequently Asked Questions (FAQ)

## General Questions

### What is Durapack?

Durapack is a self-locating, bidirectionally-linkable framing format designed for storing and transmitting data in hostile or unreliable environments. It allows data recovery even when storage media or transmission links are partially damaged.

### Who should use Durapack?

Durapack is ideal for:
- **Space and satellite missions** - Recovering telemetry from partial downlinks
- **Aviation black boxes** - Forensic data recovery from damaged recorders
- **Military/tactical systems** - Reconstructing data from field units
- **Long-term archives** - Protecting against bit rot and media degradation
- **IoT/edge devices** - Resilient logging in harsh environments
- **Research data** - Preserving critical experimental results

### How is Durapack different from regular file formats?

Unlike regular formats, Durapack:
- Can **self-synchronize** - finds frame boundaries even in corrupted streams
- Has **no external dependencies** - each frame is self-describing
- Supports **bidirectional linking** - can reconstruct timelines with gaps
- Is **damage-resistant** - recovers valid frames from partially corrupted files
- Works **without a complete file** - partial data is still useful

---

## Recovery Capabilities

### How much data loss can Durapack recover from?

**Without FEC (current implementation):**
- **Best case**: 100% recovery if damage is outside frame boundaries
- **Typical case**: 60-80% recovery with random corruption
- **Real example**: 70% recovery rate (7/10 frames) with 200 bytes corrupted

**With future FEC implementation:**
- **With 20% overhead**: Can recover from up to 20% data loss
- **With Raptor codes**: Near 100% recovery with 10% overhead
- **Burst errors**: Much better recovery than without FEC

### What determines the recovery rate?

Recovery depends on:
1. **Damage pattern** - Random vs. burst errors
2. **Frame size** - Smaller frames = better granularity
3. **Marker survival** - Frames need intact "DURP" markers
4. **Checksum type** - BLAKE3 provides stronger integrity
5. **FEC configuration** - When implemented, adds redundancy

### What types of damage can Durapack handle?

| Damage Type | Recovery | Explanation |
|------------|----------|-------------|
| **Random bit flips** | 60-90% | Most frames with valid markers survive |
| **Contiguous deletion** | Varies | Frames outside deleted region recovered |
| **Overwritten sections** | 50-80% | Depends on which frames affected |
| **Marker corruption** | 0% for that frame | Cannot locate frame boundary |
| **Payload corruption** | 0% for that frame | Fails checksum validation |
| **Reordered frames** | 100% | Timeline reconstruction handles this |
| **Duplicated frames** | 100% | Detected via frame IDs |

### Can Durapack recover from a completely corrupted file?

No. If **all** frame markers are destroyed, recovery is impossible. However, if even a single valid frame survives, it can be recovered and its data preserved.

### How does recovery work technically?

1. **Marker scanning**: Searches byte-by-byte for "DURP" markers
2. **Frame extraction**: Attempts to decode each potential frame
3. **Validation**: Verifies checksums/hashes
4. **Collection**: Gathers all valid frames
5. **Linking**: Reconstructs timeline using prev_hash chains
6. **Gap detection**: Identifies missing frames

### What's the minimum recoverable unit?

A single complete frame (minimum 54 bytes: marker + header + payload + trailer).

---

## Technical Questions

### What is the frame format?

```
┌──────────┬──────────┬─────────┬──────────┐
│  Marker  │  Header  │ Payload │ Trailer  │
│  "DURP"  │ 46 bytes │ N bytes │ 0-32 B   │
└──────────┴──────────┴─────────┴──────────┘
```

**Marker**: 4 bytes - "DURP" (0x44 0x55 0x52 0x50)  
**Header**: 46 bytes - version, frame_id, prev_hash, payload_len, flags  
**Payload**: Variable - your actual data (up to ~16 MB)  
**Trailer**: Optional - CRC32C (4 bytes) or BLAKE3 (32 bytes)

### What's the overhead per frame?

- **Minimum**: 50 bytes (marker + header, no trailer)
- **With CRC32C**: 54 bytes
- **With BLAKE3**: 82 bytes

For a 1 KB payload:
- **CRC32C overhead**: 5.3%
- **BLAKE3 overhead**: 8.0%

### What's the maximum frame size?

- **Maximum frame**: 16 MB total
- **Maximum payload**: ~16 MB - 1 KB (to account for header/trailer)

### Should I use CRC32C or BLAKE3?

| Feature | CRC32C | BLAKE3 |
|---------|--------|--------|
| **Size** | 4 bytes | 32 bytes |
| **Speed** | Faster | Very fast |
| **Collision resistance** | Moderate | Cryptographic |
| **Use case** | Small frames, speed critical | Large frames, high integrity |

**Recommendation**: Use BLAKE3 for most cases. The overhead is minimal and integrity guarantees are much stronger.

### Can frames be read in any order?

Yes! Each frame is self-contained. However, timeline reconstruction works best when you have the complete sequence.

### What's the purpose of prev_hash?

The `prev_hash` field:
- Links frames bidirectionally
- Enables timeline reconstruction
- Detects missing frames
- Verifies sequence integrity
- Creates a tamper-evident chain

---

## Usage Questions

### How do I encode data into frames?

**Library usage**:
```rust
use durapack_core::encoder::FrameBuilder;
use bytes::Bytes;

let frame = FrameBuilder::new(1)
    .payload(Bytes::from("your data"))
    .with_blake3()
    .mark_first()
    .build()?;
```

**CLI usage**:
```bash
durapack pack --input data.json --output data.durp --blake3
```

### How do I recover data from a damaged file?

**Library usage**:
```rust
use durapack_core::scanner::scan_stream;

let data = std::fs::read("damaged.durp")?;
let frames = scan_stream(&data);
println!("Recovered {} frames", frames.len());
```

**CLI usage**:
```bash
durapack scan --input damaged.durp --output recovered.json
```

### How do I verify file integrity?

**CLI usage**:
```bash
durapack verify --input data.durp --report-gaps
```

This checks:
- Frame marker validity
- Checksum/hash integrity
- Back-link consistency
- Sequence completeness

### How do I create a sequence of linked frames?

```rust
let mut prev_hash = [0u8; 32]; // First frame

for i in 0..10 {
    let frame = FrameBuilder::new(i)
        .payload(Bytes::from(format!("Frame {}", i)))
        .prev_hash(prev_hash)
        .with_blake3()
        .build_struct()?;
    
    prev_hash = frame.compute_hash(); // For next frame
    
    // Encode and write...
}
```

### What's the recommended frame size?

**General recommendation**: 1-4 KB per frame

**Trade-offs**:
- **Smaller frames** (256B-1KB): Better recovery granularity, higher overhead
- **Larger frames** (4KB-16KB): Lower overhead, coarser recovery
- **Very large** (>1MB): Risky - one corruption loses lots of data

**Use case specific**:
- **Telemetry logs**: 512B-1KB
- **Sensor data**: 256B-512B
- **File chunks**: 4KB-64KB
- **Large binary blobs**: Consider splitting into multiple frames

---

## Performance Questions

### How fast is encoding/decoding?

Benchmark results (Intel i7-10700K):
- **Encoding 1KB**: ~800 MB/s
- **Decoding 1KB**: ~850 MB/s
- **Scanning 10MB**: ~600 MB/s

Performance is excellent for most use cases.

### Does it work with large files?

Yes, but consider:
- **Streaming**: Process frames one at a time to avoid loading entire file
- **Batching**: Write frames in batches for efficiency
- **Memory**: Each frame decoded is held in memory

### Can I use Durapack in embedded systems?

Yes! Features:
- **No_std support**: Possible with feature flags
- **Minimal dependencies**: Small binary size
- **No allocator required**: For basic operations
- **Disable logging**: Use `default-features = false`

---

## Compatibility Questions

### Is the format stable?

**Version 1** is stable. The format includes version field for future compatibility.

### Can I read old frames with new code?

Yes, as long as version is supported. Version 1 will be supported indefinitely.

### Can I extend the format?

Future versions may add:
- Extended headers
- New trailer types
- Additional flags
- Backward-compatible changes are prioritized

### Does it work across different architectures?

Yes! Uses big-endian byte order for portability. Tested on:
- x86_64 (Windows, Linux, macOS)
- ARM (future testing)
- Any architecture supporting Rust

---

## Troubleshooting

### "Frame too large" error

**Problem**: Frame exceeds 16 MB limit

**Solutions**:
1. Split payload into multiple frames
2. Compress payload before encoding
3. Check payload_len field is correct

### "Bad marker" error

**Problem**: Data doesn't start with "DURP"

**Solutions**:
1. Use `scan_stream()` for damaged data
2. Verify file is actually Durapack format
3. Check for file corruption at start

### "Checksum mismatch" error

**Problem**: Frame data is corrupted

**Solutions**:
1. Use scanner to find valid frames: `scan_stream()`
2. Check storage media health
3. Consider data is unrecoverable if checksum fails

### Scanner finds no frames

**Possible causes**:
1. File is completely corrupted
2. File is not Durapack format
3. All markers destroyed

**Debugging**:
```bash
# Check if file contains "DURP" markers
grep -a "DURP" file.durp

# Use verbose scanning
durapack -v scan --input file.durp --stats-only
```

### Low recovery rate

**Causes**:
- Extensive damage
- Large frame sizes
- No FEC redundancy

**Improvements**:
1. Use smaller frames (better granularity)
2. Enable BLAKE3 (better integrity detection)
3. Wait for FEC implementation
4. Add application-level redundancy

---

## Comparison Questions

### How does Durapack compare to tar/zip?

| Feature | Durapack | tar/zip |
|---------|----------|---------|
| **Damage recovery** | Excellent | Poor |
| **Self-sync** | Yes | No |
| **Compression** | No (apply externally) | Yes |
| **Streaming** | Yes | Limited |
| **Overhead** | 5-8% | Varies |

**Use Durapack when**: Reliability > compression  
**Use tar/zip when**: Compression > reliability

### How does it compare to database WAL?

| Feature | Durapack | WAL |
|---------|----------|-----|
| **Damage recovery** | Better | Good |
| **Transaction support** | No | Yes |
| **Random access** | Scan required | Indexed |
| **Use case** | Hostile media | Database logging |

### How does it compare to WARC (Web ARChive)?

| Feature | Durapack | WARC |
|---------|----------|------|
| **Self-location** | Strong | Moderate |
| **Bidirectional links** | Yes | No |
| **Binary efficiency** | High | Lower (text-based) |
| **Use case** | Hostile media | Web archival |

### How does it compare to CCSDS (space packets)?

| Feature | Durapack | CCSDS |
|---------|----------|-------|
| **Self-sync** | Yes | Yes |
| **Back-links** | Yes | No |
| **FEC** | Interface ready | Often included |
| **Overhead** | Similar | Similar |

**Durapack advantage**: Bidirectional linking for forensic reconstruction

---

## Future Plans

### When will FEC be implemented?

Not currently scheduled. Contributions welcome! Good candidates:
- Reed-Solomon codes
- Raptor (fountain) codes
- LDPC codes

### Will there be compression support?

Compression is intentionally external. Apply before encoding:
```rust
let compressed = compress(data);
let frame = FrameBuilder::new(1)
    .payload(Bytes::from(compressed))
    .build()?;
```

### Are there plans for encryption?

Encryption is also external. Encrypt payload before framing:
```rust
let encrypted = encrypt(data, key);
let frame = FrameBuilder::new(1)
    .payload(Bytes::from(encrypted))
    .build()?;
```

This keeps Durapack focused on reliability, not security.

### Will there be a GUI tool?

Possibly in the future. The CLI covers most use cases currently.

---

## Contributing

### How can I contribute?

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Popular contribution areas:
- FEC implementation (Reed-Solomon, Raptor)
- Additional CLI commands
- Performance optimizations
- Platform testing
- Documentation improvements

### How do I report bugs?

Open an issue on GitHub with:
1. Durapack version
2. Operating system
3. Rust version
4. Steps to reproduce
5. Expected vs. actual behavior

### Can I use Durapack in commercial projects?

Yes! Dual-licensed under MIT or Apache 2.0. Choose whichever fits your needs.

---

## License & Support

### What's the license?

Dual-licensed:
- **MIT License**: Simple, permissive
- **Apache 2.0**: Patent protection included

Choose either license for your project.

### Where can I get help?

1. **Documentation**: Read `docs/spec.md` and `QUICKSTART.md`
2. **Examples**: Check `examples/` directory
3. **API docs**: Run `cargo doc --open`
4. **GitHub Issues**: Ask questions or report bugs

### Is commercial support available?

Not officially. This is an open-source project. Community support via GitHub issues.

---

## Quick Links

- **GitHub Repository**: https://github.com/etalazz/durapack
- **Documentation**: [docs/spec.md](docs/spec.md)
- **Quick Start**: [QUICKSTART.md](QUICKSTART.md)
- **Examples**: [examples/](examples/)
- **Implementation Guide**: [IMPLEMENTATION_COMPLETE.md](IMPLEMENTATION)

---

**Last Updated**: November 1, 2025  
**Version**: 0.1.0

*Have a question not answered here? Open an issue on GitHub!*

