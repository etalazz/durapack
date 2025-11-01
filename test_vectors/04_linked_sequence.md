# Test Vector 4: Linked Sequence

**File:** 04_linked_sequence.durp
**Size:** 280 bytes
**Description:** Three frames linked via BLAKE3 hash chain

## Frame Details
### Frame 1
- Frame ID: 1
- Prev Hash: All zeros
- Payload: "First frame" (11 bytes)
- Flags: 0x06 (IS_FIRST | HAS_BLAKE3)

### Frame 2
- Frame ID: 2
- Prev Hash: BLAKE3(Frame 1)
- Payload: "Second frame" (12 bytes)
- Flags: 0x02 (HAS_BLAKE3)

### Frame 3
- Frame ID: 3
- Prev Hash: BLAKE3(Frame 2)
- Payload: "Third frame" (11 bytes)
- Flags: 0x0A (IS_LAST | HAS_BLAKE3)

## Expected Behavior
- Scanner: MUST find exactly 3 frames
- Timeline: Complete chain 1 → 2 → 3
- No gaps or orphans
- All back-links MUST verify

## Hex Dump
```
4455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000000b064669727374206672616d65c6785c38c813147019dfc841139b684831f2fcf20260820eff69d8eb740c0ab444555250010000000000000002a5be97688f6d82b8a966a10a3bdd0bd2b1c658d278e704a523045c950df497e10000000c025365636f6e64206672616d655240f199a9aa2352f2854c0117e83aae879a2d81fad6aa680700a10175ebcb79445552500100000000000000034f87cbc73b4abae3301f4d438c196a492c81ba9feb9e267b4d863518cf489c030000000b0a5468697264206672616d6595623df8f17a5c5de9489968bd90e3376b1cd2740cb3f1b5c45213b1c8e47f45
```
