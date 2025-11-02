use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use durapack_core::{
    encoder::FrameBuilder,
    scanner::{scan_stream, scan_stream_with_stats},
};

fn make_stream(num_frames: usize, payload_len: usize) -> Vec<u8> {
    let mut prev_hash = [0u8; 32];
    let mut stream = Vec::new();
    for i in 0..num_frames {
        let payload = vec![b'x'; payload_len];
        let frame = FrameBuilder::new(i as u64 + 1)
            .prev_hash(prev_hash)
            .with_blake3()
            .payload(Bytes::from(payload))
            .build()
            .unwrap();
        prev_hash.copy_from_slice(
            &durapack_core::decoder::decode_frame_from_bytes(&frame)
                .unwrap()
                .compute_hash(),
        );
        stream.extend_from_slice(&frame);
        if i % 10 == 0 {
            // inject a bit of garbage periodically
            stream.extend_from_slice(b"GARBAGE");
        }
    }
    stream
}

fn bench_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanner");

    for &payload_len in &[16usize, 256, 4096] {
        let stream = make_stream(500, payload_len);
        group.throughput(Throughput::Bytes(stream.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("scan_stream", payload_len),
            &stream,
            |b, data| {
                b.iter(|| {
                    let res = scan_stream(data);
                    criterion::black_box(res);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scan_stream_with_stats", payload_len),
            &stream,
            |b, data| {
                b.iter(|| {
                    let res = scan_stream_with_stats(data);
                    criterion::black_box(res);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_scanner);
criterion_main!(benches);
