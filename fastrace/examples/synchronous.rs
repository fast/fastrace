#![allow(clippy::new_without_default)]

use std::borrow::Cow;
use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::Reporter;
use fastrace::prelude::*;
use opentelemetry_otlp::WithExportConfig;

fn func1(i: u64) {
    let _guard = LocalSpan::enter_with_local_parent("func1");
    std::thread::sleep(Duration::from_millis(i));
    func2(i);
}

#[trace]
fn func2(i: u64) {
    std::thread::sleep(Duration::from_millis(i));
}

#[tokio::main]
async fn main() {
    fastrace::set_reporter(ReportAll::new(), Config::default());

    {
        let parent = SpanContext::random();
        let root = Span::root("root", parent);

        let _g = root.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent("a span")
            .with_property(|| ("a property", "a value"));

        for i in 1..=10 {
            func1(i);
        }
    }

    fastrace::flush();
}

pub struct ReportAll {
    jaeger: fastrace_jaeger::JaegerReporter,
    datadog: fastrace_datadog::DatadogReporter,
    opentelemetry: fastrace_opentelemetry::OpenTelemetryReporter,
}

impl ReportAll {
    pub fn new() -> ReportAll {
        ReportAll {
            jaeger: fastrace_jaeger::JaegerReporter::new(
                "127.0.0.1:6831".parse().unwrap(),
                "synchronous",
            )
            .unwrap(),
            datadog: fastrace_datadog::DatadogReporter::new(
                "127.0.0.1:8126".parse().unwrap(),
                "synchronous",
                "db",
                "select",
            ),
            opentelemetry: fastrace_opentelemetry::OpenTelemetryReporter::new(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint("http://127.0.0.1:4317".to_string())
                    .with_protocol(opentelemetry_otlp::Protocol::Grpc)
                    .with_timeout(Duration::from_secs(
                        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
                    ))
                    .build_span_exporter()
                    .expect("initialize oltp exporter"),
                opentelemetry::trace::SpanKind::Server,
                Cow::Owned(opentelemetry_sdk::Resource::new([
                    opentelemetry::KeyValue::new("service.name", "synchronous(opentelemetry)"),
                ])),
                opentelemetry::InstrumentationLibrary::builder("example-crate")
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .build(),
            ),
        }
    }
}

impl Reporter for ReportAll {
    fn report(&mut self, spans: &[SpanRecord]) {
        self.jaeger.report(spans);
        self.datadog.report(spans);
        self.opentelemetry.report(spans);
    }
}
