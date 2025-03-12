// Copyright 2024 FastLabs Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This file is derived from [1] under the original license header:
// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
// [1]: https://github.com/tikv/minitrace-rust/blob/v0.6.4/minitrace/examples/asynchronous.rs

#![allow(clippy::new_without_default)]

use std::borrow::Cow;
use std::time::Duration;

use fastrace::collector::Config;
use fastrace::collector::Reporter;
use fastrace::prelude::*;
use opentelemetry_otlp::WithExportConfig;

fn parallel_job() -> Vec<tokio::task::JoinHandle<()>> {
    let mut v = Vec::with_capacity(4);
    for i in 0..4 {
        v.push(tokio::spawn(
            iter_job(i).in_span(Span::enter_with_local_parent("iter job")),
        ));
    }
    v
}

async fn iter_job(iter: u64) {
    std::thread::sleep(std::time::Duration::from_millis(iter * 10));
    tokio::task::yield_now().await;
    other_job().await;
}

#[trace(enter_on_poll = true)]
async fn other_job() {
    for i in 0..20 {
        if i == 10 {
            tokio::task::yield_now().await;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[tokio::main]
async fn main() {
    fastrace::set_reporter(ReportAll::create(), Config::default());

    {
        let parent = SpanContext::random();
        let span = Span::root("root", parent);

        let f = async {
            let jhs = {
                let _span = LocalSpan::enter_with_local_parent("a span")
                    .with_property(|| ("a property", "a value"));
                parallel_job()
            };

            other_job().await;

            for jh in jhs {
                jh.await.unwrap();
            }
        }
        .in_span(span);

        tokio::spawn(f).await.unwrap();
    }

    fastrace::flush();
}

pub struct ReportAll {
    jaeger: fastrace_jaeger::JaegerReporter,
    datadog: fastrace_datadog::DatadogReporter,
    opentelemetry: fastrace_opentelemetry::OpenTelemetryReporter,
}

impl ReportAll {
    pub fn create() -> ReportAll {
        ReportAll {
            jaeger: fastrace_jaeger::JaegerReporter::new(
                "127.0.0.1:6831".parse().unwrap(),
                "asynchronous",
            )
            .unwrap(),
            datadog: fastrace_datadog::DatadogReporter::new(
                "127.0.0.1:8126".parse().unwrap(),
                "asynchronous",
                "db",
                "select",
            ),
            opentelemetry: fastrace_opentelemetry::OpenTelemetryReporter::new(
                opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint("http://127.0.0.1:4317".to_string())
                    .with_protocol(opentelemetry_otlp::Protocol::Grpc)
                    .with_timeout(Duration::from_secs(
                        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT,
                    ))
                    .build()
                    .expect("initialize oltp exporter"),
                opentelemetry::trace::SpanKind::Server,
                Cow::Owned(
                    opentelemetry_sdk::Resource::builder()
                        .with_attributes([opentelemetry::KeyValue::new(
                            "service.name",
                            "asynchronous(opentelemetry)",
                        )])
                        .build(),
                ),
                opentelemetry::InstrumentationScope::builder("example-crate")
                    .with_version(env!("CARGO_PKG_VERSION"))
                    .build(),
            ),
        }
    }
}

impl Reporter for ReportAll {
    fn report(&mut self, spans: Vec<SpanRecord>) {
        self.jaeger.report(spans.clone());
        self.datadog.report(spans.clone());
        self.opentelemetry.report(spans);
    }
}
