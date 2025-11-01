# Durapack Frame Specification

Version: 1.0  
Date: 2025-11-01

## 1. Overview

Durapack is a self-locating, bidirectionally-linkable framing format designed for hostile or partially damaged media. Each frame is independent and can be identified and decoded starting from any byte offset in a stream.

## 2. Frame Structure

A Durapack frame consists of four parts:

```
+----------+----------+---------+----------+
| MARKER   | HEADER   | PAYLOAD | TRAILER  |
+----------+----------+---------+----------+
  4 bytes    46 bytes   N bytes   0-32 bytes
```

### 2.1 Marker (4 bytes)

Fixed byte sequence for synchronization:

```
Bytes: 0x44 0x55 0x52 0x50 (ASCII: "DURP")
```

The marker enables byte-by-byte scanning of damaged streams.

### 2.2 Header (46 bytes)

```
Offset | Size | Field        | Type    | Description
-------|------|--------------|---------|---------------------------
0      | 1    | version      | u8      | Protocol version (1)
1      | 8    | frame_id     | u64     | Unique frame identifier
9      | 32   | prev_hash    | [u8;32] | BLAKE3 hash of previous frame
41     | 4    | payload_len  | u32     | Payload length in bytes
45     | 1    | flags        | u8      | Frame flags (see below)
```

All multi-byte integers are encoded in **big-endian** byte order.

#### 2.2.1 Version

Current version: `1`

Future versions may extend the header or change the format. Decoders MUST reject frames with unsupported versions.

#### 2.2.2 Frame ID

64-bit unsigned integer uniquely identifying this frame. IDs should be sequential in the expected timeline, but the decoder makes no assumptions about continuity.

#### 2.2.3 Previous Hash

32-byte BLAKE3 hash of the complete previous frame (marker + header + payload, excluding previous frame's trailer).

For the **first frame** in a sequence, this field MUST be all zeros: `[0x00; 32]`.

This field enables:
- Bidirectional linking of frames
- Detection of missing frames
- Verification of sequence integrity

#### 2.2.4 Payload Length

32-bit unsigned integer specifying payload size in bytes.

Maximum value: 16,776,192 (16 MB - 1 KB overhead)

#### 2.2.5 Flags

Single byte containing frame options:

```
Bit | Mask | Name         | Description
----|------|--------------|----------------------------------
0   | 0x01 | HAS_CRC32C   | Frame has CRC32C trailer
1   | 0x02 | HAS_BLAKE3   | Frame has BLAKE3 trailer
2   | 0x04 | IS_FIRST     | First frame in sequence
3   | 0x08 | IS_LAST      | Last frame in sequence
4-7 | 0xF0 | (reserved)   | Reserved for future use
```

If both `HAS_CRC32C` and `HAS_BLAKE3` are set, `HAS_BLAKE3` takes precedence.

### 2.3 Payload (Variable Length)

Application-specific data. Can be:
- Raw binary data
- JSON
- CBOR
- Protocol Buffers
- Any other format

The payload is treated as opaque bytes by the framing layer.

### 2.4 Trailer (0, 4, or 32 bytes)

Optional integrity check, determined by flags:

#### No Trailer (flags = 0x00)
No trailer bytes.

#### CRC32C Trailer (flags = 0x01)
4 bytes: CRC32C checksum of (marker + header + payload)

#### BLAKE3 Trailer (flags = 0x02)
32 bytes: BLAKE3 hash of (marker + header + payload)

The trailer provides end-to-end integrity checking for the frame.

## 3. Frame Size Limits

- Minimum frame size: 50 bytes (marker + header + 0-byte payload)
- Maximum frame size: 16 MB
- Maximum payload size: ~16 MB - 1 KB

Frames exceeding the maximum MUST be rejected.

## 4. Encoding Rules

### 4.1 Deterministic Encoding

Encoders MUST:
1. Use big-endian byte order for all multi-byte integers
2. Compute prev_hash over the complete previous frame (excluding its trailer)
3. Compute trailer (if present) over marker + header + payload

### 4.2 Hash Computation

BLAKE3 hashes are computed as:

```
hash = BLAKE3(marker || header || payload)
```

Where `||` denotes concatenation.

## 5. Decoding Rules

### 5.1 Strict Mode

Strict decoders MUST:
1. Validate marker matches exactly
2. Validate version is supported
3. Validate payload_len does not exceed maximum
4. Validate trailer (if present) matches computed value
5. Return error on any validation failure

### 5.2 Scan Mode

Scan mode is used for damaged streams:
1. Search byte-by-byte for marker
2. Attempt to parse header at each marker location
3. Validate payload_len is reasonable (< MAX_FRAME_SIZE)
4. Attempt to decode full frame
5. Collect successfully decoded frames
6. Continue scanning after failures

Scan mode MUST NOT panic on invalid data.

## 6. Timeline Reconstruction

### 6.1 Linking Algorithm

1. Find first frame (prev_hash = all zeros)
2. Compute hash of first frame
3. Find next frame where prev_hash matches computed hash
4. Repeat until no matching frame found
5. Detect gaps where expected frame is missing

### 6.2 Gap Detection

A gap exists when:
- No frame has prev_hash matching the expected hash
- Frame IDs are non-sequential
- Hash chain is broken

### 6.3 Orphan Frames

Frames that cannot be linked to the main chain are marked as orphans. These may belong to:
- A different sequence
- A parallel timeline
- Damaged frames with corrupted prev_hash

## 7. Versioning Policy

### 7.1 Version 1

Current specification.

### 7.2 Future Versions

Version increments indicate:
- **Major version** (e.g., 1 → 2): Incompatible frame format changes
- Decoders SHOULD reject unknown versions
- Encoders SHOULD NOT mix versions in a single stream

## 8. Security Considerations

### 8.1 Resource Exhaustion

Decoders MUST enforce maximum frame size to prevent memory exhaustion attacks.

### 8.2 Hash Collisions

BLAKE3 provides 256-bit collision resistance. Practical collision attacks are infeasible.

### 8.3 Authentication

This specification does not include frame authentication (signatures). Applications requiring authentication should:
- Sign payloads at the application layer
- Use authenticated encryption for payloads
- Extend the format with signature fields (future version)

## 9. Examples

### 9.1 Minimal Frame (First Frame)

```
Marker:       44 55 52 50                      ("DURP")
Version:      01                                (1)
Frame ID:     00 00 00 00 00 00 00 01          (1)
Prev Hash:    00 00 00 00 ... (32 zeros)       (first frame)
Payload Len:  00 00 00 04                      (4 bytes)
Flags:        05                                (CRC32C + IS_FIRST)
Payload:      74 65 73 74                      ("test")
Trailer:      XX XX XX XX                      (CRC32C checksum)
```

### 9.2 Linked Frame (Second Frame)

```
Marker:       44 55 52 50
Version:      01
Frame ID:     00 00 00 00 00 00 00 02          (2)
Prev Hash:    [32-byte BLAKE3 hash of frame 1]
Payload Len:  00 00 00 05
Flags:        01                                (CRC32C)
Payload:      68 65 6C 6C 6F                   ("hello")
Trailer:      YY YY YY YY                      (CRC32C checksum)
```

## 10. Compliance

A compliant Durapack implementation MUST:
- ✓ Encode frames according to section 4
- ✓ Decode frames in strict mode according to section 5.1
- ✓ Enforce all size limits
- ✓ Use big-endian byte order
- ✓ Compute hashes correctly
- ✓ Validate checksums when present

A compliant implementation SHOULD:
- ✓ Implement scan mode for damaged streams
- ✓ Implement timeline reconstruction
- ✓ Provide diagnostics for gaps and orphans

## 11. References

- BLAKE3: https://github.com/BLAKE3-team/BLAKE3
- CRC32C: RFC 3720, RFC 4960
- Big-endian: IEEE Std 1003.1

---

*This specification is part of the Durapack project.*

