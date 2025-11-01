# Test Vector 8: Inserted Garbage

**File:** 08_inserted_garbage.durp
**Size:** 247 bytes
**Description:** 100 bytes of garbage inserted between frames

## Corruption Details
- Corruption type: INSERTION
- Location: Between frame 1 and frame 2
- Inserted: 100 bytes (0xAA repeated)
- Severity: Minimal (frames still recoverable)

## Expected Behavior
- Scanner: MUST find 2 valid frames
- Scanner: MUST skip garbage via marker search
- Timeline: 2 frames, may detect gap (depending on IDs)
- Recovery rate: 100%

## Hex Dump (first 200 bytes)
```
44555250010000000000000001000000000000000000000000000000000000000000000000000000000000000000000014014672616d65206265666f726520676172626167656d815820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4455525001000000000000000200000000000000000000000000
```
