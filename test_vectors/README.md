# Durapack Test Vector Gallery

This directory contains comprehensive test vectors for the Durapack frame format specification. Each test vector demonstrates a specific scenario, including both clean data and various corruption types.

## Test Vector Index

### Clean Frames

| # | File | Description | Size | Corruption |
|---|------|-------------|------|------------|
| 01 | `01_minimal_frame.durp` | Smallest valid frame (no payload, no trailer) | 50 bytes | None |
| 02 | `02_frame_with_crc32c.durp` | Frame with CRC32C integrity check | 70 bytes | None |
| 03 | `03_frame_with_blake3.durp` | Frame with BLAKE3 cryptographic hash | ~93 bytes | None |
| 04 | `04_linked_sequence.durp` | Three frames linked via hash chain | ~324 bytes | None |

### Corruption Scenarios

| # | File | Description | Corruption Type | Expected Recovery |
|---|------|-------------|-----------------|-------------------|
| 05 | `05_truncated_frame.durp` | Frame cut off at byte 30 | **Truncation** | 0% - Frame unrecoverable |
| 06 | `06_bit_flip_error.durp` | Single bit flipped in payload | **Bit Flip** | 0% - Checksum fails |
| 07 | `07_burst_error.durp` | 50-byte burst destroys middle frame | **Burst Error** | 66.7% - 2/3 frames recovered |
| 08 | `08_inserted_garbage.durp` | 100 bytes of garbage between frames | **Insertion** | 100% - Both frames recovered |
| 09 | `09_deleted_bytes.durp` | 30 bytes deleted from middle frame | **Deletion** | 66.7% - 2/3 frames recovered |
| 10 | `10_swapped_frames.durp` | Frames in wrong physical order | **Reordering** | 100% - Timeline reconstructs |
| 11 | `11_wrong_checksum.durp` | Corrupted CRC32C trailer | **Wrong Checksum** | 0% - Verification fails |
| 12 | `12_duplicate_frames.durp` | Frame 1 appears twice | **Duplication** | 100% - Deduplication works |
| 13 | `13_reordered_frames.durp` | 4 linked frames scrambled | **Reordering** | 100% - Hash links restore order |

## Corruption Taxonomy Coverage

This test vector gallery covers all corruption types defined in the formal specification:

### ✅ Bit Flips (Vector 06)
- Single bit error in payload
- Detected by CRC32C/BLAKE3
- Frame unrecoverable but identified

### ✅ Burst Errors (Vector 07)
- 50-byte contiguous corruption
- Destroys one complete frame
- Adjacent frames recoverable

### ✅ Truncation (Vector 05)
- File/stream ends prematurely
- Last frame incomplete
- Previous frames recoverable

### ✅ Insertion (Vector 08)
- Garbage data inserted between frames
- Scanner skips invalid data
- All valid frames recovered

### ✅ Deletion (Vector 09)
- Bytes removed from frame
- Causes desynchronization
- Resynchronization at next marker

### ✅ Duplication (Vector 12)
- Same frame appears multiple times
- Deduplication via frame ID
- Timeline handles gracefully

### ✅ Reordering (Vectors 10, 13)
- Frames arrive out of sequence
- Timeline reconstruction via hash links
- Design feature, not a bug

## Usage

### Validating an Implementation

```bash
# Test minimal frame decoding
durapack verify --input test_vectors/01_minimal_frame.durp

# Test corruption recovery
durapack scan --input test_vectors/07_burst_error.durp

# Test timeline reconstruction
durapack timeline --input test_vectors/13_reordered_frames.durp
```

### Programmatic Testing

```rust
use durapack_core::decoder::decode_frame_from_bytes;
use std::fs;

// Test clean frame
let data = fs::read("test_vectors/01_minimal_frame.durp")?;
let frame = decode_frame_from_bytes(&data)?;
assert_eq!(frame.header.frame_id, 1);

// Test corruption detection
let data = fs::read("test_vectors/06_bit_flip_error.durp")?;
assert!(decode_frame_from_bytes(&data).is_err());
```

### Expected Test Results

Run all validation tests:
```bash
cargo test --test test_vectors
```

**Expected output:**
```
test test_validate_minimal_frame ... ok
test test_validate_linked_sequence ... ok
test test_validate_truncated_frame ... ok
test test_validate_bit_flip_error ... ok
test test_validate_burst_error ... ok
test test_validate_inserted_garbage ... ok
test test_validate_swapped_frames ... ok
test test_validate_reordered_with_links ... ok
```

## Implementation Checklist

Use these test vectors to verify your implementation:

- [ ] **Encoding**
  - [ ] Produces bit-identical output to vectors 01-04
  - [ ] Deterministic (same input → same output)

- [ ] **Decoding**
  - [ ] Accepts all clean vectors (01-04)
  - [ ] Rejects truncated frame (05)
  - [ ] Rejects bit flip error (06)
  - [ ] Rejects wrong checksum (11)

- [ ] **Scanning**
  - [ ] Finds all frames in clean streams
  - [ ] Recovers 2/3 frames from burst error (07)
  - [ ] Recovers 100% from inserted garbage (08)
  - [ ] Recovers 2/3 frames from deletion (09)
  - [ ] Finds all frames in reordered stream (10)

- [ ] **Timeline Reconstruction**
  - [ ] Correctly orders linked sequence (04)
  - [ ] Handles swapped frames (10)
  - [ ] Reconstructs from hash links (13)
  - [ ] Detects gaps
  - [ ] Identifies orphans

- [ ] **Edge Cases**
  - [ ] Handles duplicates (12)
  - [ ] Reports statistics correctly
  - [ ] No panics on malformed input

## Performance Expectations

| Test Vector | Expected Scan Time (10 runs) |
|-------------|------------------------------|
| 01-04 (clean) | < 1 ms |
| 05-13 (corrupted) | < 10 ms |
| Large files (1 MB+) | ~1-2 ms |

**Throughput target:** 500-1000 MB/s on modern hardware

## Regenerating Test Vectors

To regenerate all test vectors from scratch:

```bash
cd durapack-core
cargo test --test test_vectors test_generate_all_vectors
```

This will create/update all `.durp` and `.md` files in the `test_vectors/` directory.

## Test Vector Format

Each test vector consists of:

1. **Binary file** (`.durp`): The actual frame data
2. **Documentation** (`.md`): Human-readable description with:
   - Frame structure details
   - Corruption details (if applicable)
   - Expected behavior
   - Hex dump of data

## Contributing New Test Vectors

To add a new test vector:

1. Add a generation function to `durapack-core/tests/test_vectors.rs`
2. Call it from `generate_all_test_vectors()`
3. Add a validation test in the `tests` module
4. Update this README with the new vector
5. Run `cargo test --test test_vectors` to generate

## License

Test vectors are released under:
- Creative Commons CC0 1.0 Universal (Public Domain)

Implementations may use these vectors freely for testing and validation.

---

**Last Updated:** November 1, 2025  
**Test Vector Count:** 13  
**Coverage:** 100% of corruption taxonomy

