//! Fuzzing placeholder for durapack-core decoder
//!
//! To use with cargo-fuzz:
//! 1. Install cargo-fuzz: cargo install cargo-fuzz
//! 2. Run fuzzer: cargo fuzz run fuzz_decoder

pub fn fuzz_decode(data: &[u8]) {
    use durapack_core::decoder::decode_frame_from_bytes;

    // Try to decode - should never panic
    let _ = decode_frame_from_bytes(data);
}

pub fn fuzz_scan(data: &[u8]) {
    use durapack_core::scanner::scan_stream;

    // Try to scan - should never panic
    let _ = scan_stream(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_decode_empty() {
        fuzz_decode(&[]);
    }

    #[test]
    fn test_fuzz_decode_random() {
        fuzz_decode(&[0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_fuzz_scan_empty() {
        fuzz_scan(&[]);
    }

    #[test]
    fn test_fuzz_scan_random() {
        fuzz_scan(&[0xFF; 1024]);
    }
}

