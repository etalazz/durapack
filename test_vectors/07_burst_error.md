# Test Vector 7: Burst Error

**File:** 07_burst_error.durp
**Size:** 300 bytes
**Description:** 50-byte burst error destroying middle frame

## Corruption Details
- Corruption type: BURST ERROR
- Location: Bytes 100 to 150
- Length: 50 bytes
- Affected: Frame 2 (completely destroyed)
- Severity: One frame lost

## Expected Behavior
- Scanner: MUST find 2 valid frames (1 and 3)
- Frame 2: Unrecoverable
- Timeline: Gap detected between frame 1 and 3
- Recovery rate: 66.7% (2/3 frames)

## Hex Dump
```
44555250010000000000000001000000000000000000000000000000000000000000000000000000000000000000000012024672616d65206265666f726520627572737410680e28e1a9438ab2ac4f64d269458e9f443e03afa7df268937b0bc0fc50647ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff4672616d6520696e206275727374207a6f6e6528ef650ff4aed75a502a8c0830dbb89cb1840a9e314a7727eb0ae2ae3d5a701c44555250010000000000000003000000000000000000000000000000000000000000000000000000000000000000000011024672616d652061667465722062757273746aec1479d42ffdcf59eed82c6e8791179483cc49e73351b3053f22592ad28889
```
