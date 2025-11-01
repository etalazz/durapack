# Test Vector 6: Bit Flip Error

**File:** 06_bit_flip_error.durp
**Size:** 80 bytes
**Description:** Single bit flipped in payload

## Corruption Details
- Corruption type: BIT FLIP
- Location: Byte 60 (within frame)
- Bit flipped: 0x01 (LSB)
- Severity: Frame detectable but invalid

## Expected Behavior
- Scanner: MUST find frame via marker
- Decoder: MUST reject (CRC32C mismatch)
- Error: ChecksumMismatch
- Recovery: Frame lost

## Hex Dump
```
4455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000001a014461746120776974682072696e676c6520626974206572726f72903966d1
```

## Diff from Clean
Original byte 60: 0x73
Corrupted byte 60: 0x72
