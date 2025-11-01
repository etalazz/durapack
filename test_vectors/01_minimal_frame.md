# Test Vector 1: Minimal Frame

**File:** 01_minimal_frame.durp
**Size:** 50 bytes
**Description:** Smallest valid frame with no payload and no trailer

## Frame Details
- Frame ID: 1
- Prev Hash: All zeros
- Payload: Empty (0 bytes)
- Flags: 0x04 (IS_FIRST)
- Trailer: None

## Expected Behavior
- Decoder: MUST accept
- Scanner: MUST find exactly 1 frame
- Timeline: Single frame, no gaps

## Hex Dump
```
4455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000000004
```
