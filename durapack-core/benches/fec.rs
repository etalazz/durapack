use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use durapack_core::encoder::FrameBuilder;
#[cfg(feature = "fec-rs")]
use durapack_core::fec::{RedundancyEncoder, RsEncoder};

fn bench_rs_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("fec_rs_encode");
    let sizes = [256usize, 1024, 4096, 16384];
    #[cfg(feature = "fec-rs")]
    for &sz in &sizes {
        let payload = vec![0u8; sz];
        group.throughput(Throughput::Bytes((sz * 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(sz), &sz, |b, &_s| {
            b.iter_batched(
                || {
                    // Build 8 small frames as a block
                    (0..8)
                        .map(|i| {
                            FrameBuilder::new(i as u64)
                                .payload(Bytes::from(payload.clone()))
                                .build_struct()
                                .unwrap()
                        })
                        .collect::<Vec<_>>()
                },
                |frames| {
                    let enc = RsEncoder::new(frames.len(), 2);
                    let _blocks = enc.encode_batch(&frames, 0).unwrap();
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_rs_encode);
criterion_main!(benches);
