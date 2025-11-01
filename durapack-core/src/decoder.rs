//! Frame decoding (strict mode)

use crate::constants::{
    FrameFlags, TrailerType, FRAME_MARKER, MAX_FRAME_SIZE, MIN_HEADER_SIZE, PROTOCOL_VERSION,
};
use crate::error::FrameError;
use crate::types::{Frame, FrameHeader};
use bytes::Bytes;
use std::io::{ErrorKind, Read};

/// Decode a frame from a reader
///
/// This function performs strict validation:
/// - Validates marker
/// - Validates version
/// - Validates length
/// - Validates checksum/hash if present
///
/// Returns an error if any validation fails.
pub fn decode_frame<R: Read>(reader: &mut R) -> Result<Frame, FrameError> {
    // Read and validate marker
    let mut marker = [0u8; 4];
    reader.read_exact(&mut marker)?;

    if &marker != FRAME_MARKER {
        return Err(FrameError::BadMarker(marker));
    }

    // Read header
    let mut header_buf = [0u8; MIN_HEADER_SIZE - 4]; // Minus marker
    reader.read_exact(&mut header_buf)?;

    let version = header_buf[0];
    if version != PROTOCOL_VERSION {
        return Err(FrameError::UnsupportedVersion(version));
    }

    let frame_id = u64::from_be_bytes([
        header_buf[1],
        header_buf[2],
        header_buf[3],
        header_buf[4],
        header_buf[5],
        header_buf[6],
        header_buf[7],
        header_buf[8],
    ]);

    let mut prev_hash = [0u8; 32];
    prev_hash.copy_from_slice(&header_buf[9..41]);

    let payload_len = u32::from_be_bytes([
        header_buf[41],
        header_buf[42],
        header_buf[43],
        header_buf[44],
    ]);

    let flags = FrameFlags::new(header_buf[45]);

    // Validate payload length
    let total_frame_size =
        MIN_HEADER_SIZE as u32 + payload_len + flags.trailer_type().size() as u32;
    if total_frame_size > MAX_FRAME_SIZE {
        return Err(FrameError::FrameTooLarge(total_frame_size, MAX_FRAME_SIZE));
    }

    // Create header
    let header = FrameHeader::with_flags(frame_id, prev_hash, payload_len, flags);
    header.validate()?;

    // Read payload
    let mut payload = vec![0u8; payload_len as usize];
    reader.read_exact(&mut payload)?;

    // Read and validate trailer if present
    let trailer_type = flags.trailer_type();
    let trailer = match trailer_type {
        TrailerType::None => None,
        TrailerType::Crc32c => {
            let mut expected_checksum = [0u8; 4];
            reader.read_exact(&mut expected_checksum)?;
            let expected = u32::from_be_bytes(expected_checksum);

            // Compute checksum over marker + header + payload
            let mut data = Vec::with_capacity(MIN_HEADER_SIZE + payload.len());
            data.extend_from_slice(FRAME_MARKER);
            data.extend_from_slice(&header_buf);
            data.extend_from_slice(&payload);

            let actual = crc32c::crc32c(&data);

            if actual != expected {
                return Err(FrameError::ChecksumMismatch { expected, actual });
            }

            Some(Bytes::copy_from_slice(&expected_checksum))
        }
        TrailerType::Blake3 => {
            let mut expected_hash = [0u8; 32];
            reader.read_exact(&mut expected_hash)?;

            // Compute hash over marker + header + payload
            let mut data = Vec::with_capacity(MIN_HEADER_SIZE + payload.len());
            data.extend_from_slice(FRAME_MARKER);
            data.extend_from_slice(&header_buf);
            data.extend_from_slice(&payload);

            let actual_hash = blake3::hash(&data);

            if actual_hash.as_bytes() != &expected_hash {
                return Err(FrameError::HashMismatch);
            }

            Some(Bytes::copy_from_slice(&expected_hash))
        }
    };

    Ok(Frame::with_trailer(
        header,
        Bytes::from(payload),
        trailer.unwrap_or_default(),
    ))
}

/// Decode a frame from a byte slice
pub fn decode_frame_from_bytes(data: &[u8]) -> Result<Frame, FrameError> {
    let mut cursor = std::io::Cursor::new(data);
    decode_frame(&mut cursor)
}

/// Try to decode a frame, returning the number of bytes consumed
///
/// This is useful for stream processing where you want to know how much
/// to advance the read position.
pub fn try_decode_frame<R: Read>(reader: &mut R) -> Result<(Frame, usize), FrameError> {
    let mut bytes_read = 0;

    // Read marker
    let mut marker = [0u8; 4];
    match reader.read_exact(&mut marker) {
        Ok(_) => bytes_read += 4,
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
            return Err(FrameError::IncompleteFrame {
                expected: 4,
                actual: 0,
            });
        }
        Err(e) => return Err(e.into()),
    }

    if &marker != FRAME_MARKER {
        return Err(FrameError::BadMarker(marker));
    }

    // Read header
    let mut header_buf = [0u8; MIN_HEADER_SIZE - 4];
    reader.read_exact(&mut header_buf)?;
    bytes_read += MIN_HEADER_SIZE - 4;

    let payload_len = u32::from_be_bytes([
        header_buf[41],
        header_buf[42],
        header_buf[43],
        header_buf[44],
    ]);

    let flags = FrameFlags::new(header_buf[45]);
    let trailer_size = flags.trailer_type().size();

    // Read payload
    let mut payload = vec![0u8; payload_len as usize];
    reader.read_exact(&mut payload)?;
    bytes_read += payload_len as usize;

    // Read trailer
    let mut trailer = vec![0u8; trailer_size];
    if trailer_size > 0 {
        reader.read_exact(&mut trailer)?;
        bytes_read += trailer_size;
    }

    // Now decode the complete frame
    let mut all_data = Vec::with_capacity(bytes_read);
    all_data.extend_from_slice(&marker);
    all_data.extend_from_slice(&header_buf);
    all_data.extend_from_slice(&payload);
    all_data.extend_from_slice(&trailer);

    let frame = decode_frame_from_bytes(&all_data)?;

    Ok((frame, bytes_read))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::encode_frame;
    use crate::types::FrameHeader;

    #[test]
    fn test_decode_simple_frame() {
        let payload = b"Hello, Durapack!";
        let header = FrameHeader::new(1, [0u8; 32], payload.len() as u32);

        let encoded = encode_frame(&header, payload).unwrap();
        let decoded = decode_frame_from_bytes(&encoded).unwrap();

        assert_eq!(decoded.header.frame_id, 1);
        assert_eq!(decoded.payload.as_ref(), payload);
    }

    #[test]
    fn test_decode_with_crc() {
        let payload = b"test payload";
        let header = FrameHeader::with_flags(
            42,
            [0u8; 32],
            payload.len() as u32,
            FrameFlags::new(FrameFlags::HAS_CRC32C),
        );

        let encoded = encode_frame(&header, payload).unwrap();
        let decoded = decode_frame_from_bytes(&encoded).unwrap();

        assert_eq!(decoded.header.frame_id, 42);
        assert_eq!(decoded.payload.as_ref(), payload);
    }

    #[test]
    fn test_decode_bad_marker() {
        let bad_data = b"NOPE\x01\x00\x00\x00\x00\x00\x00\x00\x00";
        let result = decode_frame_from_bytes(bad_data);

        assert!(matches!(result, Err(FrameError::BadMarker(_))));
    }

    #[test]
    fn test_round_trip() {
        let payload = b"Round trip test payload";
        let header = FrameHeader::with_flags(
            100,
            [1u8; 32],
            payload.len() as u32,
            FrameFlags::new(FrameFlags::HAS_BLAKE3),
        );

        let encoded = encode_frame(&header, payload).unwrap();
        let decoded = decode_frame_from_bytes(&encoded).unwrap();

        assert_eq!(decoded.header.frame_id, 100);
        assert_eq!(decoded.header.prev_hash, [1u8; 32]);
        assert_eq!(decoded.payload.as_ref(), payload);
    }
}
