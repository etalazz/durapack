# Test Vector 2: Frame with CRC32C

**File:** 02_frame_with_crc32c.durp
**Size:** 70 bytes
**Description:** Frame with short payload and CRC32C integrity check

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: "Hello, Durapack!" (16 bytes)
- Flags: 0x05 (IS_FIRST | HAS_CRC32C)
- Trailer: CRC32C (4 bytes)

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- CRC verification: MUST pass

## Hex Dump
```
445552500100000000000000010000000000000000000000000000000000000000000000000000000000000000000000100548656c6c6f2c20447572617061636b218cc6359f
```
