# Test Vector 12: Duplicate Frames

**File:** 12_duplicate_frames.durp
**Size:** 196 bytes
**Description:** Frame 1 appears twice in the stream

## Corruption Details
- Corruption type: DUPLICATION
- Duplicated: Frame 1
- Occurrences: 2 (byte 0 and byte 131)
- Severity: Minor (deduplication required)

## Expected Behavior
- Scanner: MUST find 3 frame instances
- Timeline: SHOULD keep first occurrence of frame 1
- Warning: Duplicate frame ID detected
- Effective frames: 2 (frame 1 and frame 2)

## Hex Dump
```
4455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000000b014669727374206672616d65c2eeadd74455525001000000000000000200000000000000000000000000000000000000000000000000000000000000000000000c015365636f6e64206672616d65fecf4d144455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000000b014669727374206672616d65c2eeadd7
```
