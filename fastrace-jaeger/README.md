# fastrace-jaeger

[![Documentation](https://docs.rs/fastrace-jaeger/badge.svg)](https://docs.rs/fastrace-jaeger/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-jaeger.svg)](https://crates.io/crates/fastrace-jaeger)
[![LICENSE](https://img.shields.io/github/license/fast/fastrace.svg)](https://github.com/fast/fastrace/blob/main/LICENSE)

[Jaeger](https://www.jaegertracing.io/) reporter for [`fastrace`](https://crates.io/crates/fastrace).

> [!WARNING]
>
> **`fastrace-jaeger` is deprecated** in favor of [`fastrace-opentelemetry`](https://crates.io/crates/fastrace-opentelemetry).
>
> It reports over Thrift/UDP to the (now deprecated) Jaeger agent. Jaeger has supported OTLP natively since v1.35 — the path Jaeger now recommends — and a dedicated Jaeger reporter only duplicates `fastrace-opentelemetry`. This crate will get one final release announcing the deprecation and will then be removed. See [Migrating to fastrace-opentelemetry](#migrating-to-fastrace-opentelemetry) below.

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

// Initialize reporter
let reporter =
    fastrace_jaeger::JaegerReporter::new("127.0.0.1:6831".parse().unwrap(), "asynchronous")
        .unwrap();
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush();
```

### Migrating to fastrace-opentelemetry

`fastrace-opentelemetry` exports via OTLP, which Jaeger accepts natively (v1.35+). Your instrumentation (`Span::root`, `#[trace]`, ...) stays the same — only the reporter changes. See the [`fastrace-opentelemetry`](https://crates.io/crates/fastrace-opentelemetry) README for the full `OpenTelemetryReporter` setup and a ready-to-run Jaeger stack (`dev/docker-compose.yaml`, UI on `16686`); point the OTLP endpoint at Jaeger (`http://127.0.0.1:4317`).
