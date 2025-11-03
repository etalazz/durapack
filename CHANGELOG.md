# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2025-11-03

### Added
- Burst-error mitigation helpers in `durapack-core::interleave`:
  - `InterleaveParams`, `interleave_bytes`, `deinterleave_bytes` for writer/reader-side striping
  - Tests covering round-trip interleave/deinterleave
- README: documented interleaving guidance (writer/reader) and linked functions.
- Timeline analysis and visualization:
  - Core adds `analyze_timeline`, `analyze_located_frames`, and `TimelineReport` with `gap_details` (reasons), `conflicts`, `orphan_clusters`, and `recipes` (operator hints)
  - Core exports `report_to_dot(&TimelineReport)` to render Graphviz DOT with labeled gaps, conflicts, clusters, and note-shaped recipes
  - CLI `timeline` adds `--analyze` to include analysis in JSON (new `analysis` section) and to emit richer DOT when combined with `--dot`
- Optional FEC (pluggable):
  - Reed–Solomon encoder/decoder behind `fec-rs` feature using `reed-solomon-erasure`
  - Interleaved RS helper to pair with burst-error interleaving
  - Research flags `fec-raptorq` and `fec-ldpc` (stubs) with export-control note in README
- CLI FEC wiring and sidecar support (requires building CLI with `--features fec-rs`):
  - `pack`: `--fec-rs-data N` and `--fec-rs-parity K` emit K parity frames after each N data frames; writes a sidecar JSON index (`--fec-index-out`, defaults to `<output>.fec.json`).
  - `timeline`: `--fec-index <path>` annotates DOT output with RS N+K clusters; minimal attachment in JSON when `--analyze` is not used.
  - `verify`: `--fec-index <path>` and `--rs-repair` simulate reconstructability per RS block (report-only).
- Benchmarks:
  - New `fec` benchmark in `durapack-core/benches/fec.rs` (behind `fec-rs`) measuring RS encode across payload sizes.

### Notes
- Default build unaffected; FEC backends and CLI FEC wiring are opt-in via Cargo features.

## [0.2.3] - 2025-11-02

### Added
- Sync/marker engineering (format robustness and recovery):
  - Optional robust sync word (`ROBUST_SYNC_WORD`) with low autocorrelation preceding the marker
  - Optional preamble (`PREAMBLE_PATTERN`) with `MIN_PREAMBLE_LEN` to help resync after burst errors
  - Bounded-distance Hamming matching for the 4-byte `FRAME_MARKER` (`MAX_MARKER_HAMMING`) to tolerate small bit flips
  - New `FrameFlags` bits: `HAS_PREAMBLE` and `HAS_SYNC_PREFIX`
- Superframes and skip lists:
  - New `FrameFlags` bits: `IS_SUPERFRAME`, `HAS_SKIPLIST`
  - `FrameBuilder` helpers: `.as_superframe()`, `.with_skiplist()`
  - Types to carry superframe index and skip links in payload: `SuperframeIndex`, `SkipLink`
  - `Timeline::seek_with_skiplist` helper to leverage backlinks for O(log n) seeks when present
- Confidence model in scanner:
  - Per-frame confidence scoring based on marker quality (exact/hamming), sync/preamble presence, trailer validation, and size sanity
  - Neighbor-based bonuses for backlink consistency and contiguous spacing
  - Per-gap confidence derived from neighboring frame confidences; emitted in JSONL
  - CLI `scan` supports `--min-confidence` to filter outputs/carving

### Changed
- Scanner search pipeline now tries: exact match → sync/preamble-assisted resync → bounded-Hamming fallback
- Encoder can optionally emit preamble and/or robust sync prefix when corresponding flags are set (default remains unchanged)

### Documentation
- README updated with a "Robust sync/marker" feature and a usage section (constants, flags, and behavior)

### Compatibility
- Backward compatible: existing frames (without preamble/sync) decode unchanged; optional prefixes are only emitted if explicitly enabled via flags

## [0.2.2] - 2025-11-02

### Added
- `no_std` + `alloc` support for `durapack-core`:
  - New `std` feature (ON by default) gates I/O-based decode APIs and conveniences
  - Core works in `no_std` environments with `alloc` for Bytes/Vec usage
  - Dependencies configured for `no_std`: `blake3`, `bytes(alloc)`, `crc32c`, `memchr`

### Changed
- `decoder` now exposes `decode_frame_from_bytes` and zero-copy `decode_frame_from_bytes_zero_copy` in `no_std`.
- `decode_frame<R: Read>` and `try_decode_frame` are available only with `std` feature.

### Notes
- To build without std: `cargo build -p durapack-core --no-default-features`
- To build with std explicitly: `cargo build -p durapack-core --features std`

## [0.2.1] - 2025-11-02

### Added
- Performance hygiene in core:
  - Zero-copy decoder: `decode_frame_from_bytes_zero_copy(Bytes)` slices payload/trailer without allocating
  - Zero-copy scanner: `scan_stream_zero_copy(Bytes)` produces frames by slicing a shared buffer
  - SIMD-accelerated marker search via `memchr::memmem` for fast frame marker detection
- Criterion benches:
  - New `scanner` benchmark with realistic corpora (multiple frames, interspersed garbage) and throughput reporting
- Windows CI:
  - Dedicated workflow `windows-cli-smoke.yml` running `scripts/test-cli.ps1` to exercise all CLI commands
- Scripts:
  - `scripts/test-cli.ps1` PowerShell smoke test and `scripts/README.md` usage guide

### Changed
- CLI `scan` now uses zero-copy scanning in `--jsonl` mode to reduce copies and improve throughput
- README:
  - Added a comprehensive "CLI reference (--help)" section
  - Added CI badges for Linux CI and Windows CLI smoke
  - Documented Continuous Integration and local smoke test run command

### Fixed
- GitHub Actions: removed `--locked` in Windows smoke job to prevent lockfile update failures on fresh runners
- CI config: avoided calling non-reusable workflow; split Linux CI and Windows smoke into separate workflows
- Clippy warning cleanup and formatting across new benches and CLI changes

## [0.2.0] - 2025-11-01

### Added
- CLI ergonomics upgrade:
  - JSONL I/O for `scan` (stream-friendly), emits stats, gaps, and frames as JSON Lines
  - Stdin/stdout piping for `pack`, `scan`, `verify`, and `timeline` (use "-" as path)
  - `pack`: `--jsonl`, `--chunk-strategy`, `--rate-limit`, `--progress`
  - `scan`: `--jsonl` and `--carve-payloads "pattern_{stream}_{frame}.bin"`
  - `timeline`: `--dot` to emit Graphviz DOT for visualization
  - Colored diagnostics for `verify`
- Test vectors: programmatic generator and validator for 13 vectors (clean + corruption cases)
- Formal spec: documented versioned header, flags, link semantics, trailer variants, and corruption taxonomy

### Changed
- README updated with security scope (no encryption), export-control note, and docs links
- Improved README badges and layout

### Fixed
- Clippy and rustfmt compliance across workspace; CI green
- Test vector paths and generation order to avoid missing-file failures

## [0.1.0] - 2025-11-01 

### Added
- Initial implementation of Durapack frame format
- Core library (`durapack-core`) with encoding, decoding, scanning, and linking
- CLI tool (`durapack-cli`) with pack, scan, verify, and timeline commands
- Fuzzing harness (`durapack-fuzz`)
- Comprehensive test suite including property-based tests
- Performance benchmarks
- GitHub Actions CI/CD
- Documentation and examples

### Security
- BLAKE3 hash support for frame integrity
- CRC32C checksum support
- Maximum frame size enforcement
