# fastrace-opentelemetry

[![Documentation](https://docs.rs/fastrace-opentelemetry/badge.svg)](https://docs.rs/fastrace-opentelemetry/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-opentelemetry.svg)](https://crates.io/crates/fastrace-opentelemetry)
[![LICENSE](https://img.shields.io/github/license/fastracelabs/fastrace.svg)](https://github.com/fastracelabs/fastrace/blob/main/LICENSE)

[OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust) reporter for [`fastrace`](https://crates.io/crates/fastrace).

## Dependencies

```toml
[dependencies]
fastrace = "0.6"
fastrace-opentelemetry = "0.6"
```

## Setup OpenTelemetry Collector

```sh
cd fastrace-opentelemetry/examples
docker compose up -d

cargo run --example synchronous
```

Jaeger UI is available on [http://127.0.0.1:16686/](http://127.0.0.1:16686/)

Zipkin UI is available on [http://127.0.0.1:9411/](http://127.0.0.1:9411/)

## Report to OpenTelemetry Collector

```rust, no_run
use std::borrow::Cow;
use std::time::Duration;
use fastrace::collector::Config;
use fastrace::prelude::*;
use fastrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry_otlp::{SpanExporter, ExportConfig, Protocol, TonicConfig};
use opentelemetry::trace::SpanKind;
use opentelemetry_sdk::Resource;
use opentelemetry::KeyValue;
use opentelemetry::InstrumentationLibrary;
use opentelemetry_otlp::WithExportConfig;

// Initialize reporter
let reporter = OpenTelemetryReporter::new(
    opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("http://127.0.0.1:4317".to_string())
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(Duration::from_secs(
            opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
        ))
        .build_span_exporter()
        .expect("initialize oltp exporter"),
    SpanKind::Server,
    Cow::Owned(Resource::new([KeyValue::new("service.name", "asynchronous")])),
    InstrumentationLibrary::new("example-crate", Some(env!("CARGO_PKG_VERSION")), None::<&'static str>, None),
);
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush()
```
