# Test Vector 5: Truncated Frame

**File:** 05_truncated_frame.durp
**Size:** 30 bytes (truncated from 82 bytes)
**Description:** Frame truncated in the middle of the header

## Corruption Details
- Original size: 82 bytes
- Truncated at: byte 30
- Corruption type: TRUNCATION
- Severity: Frame unrecoverable

## Expected Behavior
- Scanner: MUST detect incomplete frame
- Decoder: MUST reject (insufficient data)
- Recovery: No valid frames found

## Hex Dump
```
445552500100000000000000010000000000000000000000000000000000
```
