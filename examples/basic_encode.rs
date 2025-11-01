//! Basic encoding example

use bytes::Bytes;
use durapack_core::encoder::FrameBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Durapack Basic Encoding Example\n");

    // Create a sequence of frames with linked hashes
    let mut frames = Vec::new();
    let mut prev_hash = [0u8; 32]; // First frame has zero prev_hash

    for i in 1..=5 {
        let payload = format!("This is frame {} with some telemetry data", i);

        let mut builder = FrameBuilder::new(i)
            .payload(Bytes::from(payload))
            .prev_hash(prev_hash)
            .with_blake3();

        if i == 1 {
            builder = builder.mark_first();
        }
        if i == 5 {
            builder = builder.mark_last();
        }

        let frame_struct = builder.build_struct()?;

        // Compute hash for next frame's prev_hash
        prev_hash = frame_struct.compute_hash();

        // Encode to bytes
        let encoded = durapack_core::encoder::encode_frame_struct(&frame_struct)?;

        println!("Frame {}: {} bytes", i, encoded.len());
        frames.push(encoded);
    }

    // Concatenate all frames into a single file
    let mut output = Vec::new();
    for frame in frames {
        output.extend_from_slice(&frame);
    }

    std::fs::write("example_output.durp", &output)?;

    println!("\nWrote {} bytes to example_output.durp", output.len());
    println!("Use 'durapack scan --input example_output.durp' to read it back");

    Ok(())
}
