// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::time::Duration;
use std::time::Instant;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;

fn spsc_comparison(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("spsc channel");

    for &len in &[1, 10, 100, 1000, 10000] {
        bgroup.bench_function(format!("crossbeam/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, rx) = crossbeam::channel::bounded(10240);

                    let start = Instant::now();

                    std::thread::spawn(move || {
                        for i in 0..len {
                            while tx.try_send(i).is_err() {}
                        }
                    });

                    for _ in 0..len {
                        while rx.try_recv().is_err() {}
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("ringbuffer/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (mut tx, mut rx) = rtrb::RingBuffer::new(10240);

                    let start = Instant::now();

                    std::thread::spawn(move || {
                        for i in 0..len {
                            while tx.push(i).is_err() {}
                        }
                    });

                    for _ in 0..len {
                        while rx.pop().is_err() {}
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("flume/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, rx) = flume::bounded(10240);

                    let start = Instant::now();

                    std::thread::spawn(move || {
                        for i in 0..len {
                            while tx.send(i).is_err() {}
                        }
                    });

                    for _ in 0..len {
                        while rx.recv().is_err() {}
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("fastrace/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (mut tx, mut rx) = fastrace::util::spsc::bounded(10240);

                    let start = Instant::now();

                    std::thread::spawn(move || {
                        for i in 0..len {
                            while tx.send(i).is_err() {}
                        }
                    });

                    for _ in 0..len {
                        loop {
                            if let Ok(Some(_)) = rx.try_recv() {
                                break;
                            }
                        }
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("fastrace-legacy/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, mut rx) = fastrace::util::legacy_spsc::bounded(10240);

                    let start = Instant::now();

                    std::thread::spawn(move || {
                        for i in 0..len {
                            while tx.send(i).is_err() {}
                        }
                    });

                    for _ in 0..len {
                        loop {
                            if let Ok(Some(_)) = rx.try_recv() {
                                break;
                            }
                        }
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
    }

    bgroup.finish();
}

fn spsc_send_only_comparison(c: &mut Criterion) {
    let mut bgroup = c.benchmark_group("spsc channel send only");

    for &len in &[1, 10, 100, 1000, 10000] {
        bgroup.bench_function(format!("crossbeam/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, _rx) = crossbeam::channel::bounded(10240);

                    let start = Instant::now();

                    for i in 0..len {
                        tx.send(i).unwrap();
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("ringbuffer/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (mut tx, _rx) = rtrb::RingBuffer::new(10240);

                    let start = Instant::now();

                    for i in 0..len {
                        tx.push(i).unwrap();
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("flume/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, _rx) = flume::bounded(10240);

                    let start = Instant::now();

                    for i in 0..len {
                        tx.send(i).unwrap();
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("fastrace/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (mut tx, _rx) = fastrace::util::spsc::bounded(10240);

                    let start = Instant::now();

                    for i in 0..len {
                        tx.send(i).unwrap();
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
        bgroup.bench_function(format!("fastrace-legacy/{}", len), |b| {
            b.iter_custom(|iters| {
                let mut total_time = Duration::default();
                for _ in 0..iters {
                    let (tx, _rx) = fastrace::util::legacy_spsc::bounded(10240);

                    let start = Instant::now();

                    for i in 0..len {
                        tx.send(i).unwrap();
                    }

                    total_time += start.elapsed();
                }
                total_time
            })
        });
    }

    bgroup.finish();
}

criterion_group!(benches, spsc_comparison, spsc_send_only_comparison);
criterion_main!(benches);
