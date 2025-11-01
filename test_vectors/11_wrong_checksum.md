# Test Vector 11: Wrong Checksum

**File:** 11_wrong_checksum.durp
**Size:** 79 bytes
**Description:** Frame with intentionally corrupted CRC32C trailer

## Corruption Details
- Corruption type: WRONG CHECKSUM
- Location: CRC32C trailer (last 4 bytes)
- Corruption: XOR with 0xFFFF0000
- Severity: Frame detectable but invalid

## Expected Behavior
- Scanner: MUST find frame via marker
- Decoder: MUST reject (ChecksumMismatch)
- Error message: MUST indicate expected vs actual checksum
- Recovery: Frame lost

## Hex Dump
```
44555250010000000000000001000000000000000000000000000000000000000000000000000000000000000000000019014672616d6520776974682077726f6e6720636865636b73756d72ad3659
```
