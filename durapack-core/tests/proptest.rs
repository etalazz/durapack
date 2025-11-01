//! Property-based tests using proptest

use bytes::Bytes;
use durapack_core::{
    decoder::decode_frame_from_bytes,
    encoder::{encode_frame, FrameBuilder},
    scanner::scan_stream,
    types::FrameHeader,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_round_trip_encode_decode(
        frame_id in 1u64..1000u64,
        payload in prop::collection::vec(any::<u8>(), 0..1024)
    ) {
        let payload_bytes = Bytes::from(payload);
        let encoded = FrameBuilder::new(frame_id)
            .payload(payload_bytes.clone())
            .mark_first()
            .with_crc32c()
            .build()
            .unwrap();

        let decoded = decode_frame_from_bytes(&encoded).unwrap();

        prop_assert_eq!(decoded.header.frame_id, frame_id);
        prop_assert_eq!(decoded.payload, payload_bytes);
    }

    #[test]
    fn prop_encode_never_panics(
        frame_id in any::<u64>(),
        payload in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        let result = FrameBuilder::new(frame_id)
            .payload(Bytes::from(payload))
            .with_blake3()
            .build();

        // Should either succeed or return an error, never panic
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn prop_decode_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        // Should never panic, even on random data
        let result = decode_frame_from_bytes(&data);
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn prop_scan_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..8192)
    ) {
        // Scanner should never panic
        let _ = scan_stream(&data);
    }

    #[test]
    fn prop_corrupted_stream_recovers_something(
        num_frames in 2usize..10,
        corruption_len in 10usize..100
    ) {
        // Create multiple frames
        let mut stream = Vec::new();

        for i in 0..num_frames {
            let frame = FrameBuilder::new(i as u64 + 1)
                .payload(Bytes::from(format!("Frame {}", i)))
                .with_crc32c()
                .build()
                .unwrap();
            stream.extend_from_slice(&frame);
        }

        let original_len = stream.len();

        // Add corruption in the middle
        let corrupt_pos = original_len / 2;
        stream.splice(
            corrupt_pos..corrupt_pos,
            vec![0xFF; corruption_len]
        );

        // Scan should recover at least some frames
        let located_frames = scan_stream(&stream);

        // We should recover at least 1 frame (usually more)
        prop_assert!(located_frames.len() >= 1);
    }

    #[test]
    fn prop_max_frame_size_enforced(
        payload_len in 16_777_000u32..17_000_000u32
    ) {
        let header = FrameHeader::new(1, [0u8; 32], payload_len);

        // Should fail validation for oversized payloads
        let result = header.validate();

        if payload_len > durapack_core::constants::MAX_PAYLOAD_SIZE {
            prop_assert!(result.is_err());
        }
    }
}
