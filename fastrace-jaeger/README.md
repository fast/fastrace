# fastrace-jaeger

[![Documentation](https://docs.rs/fastrace-jaeger/badge.svg)](https://docs.rs/fastrace-jaeger/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-jaeger.svg)](https://crates.io/crates/fastrace-jaeger)
[![LICENSE](https://img.shields.io/github/license/fast/fastrace.svg)](https://github.com/fast/fastrace/blob/main/LICENSE)

[Jaeger](https://www.jaegertracing.io/) reporter for [`fastrace`](https://crates.io/crates/fastrace).

> [!WARNING]
>
> `fastrace-jaeger` is deprecated in favor of [`fastrace-opentelemetry`](https://crates.io/crates/fastrace-opentelemetry).
>
> Jaeger has supported OTLP natively since v1.35 — the path Jaeger now recommends — and a dedicated Jaeger reporter only duplicates `fastrace-opentelemetry`.
> 
> See [Migrating to fastrace-opentelemetry](#migrating-to-fastrace-opentelemetry) below.

## Dependencies

```toml
[dependencies]
fastrace = "0.7"
fastrace-jaeger = "0.7"
```

## Setup Jaeger Agent

```sh
docker run --rm -d -p6831:6831/udp -p14268:14268 -p16686:16686 --name jaeger jaegertracing/all-in-one:1.6.0

cargo run --example synchronous
```

Web UI is available on [http://127.0.0.1:16686/](http://127.0.0.1:16686/)

## Report to Jaeger Agent

```rust
// deprecated
use std::net::SocketAddr;

use fastrace::collector::Config;
use fastrace::prelude::*;
use fastrace_jaeger::JaegerReporter;

// Initialize reporter
let reporter = JaegerReporter::new("127.0.0.1:6831".parse().unwrap(), "asynchronous").unwrap();
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush();
```

### Migrating to fastrace-opentelemetry

`fastrace-opentelemetry` exports via OTLP, which Jaeger accepts natively (v1.35+). Your instrumentation (`Span::root`, `#[trace]`, ...) stays the same — only the reporter changes.

For example, given that you have a Jaeger stack as below:

```shell
docker run --rm -d -p6831:6831/udp -p14268:14268 -p16686:16686 -p4317:4317 --name jaeger jaegertracing/all-in-one:1.76.0
```

You can report to it via OTLP as below:

First, update your dependencies:

```toml
[dependencies]
fastrace = { version = "0.7", features = ["enable"] }
fastrace-opentelemetry = { version = "0.18.0" }
opentelemetry = { version = "0.32.0", default-features = false, features = ["trace"] }
opentelemetry-otlp = { version = "0.32.0", default-features = false, features = ["trace", "grpc-tonic"] }
opentelemetry_sdk = { version = "0.32.0", default-features = false, features = ["trace"] }
```

Then, initialize `OpenTelemetryReporter` with OTLP exporter:

```rust,ignore
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
