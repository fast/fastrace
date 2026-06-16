# fastrace-datadog

[![Documentation](https://docs.rs/fastrace-datadog/badge.svg)](https://docs.rs/fastrace-datadog/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-datadog.svg)](https://crates.io/crates/fastrace-datadog)
[![LICENSE](https://img.shields.io/github/license/fast/fastrace.svg)](https://github.com/fast/fastrace/blob/main/LICENSE)

[Datadog](https://docs.datadoghq.com/tracing/) reporter for [`fastrace`](https://crates.io/crates/fastrace).

> [!WARNING]
>
> `fastrace-datadog` is deprecated in favor of [`fastrace-opentelemetry`](https://crates.io/crates/fastrace-opentelemetry).
>
> The Datadog Agent ingests OTLP natively, so traces can be sent through `fastrace-opentelemetry` instead.
> 
> See [Migration](#migration-to-fastrace-opentelemetry) below.

## Migration to fastrace-opentelemetry

The Datadog Agent ingests OTLP natively. First, enable OTLP ingestion in your Agent (see Datadog's [OTLP ingest docs](https://docs.datadoghq.com/opentelemetry/interoperability/otlp_ingest_in_the_agent/)). Then replace the reporter with `fastrace-opentelemetry` pointed at the Agent's OTLP endpoint (gRPC `4317` by default). Your instrumentation (`Span::root`, `#[trace]`, ...) stays the same — only the reporter setup changes.

Update your dependencies:

```toml
[dependencies]
fastrace = { version = "0.7", features = ["enable"] }
fastrace-opentelemetry = { version = "0.18.0" }
opentelemetry = { version = "0.32.0", default-features = false, features = ["trace"] }
opentelemetry-otlp = { version = "0.32.0", default-features = false, features = ["trace", "grpc-tonic"] }
opentelemetry_sdk = { version = "0.32.0", default-features = false, features = ["trace"] }
```

**Before** (deprecated):

```rust,ignore
let reporter = fastrace_datadog::DatadogReporter::new(
    "127.0.0.1:8126".parse().unwrap(),
    "my-service",
    "db",
    "select",
);
fastrace::set_reporter(reporter, Config::default());
```

**After** (OTLP to the Datadog Agent):

```rust,ignore
use std::borrow::Cow;

use fastrace::collector::Config;
use fastrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;

let reporter = OpenTelemetryReporter::new(
    SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://127.0.0.1:4317".to_string()) // Datadog Agent OTLP endpoint
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
        .build()
        .expect("initialize otlp exporter"),
    Cow::Owned(
        Resource::builder()
            .with_service_name("my-service")
            .with_attributes([KeyValue::new("span.resource", "db")])
            .with_attributes([KeyValue::new("span.type", "select")])
            .build(),
    ),
    InstrumentationScope::builder("my-crate")
        .with_version(env!("CARGO_PKG_VERSION"))
        .build(),
);
fastrace::set_reporter(reporter, Config::default());
```

## Dependencies

```toml
[dependencies]
fastrace = "0.7"
fastrace-datadog = "0.7"
```

## Setup Datadog Agent

Please follow the Datadog [official documentation](https://docs.datadoghq.com/getting_started/tracing/#datadog-agent).

```sh
cargo run --example synchronous
```

## Report to Datadog Agent

```rust
use std::net::SocketAddr;

use fastrace::collector::Config;
use fastrace::prelude::*;

// Initialize reporter
let reporter = fastrace_datadog::DatadogReporter::new(
    "127.0.0.1:8126".parse().unwrap(),
    "asynchronous",
    "db",
    "select",
);
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush();
```
