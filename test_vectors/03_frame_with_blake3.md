# Test Vector 3: Frame with BLAKE3

**File:** 03_frame_with_blake3.durp
**Size:** 104 bytes
**Description:** Frame with payload and BLAKE3 cryptographic hash

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: "Frame with BLAKE3 hash" (22 bytes)
- Flags: 0x06 (IS_FIRST | HAS_BLAKE3)
- Trailer: BLAKE3 (32 bytes)

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- BLAKE3 verification: MUST pass

## Hex Dump
```
44555250010000000000000001000000000000000000000000000000000000000000000000000000000000000000000016064672616d65207769746820424c414b453320686173680ce8583cdbee7a0b1c58b1ff2333293189baa1f37533da37bd6a4da3368e3f30
```
