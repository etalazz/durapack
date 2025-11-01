# Contributing to Durapack

Thank you for your interest in contributing to Durapack!

## Development Setup

1. Install Rust (stable): https://rustup.rs/
2. Clone the repository
3. Run tests: `cargo test --all`
4. Run benchmarks: `cargo bench`

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy --all-targets --all-features` and fix warnings
- Follow Rust naming conventions
- Add documentation for public APIs

## Testing

- Add unit tests for new functions
- Add integration tests for new features
- Run property tests: `cargo test --test proptest`
- Ensure no panics on invalid input

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Add tests
5. Run `cargo test --all`
6. Run `cargo fmt` and `cargo clippy`
7. Commit with clear messages
8. Push to your fork
9. Open a pull request

## Areas for Contribution

- Reed-Solomon or Raptor FEC implementation
- Additional CLI commands
- Performance optimizations
- Documentation improvements
- Example use cases
- Fuzzing improvements

## Questions?

Open an issue for discussion before starting major work.

