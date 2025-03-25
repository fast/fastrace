use std::sync::OnceLock;

use divan::Bencher;

fn main() {
    divan::main();
}

#[divan::bench(args = [1, 10, 100, 1000])]
fn tokio_tracing(bencher: Bencher, n: usize) {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(init_tokio_tracing);

    bencher.bench(|| opentelemetry_harness(n));
}

#[divan::bench(args = [1, 10, 100, 1000])]
fn fastrace(bencher: Bencher, n: usize) {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(init_fastrace);

    bencher.bench(|| fastrace_harness(n));
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

    let exporter = make_span_exporter();
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();
    let tracer = provider.tracer("tracing-otel-subscriber");
    tracing_subscriber::registry()
        .with(tracing_opentelemetry::OpenTelemetryLayer::new(tracer))
        .init();
}

fn init_fastrace() {
    use std::borrow::Cow;

    let exporter = make_span_exporter();
    let reporter = fastrace_opentelemetry::OpenTelemetryReporter::new(
        exporter,
        opentelemetry::trace::SpanKind::Server,
        Cow::Owned(opentelemetry_sdk::Resource::builder().build()),
        opentelemetry::InstrumentationScope::builder("example-crate").build(),
    );

    fastrace::set_reporter(reporter, fastrace::collector::Config::default());
}

fn opentelemetry_harness(n: usize) {
    fn dummy_opentelementry(n: usize) {
        for _ in 0..n {
            let child = tracing::span!(tracing::Level::TRACE, "child");
            let _enter = child.enter();
        }
    }

    let root = tracing::span!(tracing::Level::TRACE, "parent");
    let _enter = root.enter();

    dummy_opentelementry(n);
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
