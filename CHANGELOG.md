# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

## [0.1.0] - 2025-11-01

### Added
- First release

