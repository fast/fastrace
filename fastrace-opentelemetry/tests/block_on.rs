use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use fastrace::collector::Reporter;
use fastrace::collector::SpanId;
use fastrace::collector::SpanRecord;
use fastrace::collector::TraceId;
use fastrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry::InstrumentationScope;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;

#[derive(Debug)]
struct TestExporter {
    exported_spans: Arc<AtomicUsize>,
}

impl SpanExporter for TestExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        self.exported_spans
            .fetch_add(batch.len(), Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn custom_block_on_is_used() {
    let exported_spans = Arc::new(AtomicUsize::new(0));
    let block_on_calls = Arc::new(AtomicUsize::new(0));
    let block_on_calls_inner = block_on_calls.clone();

    let mut reporter = OpenTelemetryReporter::new(
        TestExporter {
            exported_spans: exported_spans.clone(),
        },
        Cow::Owned(Resource::builder_empty().build()),
        InstrumentationScope::builder("test-crate").build(),
    )
    .with_block_on(move |future| {
        block_on_calls_inner.fetch_add(1, Ordering::Relaxed);
        pollster::block_on(future)
    });

    reporter.report(vec![SpanRecord {
        trace_id: TraceId(1),
        span_id: SpanId(2),
        parent_id: SpanId(0),
        begin_time_unix_ns: 0,
        duration_ns: 1,
        name: "span".into(),
        properties: Vec::new(),
        events: Vec::new(),
        links: Vec::new(),
    }]);

    assert_eq!(block_on_calls.load(Ordering::Relaxed), 1);
    assert_eq!(exported_spans.load(Ordering::Relaxed), 1);
}
