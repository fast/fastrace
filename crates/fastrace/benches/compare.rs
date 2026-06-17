use std::sync::OnceLock;

use divan::Bencher;

fn main() {
    divan::main();
}

#[divan::bench_group(name = "single thread")]
mod single_thread {
    use super::*;

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn tokio_tracing(bencher: Bencher, n: usize) {
        init_tokio_tracing();

        bencher.bench(|| tokio_tracing_harness(n));
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn fastrace(bencher: Bencher, n: usize) {
        init_fastrace();

        bencher.bench(|| fastrace_harness(n));
    }
}

#[divan::bench_group(name = "multi threads")]
mod multi_thread {
    use super::*;

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn tokio_tracing(bencher: Bencher, n: usize) {
        init_tokio_tracing();

        let parallelism = std::thread::available_parallelism().unwrap().get() - 1;

        bencher.bench(|| {
            let handles: Vec<_> = (0..parallelism)
                .map(|_| std::thread::spawn(move || tokio_tracing_harness(n)))
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    #[divan::bench(args = [1, 10, 100, 1000, 10000])]
    fn fastrace(bencher: Bencher, n: usize) {
        init_fastrace();

        let parallelism = std::thread::available_parallelism().unwrap().get() - 1;

        bencher.bench(|| {
            let handles: Vec<_> = (0..parallelism)
                .map(|_| std::thread::spawn(move || fastrace_harness(n)))
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    }
}

fn make_span_exporter() -> opentelemetry_otlp::SpanExporter {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let exporter = rt.block_on(async {
        opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .unwrap()
    });
    std::mem::forget(rt);
    exporter
}

fn init_tokio_tracing() {
    use opentelemetry::trace::TracerProvider;
    use tracing_subscriber::prelude::*;

    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let exporter = make_span_exporter();
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .build();
        let tracer = provider.tracer("tracing-otel-subscriber");
        tracing_subscriber::registry()
            .with(tracing_opentelemetry::OpenTelemetryLayer::new(tracer))
            .init();
    });
}

fn init_fastrace() {
    use std::borrow::Cow;

    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let exporter = make_span_exporter();
        let reporter = fastrace_opentelemetry::OpenTelemetryReporter::new(
            exporter,
            Cow::Owned(opentelemetry_sdk::Resource::builder().build()),
            opentelemetry::InstrumentationScope::builder("example-crate").build(),
        );

        fastrace::set_reporter(reporter, fastrace::collector::Config::default());
    });
}

fn tokio_tracing_harness(n: usize) {
    fn dummy_tokio_tracing(n: usize) {
        for _ in 0..n {
            let child = tracing::span!(tracing::Level::TRACE, "child");
            let _enter = child.enter();
        }
    }

    let root = tracing::span!(tracing::Level::TRACE, "parent");
    let _enter = root.enter();

    dummy_tokio_tracing(n);
}

fn fastrace_harness(n: usize) {
    use fastrace::prelude::*;

    fn dummy_fastrace(n: usize) {
        for _ in 0..n {
            let _guard = LocalSpan::enter_with_local_parent("child");
        }
    }

    let root = Span::root("parent", SpanContext::new(TraceId(12), SpanId::default()));
    let _g = root.set_local_parent();

    dummy_fastrace(n);
}
