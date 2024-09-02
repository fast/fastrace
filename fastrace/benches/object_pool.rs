// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BatchSize;
use criterion::Criterion;
use fastrace::util::object_pool::GlobalVecPool;

fn bench_alloc_vec(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("Vec::with_capacity");

    for cap in &[1, 10, 100, 1000, 10000, 100000] {
        static VEC_POOL: GlobalVecPool<usize> = GlobalVecPool::new();
        let mut puller = VEC_POOL.new_local(512);
        bgroup.bench_function(format!("object-pool/{}", cap), |b| {
            b.iter_batched(
                || (),
                |_| {
                    let mut vec = puller.take();
                    if vec.capacity() < *cap {
                        vec.reserve(*cap);
                    }
                    vec
                },
                BatchSize::NumIterations(512),
            )
        });

        bgroup.bench_function(format!("alloc/{}", cap), |b| {
            b.iter_batched(
                || (),
                |_| Vec::<usize>::with_capacity(*cap),
                BatchSize::NumIterations(512),
            )
        });
    }

    bgroup.finish();
}

criterion_group!(benches, bench_alloc_vec);
criterion_main!(benches);
