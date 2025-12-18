# fastrace-opentelemetry

[![Documentation](https://docs.rs/fastrace-opentelemetry/badge.svg)](https://docs.rs/fastrace-opentelemetry/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-opentelemetry.svg)](https://crates.io/crates/fastrace-opentelemetry)
[![LICENSE](https://img.shields.io/github/license/fast/fastrace.svg)](https://github.com/fast/fastrace/blob/main/LICENSE)

[OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust) reporter for [`fastrace`](https://crates.io/crates/fastrace).

## Dependencies

```toml
[dependencies]
fastrace = { version = "0.7", features = ["enable"] }
fastrace-opentelemetry = "0.15"
```

## Setup OpenTelemetry Collector

Start OpenTelemetry Collector with Jaeger and Zipkin receivers:

```shell
docker compose -f dev/docker-compose.yaml up
```

Then, run the synchronous example:

```shell
cargo run --example synchronous
```

Jaeger UI is available on [http://127.0.0.1:16686/](http://127.0.0.1:16686/)

Zipkin UI is available on [http://127.0.0.1:9411/](http://127.0.0.1:9411/)

## Report to OpenTelemetry Collector

```rust, no_run
use std::borrow::Cow;

use fastrace::collector::Config;
use fastrace::prelude::*;
use fastrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;

// Initialize reporter
let reporter = OpenTelemetryReporter::new(
    SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://127.0.0.1:4317".to_string())
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
        .build()
        .expect("initialize otlp exporter"),
    Cow::Owned(
        Resource::builder()
            .with_attributes([KeyValue::new("service.name", "asynchronous")])
            .build()
    ),
    InstrumentationScope::builder("example-crate").with_version(env!("CARGO_PKG_VERSION")).build(),
);
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush();
```

## Activate OpenTelemetry Trace Context

If you use fastrace spans but also depend on libraries that expect an OpenTelemetry parent
[`Context`](https://docs.rs/opentelemetry/latest/opentelemetry/struct.Context.html), you can bridge
the current fastrace **local parent** into an OpenTelemetry context.

This requires a local parent to be set for the current thread (e.g. via
[`Span::set_local_parent`](https://docs.rs/fastrace/latest/fastrace/struct.Span.html#method.set_local_parent)).

```rust
use fastrace_opentelemetry::current_opentelemetry_context;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;

let _otel_guard = current_opentelemetry_context()
    .map(|sc| Context::current().with_remote_span_context(sc).attach());

// Call library code that uses `Context::current()`.
```
