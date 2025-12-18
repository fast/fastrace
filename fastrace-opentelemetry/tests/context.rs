use std::sync::Arc;
use std::sync::Mutex;

use fastrace::prelude::*;
use opentelemetry::trace::Span as _;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::Tracer as _;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::Context;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;

#[derive(Debug, Clone, Default)]
struct CapturingExporter {
    spans: Arc<Mutex<Vec<SpanData>>>,
}

impl CapturingExporter {
    fn new() -> (Self, Arc<Mutex<Vec<SpanData>>>) {
        let spans = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                spans: Arc::clone(&spans),
            },
            spans,
        )
    }
}

impl SpanExporter for CapturingExporter {
    fn export(
        &self,
        batch: Vec<SpanData>,
    ) -> impl std::future::Future<Output = OTelSdkResult> + Send {
        self.spans.lock().unwrap().extend(batch);
        std::future::ready(Ok(()))
    }
}

#[test]
fn otel_span_can_be_parented_by_fastrace_local_parent() {
    let (exporter, exported_spans) = CapturingExporter::new();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter)
        .build();
    let tracer = provider.tracer("fastrace-opentelemetry-test");

    let root = Span::root("root", SpanContext::random());
    let root_context = SpanContext::from_span(&root).unwrap();
    let _g = root.set_local_parent();

    let _otel_guard = fastrace_opentelemetry::current_opentelemetry_context()
        .map(|sc| Context::current().with_remote_span_context(sc).attach());

    let mut span = tracer.start("otel-child");
    span.end();

    provider.force_flush().unwrap();
    provider.shutdown().unwrap();

    let spans = exported_spans.lock().unwrap();
    assert_eq!(spans.len(), 1);
    let span = &spans[0];

    assert_eq!(
        span.span_context.trace_id().to_string(),
        root_context.trace_id.to_string()
    );
    assert_eq!(
        span.parent_span_id.to_string(),
        root_context.span_id.to_string()
    );
}
