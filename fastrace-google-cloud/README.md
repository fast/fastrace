# fastrace-google-cloud

[![Documentation](https://docs.rs/fastrace-google-cloud/badge.svg)](https://docs.rs/fastrace-google-cloud/)
[![Crates.io](https://img.shields.io/crates/v/fastrace-google-cloud.svg)](https://crates.io/crates/fastrace-google-cloud)
[![LICENSE](https://img.shields.io/github/license/fast/fastrace.svg)](https://github.com/fast/fastrace/blob/main/LICENSE)

[Datadog](https://docs.datadoghq.com/tracing/) reporter for [`fastrace`](https://crates.io/crates/fastrace).

## Dependencies

```toml
[dependencies]
fastrace = "0.7"
fastrace-google-cloud = "0.7"
google-cloud-trace-v2 = "0.2.0"
```

## Report to Google Cloud Trace

```rust
use std::net::SocketAddr;

use fastrace::collector::Config;
use fastrace::prelude::*;
use google_cloud_trace_v2::model::span::SpanKind;
use google_cloud_trace_v2::client::TraceService;

# async fn run_trace() {
// Initialize reporter
let trace_service = TraceService::builder().build().await.unwrap();
let reporter = fastrace_google_cloud::GoogleCloudReporter::new(
    trace_service,
    "project-id".to_string(),
);
fastrace::set_reporter(reporter, Config::default());

{
    // Start tracing
    let root = Span::root("root", SpanContext::random());
}

fastrace::flush();
# }
```
