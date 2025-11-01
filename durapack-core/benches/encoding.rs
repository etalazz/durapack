use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use durapack_core::{
    decoder::decode_frame_from_bytes, encoder::FrameBuilder, scanner::scan_stream,
};

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    for size in [256, 1024, 4096, 16384] {
        let payload = vec![0x42u8; size];
        let payload_bytes = Bytes::from(payload);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                FrameBuilder::new(1)
                    .payload(payload_bytes.clone())
                    .with_crc32c()
                    .build()
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    for size in [256, 1024, 4096, 16384] {
        let payload = vec![0x42u8; size];
        let encoded = FrameBuilder::new(1)
            .payload(Bytes::from(payload))
            .with_crc32c()
            .build()
            .unwrap();

        group.throughput(Throughput::Bytes(encoded.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &encoded, |b, data| {
            b.iter(|| decode_frame_from_bytes(black_box(data)).unwrap());
        });
    }

    group.finish();
}

fn bench_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan");

    // Create a 10MB file with frames
    let mut stream = Vec::new();
    for i in 0..1000 {
        let payload = format!("Frame {} payload with some data", i);
        let frame = FrameBuilder::new(i)
            .payload(Bytes::from(payload))
            .with_crc32c()
            .build()
            .unwrap();
        stream.extend_from_slice(&frame);
    }

    // Add some corruption
    for i in (0..stream.len()).step_by(10000) {
        if i + 100 < stream.len() {
            stream[i..i + 100].fill(0xFF);
        }
    }

    let stream_size = stream.len();
    group.throughput(Throughput::Bytes(stream_size as u64));

    group.bench_function("scan_10mb", |b| {
        b.iter(|| {
            let results = scan_stream(black_box(&stream));
            black_box(results);
        });
    });

    group.finish();
}

fn bench_encode_with_blake3(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_blake3");

    for size in [256, 1024, 4096, 16384] {
        let payload = vec![0x42u8; size];
        let payload_bytes = Bytes::from(payload);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                FrameBuilder::new(1)
                    .payload(payload_bytes.clone())
                    .with_blake3()
                    .build()
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    for size in [256, 1024, 4096] {
        let payload = vec![0x42u8; size];

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let encoded = FrameBuilder::new(1)
                    .payload(Bytes::from(payload.clone()))
                    .with_crc32c()
                    .build()
                    .unwrap();

                let decoded = decode_frame_from_bytes(&encoded).unwrap();
                black_box(decoded);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encode,
    bench_decode,
    bench_scan,
    bench_encode_with_blake3,
    bench_round_trip
);
criterion_main!(benches);
