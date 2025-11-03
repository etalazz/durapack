# Durapack Frame Format Specification

**Version:** 1.0  
**Status:** Draft  
**Date:** November 1, 2025  
**Authors:** Durapack Contributors

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Design Goals](#2-design-goals)
3. [Frame Structure](#3-frame-structure)
4. [Header Specification](#4-header-specification)
5. [Feature Flags](#5-feature-flags)
6. [Backward Link Semantics](#6-backward-link-semantics)
7. [Trailer Variants](#7-trailer-variants)
8. [Alignment and Padding Rules](#8-alignment-and-padding-rules)
9. [Reserved Fields](#9-reserved-fields)
10. [Corruption Taxonomy](#10-corruption-taxonomy)
11. [Scanner Behavior](#11-scanner-behavior)
12. [Versioning and Compatibility](#12-versioning-and-compatibility)
13. [Implementation Requirements](#13-implementation-requirements)
14. [Test Vectors](#14-test-vectors)
15. [Interleaving Guidance (Burst-error Mitigation)](#15-interleaving-guidance-burst-error-mitigation)

---

## 1. Introduction

### 1.1 Purpose

Durapack is a binary framing format designed for hostile storage and transmission environments where data corruption, loss, and reordering are expected. Each frame is self-locating, self-describing, and cryptographically linkable.

### 1.2 Scope

This specification defines:
- Binary layout of frames
- Encoding and decoding rules
- Error detection and recovery mechanisms
- Scanner behavior under various corruption scenarios

### 1.3 Terminology

- **Frame**: A single unit of data containing a header, payload, and optional trailer
- **Marker**: A fixed byte sequence used for synchronization
- **Scanner**: A decoder that can locate frames in corrupted streams
- **Timeline**: A reconstructed sequence of frames based on bidirectional links

---

## 2. Design Goals

1. **Self-synchronization**: Frames can be located without external context
2. **Local decodability**: Each frame can be decoded independently
3. **Forensic reconstruction**: Timeline can be rebuilt from partial data
4. **Damage resilience**: Survive bit flips, burst errors, and truncation
5. **Deterministic encoding**: Same input always produces same output
6. **Minimal overhead**: Small header size relative to payload
7. **Future-proof**: Versioning and reserved fields for extensions

---

## 3. Frame Structure

### 3.1 Overall Layout

```
┌─────────────┬─────────────┬─────────────┬─────────────┐
│   MARKER    │   HEADER    │   PAYLOAD   │   TRAILER   │
│   4 bytes   │  46 bytes   │   N bytes   │  0-32 bytes │
└─────────────┴─────────────┴─────────────┴─────────────┘
```

### 3.2 Size Limits

| Component | Minimum | Maximum | Notes |
|-----------|---------|---------|-------|
| Frame Size | 54 bytes | 16 MiB | Includes all components |
| Payload Size | 0 bytes | 16 MiB - 1 KiB | Accounts for header/trailer |
| Header Size | 46 bytes | 46 bytes | Fixed in v1.0 |
| Trailer Size | 0 bytes | 32 bytes | Depends on flags |

### 3.3 Byte Order

All multi-byte integers are encoded in **big-endian** (network) byte order.

---

## 4. Header Specification

### 4.1 Marker (4 bytes)

**Offset:** 0-3  
**Value:** `0x44 0x55 0x52 0x50` (ASCII: "DURP")

The marker enables byte-by-byte scanning for frame boundaries in corrupted streams.

**Design rationale:**
- 4 bytes provides good balance between false-positive rate and scanning speed
- ASCII representation aids debugging
- Distinctive pattern reduces natural occurrence in random data

### 4.2 Version (1 byte)

**Offset:** 4  
**Range:** 0-255  
**Current:** 1

Version number indicating frame format version. Decoders MUST reject frames with unsupported versions.

**Version history:**
- `0x00`: Reserved (invalid)
- `0x01`: Initial specification (this document)
- `0x02-0xFF`: Reserved for future use

### 4.3 Frame ID (8 bytes)

**Offset:** 5-12  
**Type:** Unsigned 64-bit integer (big-endian)  
**Range:** 0 to 2^64 - 1

Monotonically increasing sequence number. Frame IDs SHOULD increase sequentially but gaps are allowed (indicating lost frames).

**Special values:**
- `0x0000000000000000`: Valid (first frame in sequence)
- `0xFFFFFFFFFFFFFFFF`: Valid (highest possible frame ID)

### 4.4 Previous Hash (32 bytes)

**Offset:** 13-44  
**Type:** BLAKE3 hash (256 bits)

Hash of the complete previous frame (including marker, header, payload, and trailer). Forms a backward-linking chain for timeline reconstruction.

**Special values:**
- All zeros (`0x00` × 32): First frame in sequence OR orphaned frame

**Computation:**
```
prev_hash = BLAKE3(marker || header || payload || trailer)
```

### 4.5 Payload Length (4 bytes)

**Offset:** 45-48  
**Type:** Unsigned 32-bit integer (big-endian)  
**Range:** 0 to MAX_PAYLOAD_SIZE

Length of payload in bytes. Does NOT include marker, header, or trailer.

**Validation rules:**
1. MUST be ≤ MAX_PAYLOAD_SIZE (16 MiB - 1 KiB)
2. Total frame size MUST be ≤ MAX_FRAME_SIZE (16 MiB)

### 4.6 Flags (1 byte)

**Offset:** 49  
**Type:** Bitfield (8 bits)

```
Bit 7   6   5   4   3   2   1   0
    │   │   │   │   │   │   │   └─ HAS_CRC32C    (0x01)
    │   │   │   │   │   │   └───── HAS_BLAKE3     (0x02)
    │   │   │   │   │   └───────── IS_FIRST       (0x04)
    │   │   │   │   └───────────── IS_LAST        (0x08)
    │   │   │   └───────────────── RESERVED       (0x10)
    │   │   └───────────────────── RESERVED       (0x20)
    │   └───────────────────────── RESERVED       (0x40)
    └───────────────────────────── RESERVED       (0x80)
```

See [Section 5](#5-feature-flags) for detailed flag semantics.

---

## 5. Feature Flags

### 5.1 HAS_CRC32C (0x01)

When set, frame has a 4-byte CRC32C trailer.

**Mutually exclusive with:** HAS_BLAKE3

**Usage:**
- Lightweight integrity check
- Lower CPU overhead than BLAKE3
- Suitable for small payloads or resource-constrained environments

### 5.2 HAS_BLAKE3 (0x02)

When set, frame has a 32-byte BLAKE3 trailer.

**Mutually exclusive with:** HAS_CRC32C

**Usage:**
- Cryptographic integrity check
- Recommended for most use cases
- Required for forensic applications

### 5.3 IS_FIRST (0x04)

Marks this frame as the first in a logical sequence.

**Semantics:**
- Frame with this flag SHOULD have prev_hash = all zeros
- Multiple frames with IS_FIRST indicate multiple independent sequences
- Useful for identifying sequence boundaries after reordering

### 5.4 IS_LAST (0x08)

Marks this frame as the last in a logical sequence.

**Semantics:**
- No frames should reference this frame's hash as their prev_hash (in a clean sequence)
- Useful for detecting truncation
- Multiple frames with IS_LAST indicate multiple independent sequences or branches

### 5.5 Flag Validation Rules

1. **At most one trailer flag** MUST be set (HAS_CRC32C XOR HAS_BLAKE3 XOR neither)
2. **IS_FIRST and IS_LAST** MAY both be set (indicating a single-frame sequence)
3. **Reserved bits** MUST be zero in v1.0 frames
4. Decoders MUST reject frames with invalid flag combinations

---

## 6. Backward Link Semantics

### 6.1 Link Formation

Each frame (except the first) contains a BLAKE3 hash of the previous frame in its `prev_hash` field.

```
Frame N-1              Frame N
┌──────────┐          ┌──────────┐
│  Header  │          │  Header  │
├──────────┤          ├──────────┤
│ frame_id │          │ frame_id │
│ prev_hash│◄─────────│ prev_hash│ = BLAKE3(Frame N-1)
├──────────┤          ├──────────┤
│ Payload  │          │ Payload  │
└──────────┘          └──────────┘
```

### 6.2 Chain Properties

**Immutability**: Modifying any frame breaks the chain from that point forward.

**Tamper Evidence**: Any alteration to frame N-1 will cause frame N's prev_hash verification to fail.

**Gap Detection**: If frame M is missing, frame M+1's prev_hash won't match any known frame.

### 6.3 Timeline Reconstruction Algorithm

```
1. Collect all valid frames
2. Identify frames with IS_FIRST flag or prev_hash = zeros
3. For each starting frame:
   a. Follow forward by finding frames that reference this frame's hash
   b. Build chain until no more references or IS_LAST encountered
4. Identify gaps where no frame matches a prev_hash
5. Report orphaned frames that can't be linked to any chain
```

### 6.4 Special Cases

**Orphaned frames**: Valid frames that don't link to any chain (prev_hash doesn't match any known frame).

**Duplicate frames**: Same frame_id appears multiple times. Implementation SHOULD keep first occurrence and warn.

**Cycles**: Should not occur in well-formed sequences. Implementation MUST detect and report.

**Branches**: Multiple frames with same prev_hash indicate duplication or tampering.

---

## 7. Trailer Variants

### 7.1 No Trailer (0 bytes)

When neither HAS_CRC32C nor HAS_BLAKE3 flags are set.

**Use case:** Maximum space efficiency, external integrity checking.

### 7.2 CRC32C Trailer (4 bytes)

**Algorithm:** CRC-32C (Castagnoli) polynomial 0x1EDC6F41

**Computation:**
```
crc = CRC32C(marker || header || payload)
trailer = crc (4 bytes, big-endian)
```

**Layout:**
```
Offset  Size  Field
0       4     CRC32C checksum
```

**Verification:**
1. Recompute CRC over marker, header, and payload
2. Compare with trailer value
3. Accept if match, reject otherwise

### 7.3 BLAKE3 Trailer (32 bytes)

**Algorithm:** BLAKE3 cryptographic hash

**Computation:**
```
hash = BLAKE3(marker || header || payload)
trailer = hash (32 bytes)
```

**Layout:**
```
Offset  Size  Field
0       32    BLAKE3 hash
```

**Verification:**
1. Recompute BLAKE3 over marker, header, and payload
2. Compare with trailer value (constant-time comparison recommended)
3. Accept if match, reject otherwise

### 7.4 Trailer Selection Guidelines

| Scenario | Recommended | Rationale |
|----------|-------------|-----------|
| Small frames (<256B) | CRC32C | Lower overhead ratio |
| Large frames (>1KB) | BLAKE3 | Better security, minimal overhead |
| Cryptographic use | BLAKE3 | Required for security properties |
| Resource-constrained | CRC32C | Lower CPU/memory requirements |
| Forensic applications | BLAKE3 | Tamper evidence |

---

## 8. Alignment and Padding Rules

### 8.1 Frame Alignment

Frames have **no mandatory alignment** requirement. They may start at any byte offset.

**Rationale:** Allows frames to be tightly packed, reducing overhead.

### 8.2 Payload Alignment

Payload begins immediately after the header (offset 50 from frame start).

**No padding** is inserted between header and payload.

### 8.3 Trailer Alignment

Trailer (if present) begins immediately after payload with **no padding**.

### 8.4 Multi-Frame Streams

When concatenating frames into a stream:
```
Frame 1 | Frame 2 | Frame 3 | ...
```

**No padding or delimiters** between frames. Next frame's marker immediately follows previous frame's trailer (or payload if no trailer).

### 8.5 File Format

A Durapack file is simply a concatenation of frames:
```
File := Frame+
```

**No file header** or footer. First byte of file MUST be start of a frame marker.

---

## 9. Reserved Fields

### 9.1 Flag Bits

Bits 4-7 of the flags field are **reserved** for future use.

**Current behavior:**
- Encoders MUST set reserved bits to 0
- Decoders MUST reject frames with reserved bits set to 1

**Future behavior:**
- New features may define semantics for these bits in future versions
- Decoders for v1.0 will reject frames using future features (fail-safe)

### 9.2 Version Number Space

Version numbers 2-255 are reserved for future format versions.

**Compatibility strategy:**
- Minor changes (e.g., new flags): Same version, reserved bits
- Major changes (e.g., header layout): New version number
- Decoders SHOULD support multiple versions when feasible

### 9.3 Extension Mechanism

Future versions MAY define extension headers by:
1. Incrementing version number
2. Using reserved flag bits to indicate extension presence
3. Adding extension data between header and payload

**Example future extension:**
```
Version 2: Add optional metadata section
- Use reserved bit 4 (HAS_METADATA)
- Insert metadata between header and payload
- Update payload_len semantics
```

---

## 10. Corruption Taxonomy

This section defines types of corruption and their effects on frame recovery.

### 10.1 Bit Flips

**Description:** Individual bits changed from 0→1 or 1→0.

**Effects:**
- In marker: Frame becomes undetectable by standard scan
- In header: May cause parsing errors or incorrect metadata
- In payload: Detected by CRC/BLAKE3, frame rejected
- In trailer: Causes verification failure, frame rejected

**Recovery:**
- Scanner continues byte-by-byte search for next valid marker
- Frames before/after corrupted frame may be recoverable

### 10.2 Burst Errors

**Description:** Contiguous sequence of corrupted bytes.

**Effects:**
- Small bursts (<4 bytes): Similar to bit flips
- Medium bursts (4-50 bytes): May destroy marker and header
- Large bursts (>50 bytes): Multiple frames lost

**Recovery:**
- Scanner skips corrupted region until next valid marker found
- Timeline reconstruction identifies gaps via frame_id discontinuities
- prev_hash chain shows break in sequence

### 10.3 Truncation

**Description:** Data ends prematurely (file/transmission cut off).

**Effects:**
- Last frame incomplete (partial header, payload, or trailer)
- All frames before truncation point may be recoverable

**Recovery:**
- Scanner detects insufficient bytes for expected frame size
- Last partial frame rejected
- Timeline shows IS_LAST flag missing (if expected)
- Gap detection identifies final frame before truncation

### 10.4 Insertion

**Description:** Extra bytes inserted into stream.

**Effects:**
- If inserted between frames: Scanner finds next marker, minimal impact
- If inserted within frame: Payload/trailer desynchronized
- If inserted contains valid marker: False frame detected

**Recovery:**
- Authentic frames still recoverable via marker scanning
- False frames rejected by checksum/hash verification
- Timeline reconstruction filters invalid frames

### 10.5 Deletion

**Description:** Bytes removed from stream.

**Effects:**
- Frame boundaries misaligned
- Payload data shifted into trailer position
- Subsequent frames desynchronized

**Recovery:**
- Scanner resynchronizes at next valid marker
- Affected frame(s) lost
- Frames after resync point may be recoverable

### 10.6 Duplication

**Description:** Frames repeated in stream.

**Effects:**
- Same frame_id appears multiple times
- Timeline reconstruction ambiguous

**Recovery:**
- Decoders SHOULD keep first occurrence by default
- Alternative: Keep frame with highest prev_hash matches
- Timeline shows duplicate frame warning

### 10.7 Reordering

**Description:** Frames arrive/stored out of sequence.

**Effects:**
- frame_id not monotonic
- prev_hash references frames not yet seen

**Recovery:**
- Collect all frames first
- Sort by frame_id
- Timeline reconstruction handles out-of-order via hash linking
- This is a **design feature**, not a bug

---

## 11. Scanner Behavior

### 11.1 Basic Scanning Algorithm

```
offset = 0
while offset < data.len():
    # Search for marker
    marker_pos = find_next_marker(data, offset)
    if marker_pos == None:
        break
    
    # Attempt to decode frame
    frame = try_decode_frame(data, marker_pos)
    if frame.is_valid():
        emit(frame, marker_pos)
        offset = marker_pos + frame.total_size()
    else:
        offset = marker_pos + 1  # Try next byte
```

### 11.2 Marker Detection

**Requirement:** Scanner MUST search byte-by-byte for marker `0x44555250`.

**Optimization:** Implementations MAY use Boyer-Moore or similar algorithms for faster scanning.

### 11.3 Frame Validation

After finding a marker, scanner MUST validate:

1. **Version check:** Version field == 1
2. **Length check:** payload_len ≤ MAX_PAYLOAD_SIZE
3. **Flag check:** No reserved bits set, valid flag combination
4. **Size check:** Sufficient bytes available for complete frame
5. **Checksum/hash:** If trailer present, verify integrity

**Validation order matters:** Check cheaper validations first (version, length) before expensive ones (hash computation).

### 11.4 Partial Frame Handling

If insufficient bytes remain for complete frame:
- **In strict mode:** Reject frame, treat as truncation
- **In recovery mode:** Mark as incomplete, include in report with offset

### 11.5 False Marker Handling

If marker found but validation fails:
- **Continue scanning** from next byte (marker_pos + 1)
- **Do NOT assume** marker was genuine
- **Track statistics:** false marker count for debugging

### 11.6 Scanner Output

Scanner MUST output:

```
struct LocatedFrame {
    offset: u64,        // Byte offset of marker in stream
    frame: Frame,        // Decoded frame data
}

struct ScanStatistics {
    bytes_scanned: u64,
    markers_found: u64,
    frames_found: u64,
    decode_failures: u64,
    truncations: u64,
}
```

### 11.7 Performance Considerations

**Expected throughput:** 500-1000 MB/s on modern hardware (scanning + validation).

**Memory usage:** O(1) for scanning, O(n) for storing results.

**Optimization opportunities:**
- SIMD for marker search
- Parallel scanning of multiple regions
- Incremental hash computation

---

## 12. Versioning and Compatibility

### 12.1 Version Policy

**Semantic versioning for format:**
- **Major version** (in version field): Breaking changes to layout
- **Minor version** (via reserved flags): Backward-compatible features
- **Patch version** (spec document): Clarifications, no format changes

### 12.2 Forward Compatibility

Version 1 decoders encountering future versions MUST:
1. Check version field
2. If version > 1, reject frame
3. Report unsupported version error
4. Continue scanning for v1 frames

### 12.3 Backward Compatibility

Future versions SHOULD:
- Maintain marker format for scanability
- Keep version field at same offset
- Use reserved bits before changing layout
- Provide migration tools for existing data

### 12.4 Interoperability

Implementations MUST:
- Support v1.0 encoding/decoding
- Produce deterministic output (same input → same bytes)
- Handle all corruption types gracefully
- Provide standard test vectors

---

## 13. Implementation Requirements

### 13.1 Mandatory Features

All implementations MUST support:
- [ ] Frame encoding with CRC32C trailer
- [ ] Frame encoding with BLAKE3 trailer
- [ ] Frame decoding with validation
- [ ] Scanner for corrupted streams
- [ ] Timeline reconstruction
- [ ] Gap detection

### 13.2 Optional Features

Implementations MAY support:
- [ ] Compression (applied to payload before framing)
- [ ] Encryption (applied to payload before framing)
- [ ] Forward error correction (FEC)
- [ ] Streaming API for large files
- [ ] Parallel scanning

### 13.3 Quality Requirements

- **No panics/crashes** on malformed input
- **Constant-time** comparison for BLAKE3 hashes
- **Secure random** for any random number generation
- **Clear error messages** with context

### 13.4 Testing Requirements

Implementations MUST pass:
- [ ] Round-trip tests (encode → decode)
- [ ] Corruption recovery tests (all taxonomy types)
- [ ] Timeline reconstruction tests
- [ ] Property-based tests (fuzzing)
- [ ] Standard test vectors (Section 14)

---

## 14. Test Vectors

**Complete Test Vector Gallery:** See `test_vectors/` directory for comprehensive binary test files covering all corruption scenarios.

**Test Vector Repository:** `test_vectors/README.md` contains a complete index with 13 test vectors covering:
- Clean frames (minimal, CRC32C, BLAKE3, linked sequences)
- All corruption types (bit flips, burst errors, truncation, insertion, deletion, duplication, reordering)
- Expected behaviors and recovery rates

### 14.1 Minimal Frame (No Trailer)

```
Description: Smallest valid frame
Frame ID: 1
Prev Hash: All zeros
Payload: Empty
Flags: 0x04 (IS_FIRST)
Trailer: None

Hex Encoding:
44 55 52 50 01 00 00 00 00 00 00 00 01 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 
00 00 00 00 00 00 00 00 04

Total size: 50 bytes
```

### 14.2 Frame with CRC32C

```
Description: Frame with short payload and CRC32C
Frame ID: 1
Prev Hash: All zeros
Payload: "Hello, Durapack!" (16 bytes)
Flags: 0x05 (IS_FIRST | HAS_CRC32C)
CRC32C: 0xABCD1234 (example value)

Total size: 70 bytes
```

### 14.3 Frame with BLAKE3

```
Description: Frame with payload and BLAKE3
Frame ID: 2
Prev Hash: BLAKE3 of previous frame
Payload: "Frame 2 data" (12 bytes)
Flags: 0x02 (HAS_BLAKE3)

Total size: 108 bytes
```

### 14.4 Corrupted Stream

```
Description: Stream with bit flip in middle frame

Original Stream: Frame1 | Frame2 | Frame3
Corruption: Flip bit in Frame2 payload

Expected Scanner Output:
- Frame1: Valid (offset 0)
- Frame2: Invalid checksum
- Frame3: Valid (offset X)

Timeline Output:
- Frames: [Frame1, Frame3]
- Gaps: [{before: 1, after: 3}]
```

### 14.5 Reordered Stream

```
Description: Frames arrive out of order

Physical Order: Frame3 | Frame1 | Frame2
Logical Order: Frame1 | Frame2 | Frame3

Expected Timeline Output:
- Correctly reconstructed sequence: Frame1 → Frame2 → Frame3
- No gaps detected
```

---

## 15. Interleaving Guidance (Burst-error Mitigation)

This section is informative and defines a recommended application-level technique to reduce the impact of burst errors without changing the on-disk frame format.

### 15.1 Motivation

Burst errors (e.g., tape dropouts, RF fades, disk bad blocks) tend to corrupt contiguous runs of bytes. If an application writes large payloads into single frames, a burst can destroy the entire payload. By striping the payload across multiple consecutive frames, a burst damages only small portions of several frames, improving the chance that useful data survives and can be recovered.

### 15.2 Writer-side Interleaving

Applications may split a contiguous byte stream into `group` stripes in round-robin blocks of `shard_len` bytes and then place each stripe into consecutive frames. The Durapack core provides helpers:

- `durapack_core::interleave::InterleaveParams { group, shard_len }`
- `durapack_core::interleave::interleave_bytes(&data, params) -> Vec<Bytes>`

Emit the resulting stripes over `group` consecutive frames, in lane order (0..group-1). Include the `group` and `shard_len` values in your metadata (e.g., in a superframe index or application header) so readers can reassemble.

### 15.3 Reader-side Deinterleaving

To reconstruct the original contiguous byte stream, collect the stripes in lane order and call:

- `durapack_core::interleave::deinterleave_bytes(&stripes, params) -> Bytes`

The function pulls blocks of `shard_len` from each lane in round-robin order until all stripes are consumed, yielding the original data.

### 15.4 Compatibility

- The Durapack on-disk format is unchanged; interleaving is an application-level technique.
- Interleaving parameters should be discoverable (e.g., stored in application metadata or superframe summaries) to ensure interoperability.
- Readers that do not implement deinterleaving will still decode individual frames normally but will see striped payloads.

---

## 16. Appendix A: BLAKE3 Hash Computation

### Reference Implementation

```rust
use blake3;

fn compute_frame_hash(marker: &[u8; 4], 
                      header: &[u8; 46], 
                      payload: &[u8],
                      trailer: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(marker);
    hasher.update(header);
    hasher.update(payload);
    hasher.update(trailer);
    hasher.finalize().into()
}
```

### Test Vector

```
Input: Empty frame (marker + header with zeros + empty payload)
Output: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        (BLAKE3 of empty input)
```

---

## 17. Appendix B: CRC32C Computation

### Polynomial

CRC-32C (Castagnoli): `0x1EDC6F41`

### Reference Implementation

```rust
use crc32c;

fn compute_crc32c(marker: &[u8; 4], 
                  header: &[u8; 46], 
                  payload: &[u8]) -> u32 {
    let mut crc = crc32c::Hasher::new();
    crc.update(marker);
    crc.update(header);
    crc.update(payload);
    crc.finalize()
}
```

### Test Vector

```
Input: "Hello, World!" (13 bytes)
Output: 0xC99465AA
```

---

## 18. Appendix C: Corruption Detection Probability

### False Marker Rate

Probability of random data containing "DURP" marker:

```
P(false_marker) = (1/256)^4 = 1 / 4,294,967,296 ≈ 2.3 × 10^-10
```

**Expected false markers in 1 GB random data:** ~0.23

### CRC32C Detection Rate

- Single bit flip: 100%
- 2 bit flips: 100%
- Odd number of bit flips: 100%
- Burst ≤ 32 bits: 100%
- Random corruption: 99.9999999767% (1 - 1/2^32)

### BLAKE3 Detection Rate

- Any alteration: ~100% (collision resistance: 2^-128)

---

## 19. Change Log

### Version 1.0 (November 1, 2025)
- Initial specification
- Defined v1.0 frame format
- Documented corruption taxonomy
- Specified scanner behavior
- Added test vectors

---

## 20. References

1. BLAKE3 Specification: https://github.com/BLAKE3-team/BLAKE3-specs
2. CRC-32C (Castagnoli) RFC: RFC 3720
3. CCSDS Space Packet Protocol: CCSDS 133.0-B-2
4. DTN Bundle Protocol: RFC 9171

---

## 21. License

This specification document is released under:
- Creative Commons Attribution 4.0 International (CC BY 4.0)

Implementations may be released under any license compatible with the reference implementation.

---

**End of Specification**
