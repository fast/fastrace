# fastrace-datadog

[![Documentation](https://docs.rs/fastrace-datadog/badge.svg)](https://docs.rs/fastrace-datadog/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-datadog.svg)](https://crates.io/crates/fastrace-datadog)
[![LICENSE](https://img.shields.io/github/license/fastracelabs/fastrace.svg)](https://github.com/fastracelabs/fastrace/blob/main/LICENSE)

[Datadog](https://docs.datadoghq.com/tracing/) reporter for [`fastrace`](https://crates.io/crates/fastrace).

## Dependencies

```toml
[dependencies]
fastrace = "0.6"
fastrace-datadog = "0.6"
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
