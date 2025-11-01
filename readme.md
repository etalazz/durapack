# Durapack: A Bidirectional, Self-Locating Framing Format for Hostile or Partially Damaged Media

## Abstract

Durapack is a Rust prototype for encoding telemetry, audit, or mission data so that it remains **recoverable even when the storage or link is damaged**. Each Durapack record (“frame”) is **self-locating** (has a strong marker), **self-describing** (carries its own header/length), and **bidirectionally linkable** (can be re-threaded using IDs or hashes). This allows reconstruction of timelines from unordered or incomplete data — a property conventional linear logs and transport-dependent formats lack.

## 1. Motivation

Operational domains such as space, aerospace, defense, disaster recovery, and field robotics cannot assume:

* perfect links,
* intact media,
* or preserved catalogs.

A usable format must let an analyst start **in the middle** of a capture, detect valid frames, and reconstruct as much as possible. Durapack addresses this with per-frame structure.

## 2. Design Goals

* **Self-synchronization**: detect frame boundaries in noisy/damaged streams.
* **Local decodability**: parse a single frame without external schema files.
* **Bidirectional reconstruction**: reassemble timelines using forward/back references.
* **FEC-ready layout**: payload kept separable for erasure coding.
* **Small, auditable Rust core**.

## 3. Frame Model

Each Durapack frame consists of:

1. **Marker**

    * Fixed 4–8 byte sequence (e.g. `DURP`).
    * Enables byte-by-byte scanning.

2. **Header**

    * `version`
    * `frame_id` (u64)
    * backward link (`prev_id` or `prev_hash`)
    * `payload_len`
    * optional `schema_id`
    * Makes the frame self-describing.

3. **Payload**

    * Application data (telemetry sample, log entry, sensor chunk).
    * Can be structured (JSON/CBOR/Protobuf) or raw.

4. **Trailer / Integrity**

    * CRC or hash (future: signature).
    * Detects corruption at frame level.

With this layout, a decoder can enter at any offset, find `Marker`, read a known-size header, and jump over the payload.

## 4. Recovery Model

Durapack decoding proceeds in three passes:

1. **Scan:** walk the byte stream and collect all byte ranges that look like frames (correct marker + plausible length).
2. **Validate:** re-parse those candidates; discard ones that fail integrity checks.
3. **Re-thread:** use the pair (`frame_id`, `prev_id|prev_hash`) to rebuild a timeline. Missing frames become gaps, but the rest of the sequence stays usable.

This model works even when:

* the **start** of the file is missing,
* frames have been **reordered**,
* or large spans have been **physically destroyed**.

## 5. Testing Strategy

Durapack’s effectiveness is shown by tests that **deliberately break** the data:

1. Encode → decode (round trip).
2. Delete random segments from the stream → decoder must still find surviving frames.
3. Shuffle frames → re-threader must order them again.
4. Drop 10–50% of frames → measure recovered portion (with/without FEC).
5. Fuzz decoder with random bytes → no panic, no infinite loops.

These tests approximate space/military/field conditions where retransmission is expensive or impossible.

## 6. Target Use Cases

1. **Probe / rover / satellite data packs**

    * Downlinks may be partial; Durapack lets ground reconstruct from what arrived.

2. **Black-box / crash forensics media**

    * After impact, investigators scan remaining bytes and still get a timeline.

3. **Tactical / intermittent networks**

    * Units exchange partial captures; HQ later stitches them together.

4. **Distributed, human-capturable archives**

    * Frames can be printed or photographed; self-delimiting structure helps reconstruction.

## 7. Why Durapack vs. Existing Solutions

* **Versus plain logs:** Durapack does not need the first byte to be intact.
* **Versus transport reliability (TCP/QUIC):** Durapack protects *after* the data leaves the network layer (on disk, on flash, on debris).
* **Versus pure FEC:** FEC fixes missing bytes, but Durapack also tells you *where frames start and end*.
* **Versus ad-hoc binary dumps:** Durapack can be scanned, validated, and partially reconstructed by tools that know nothing about the original application.

## 8. Limitations

* Per-frame overhead reduces raw goodput.
* Congestion control and retransmission are **out of scope**.
* Applications must define payload schemas and evolution strategy.

## 9. Rust Implementation Notes

* Encoder builds the 4-part frame into a `Vec<u8>`.
* Decoder scans `&[u8]` for the marker, then attempts to parse a full frame.
* Rebuilder orders frames by ID and backward links.
* Future: integrate a Rust FEC crate (e.g. Reed–Solomon) to enable “any k of n” recovery batches.

## 10. Roadmap

1. Configurable marker length and header fields.
2. Real backward links via BLAKE3/SHA-256 over previous frame.
3. Batch-level erasure coding.
4. CLI: `durapack scan <file>` → JSON of recovered frames.
5. WASM/GUI inspector for damaged captures.

## 11. Related Work

* CCSDS space packet standards
* DTN / Bundle Protocol
* WARC (self-describing web archives)
* FEC / fountain codes

**Name:** **Durapack**
**Tagline:** *Frames that survive what the link and the disk don’t.*
