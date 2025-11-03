use divan::Bencher;
use divan::black_box;
use fastrace::local::LocalCollector;
use fastrace::prelude::*;

fn main() {
    divan::main();
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn concurrent(bencher: Bencher, len: usize) {
    init_fastrace();

    let parallelism = std::thread::available_parallelism().unwrap().get() - 1;
    bencher.bench(|| {
        let handles: Vec<_> = (0..parallelism)
            .map(|_| {
                std::thread::spawn(move || {
                    for _ in 0..len {
                        let _ =
                            Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    });
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn wide_raw(bencher: Bencher, len: usize) {
    bencher.bench(|| {
        let local_collector = LocalCollector::start();
        dummy_iter(len);
        local_collector.collect()
    });
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn wide(bencher: Bencher, len: usize) {
    init_fastrace();
    bencher.bench(|| {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _sg = root.set_local_parent();
        dummy_iter(len - 1);
    });
}

#[divan::bench(args = [1, 10, 100, 1000])]
fn deep_raw(bencher: Bencher, len: usize) {
    bencher.bench(|| {
        let local_collector = LocalCollector::start();
        dummy_rec(len);
        local_collector.collect()
    });
}

#[divan::bench(args = [1, 10, 100, 1000])]
fn deep(bencher: Bencher, len: usize) {
    init_fastrace();
    bencher.bench(|| {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        let _sg = root.set_local_parent();
        dummy_rec(len - 1);
    });
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn future(bencher: Bencher, len: u32) {
    init_fastrace();

    async fn f(i: u32) {
        for _ in 0..i - 1 {
            async {}.enter_on_poll(black_box("")).await
        }
    }

    bencher.bench(|| {
        let root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
        pollster::block_on(f(len).in_span(root));
    });
}

fn init_fastrace() {
    struct DummyReporter;

    impl fastrace::collector::Reporter for DummyReporter {
        fn report(&mut self, _spans: Vec<SpanRecord>) {}
    }

    fastrace::set_reporter(DummyReporter, fastrace::collector::Config::default());
}

fn dummy_iter(i: usize) {
    #[trace]
    fn dummy() {}

    for _ in 0..i {
        dummy();
    }
}

#[trace]
fn dummy_rec(i: usize) {
    if i > 1 {
        dummy_rec(i - 1);
    }
}
