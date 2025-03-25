use divan::Bencher;

fn main() {
    divan::main();
}

#[divan::bench_group(name = "send and receive")]
mod spsc {
    use super::*;

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn crossbeam(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| crossbeam::channel::bounded(10240))
            .bench_values(|(tx, rx)| {
                std::thread::spawn(move || {
                    for i in 0..len {
                        while tx.try_send(i).is_err() {}
                    }
                });

                for _ in 0..len {
                    while rx.try_recv().is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn ringbuffer(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| rtrb::RingBuffer::new(10240))
            .bench_values(|(mut tx, mut rx)| {
                std::thread::spawn(move || {
                    for i in 0..len {
                        while tx.push(i).is_err() {}
                    }
                });

                for _ in 0..len {
                    while rx.pop().is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn flume(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| flume::bounded(10240))
            .bench_values(|(tx, rx)| {
                std::thread::spawn(move || {
                    for i in 0..len {
                        while tx.send(i).is_err() {}
                    }
                });

                for _ in 0..len {
                    while rx.recv().is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn fastrace(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| fastrace::util::spsc::bounded(10240))
            .bench_values(|(mut tx, mut rx)| {
                std::thread::spawn(move || {
                    for i in 0..len {
                        while tx.send(i).is_err() {}
                    }
                });

                for _ in 0..len {
                    while rx.try_recv().is_err() {}
                }
            })
    }
}

#[divan::bench_group(name = "send only")]
mod spsc_send_only {
    use super::*;

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn crossbeam(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| crossbeam::channel::bounded(10240))
            .bench_values(|(tx, _rx)| {
                for i in 0..len {
                    while tx.try_send(i).is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn ringbuffer(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| rtrb::RingBuffer::new(10240))
            .bench_values(|(mut tx, _rx)| {
                for i in 0..len {
                    while tx.push(i).is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn flume(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| flume::bounded(10240))
            .bench_values(|(tx, _rx)| {
                for i in 0..len {
                    while tx.send(i).is_err() {}
                }
            });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn fastrace(bencher: Bencher, len: usize) {
        bencher
            .with_inputs(|| fastrace::util::spsc::bounded(10240))
            .bench_values(|(mut tx, _rx)| {
                for i in 0..len {
                    while tx.send(i).is_err() {}
                }
            });
    }
}
