# Test Vector 13: Reordered Frames with Hash Links

**File:** 13_reordered_frames.durp
**Size:** 374 bytes
**Description:** 4 linked frames stored in scrambled order

## Frame Details
- Logical order: 1 → 2 → 3 → 4
- Physical order: 3, 1, 4, 2
- All frames have prev_hash links

## Corruption Details
- Corruption type: REORDERING (intentional)
- Severity: None (timeline reconstruction handles this)

## Expected Behavior
- Scanner: MUST find all 4 frames
- Timeline: MUST reconstruct correct order via hash links
- Result: 1 → 2 → 3 → 4
- No gaps, no orphans
- Recovery rate: 100%

## Hex Dump
```
445552500100000000000000034f87cbc73b4abae3301f4d438c196a492c81ba9feb9e267b4d863518cf489c030000000b025468697264206672616d65c89d53a146c1988272f2c161faa663811220cde997dec6e8f4c32b329c3c46114455525001000000000000000100000000000000000000000000000000000000000000000000000000000000000000000b064669727374206672616d65c6785c38c813147019dfc841139b684831f2fcf20260820eff69d8eb740c0ab4445552500100000000000000045ca4b873f6fa4b1e4e0ef9fb10b1856580052bfc4775dc58d2e33602552bf1b30000000c0a466f75727468206672616d655f201b182cf6198de3dcaa34f24deb173e39d797aaa176e7f0bfbda29da323f544555250010000000000000002a5be97688f6d82b8a966a10a3bdd0bd2b1c658d278e704a523045c950df497e10000000c025365636f6e64206672616d655240f199a9aa2352f2854c0117e83aae879a2d81fad6aa680700a10175ebcb79
```
