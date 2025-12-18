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
// [1]: https://github.com/tikv/minitrace-rust/blob/v0.6.4/minitrace-opentelemetry/src/lib.rs

#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::SystemTime;

use fastrace::collector::EventRecord;
use fastrace::collector::Reporter;
use fastrace::prelude::*;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry::trace::Event;
use opentelemetry::trace::SpanContext as OtelSpanContext;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status;
use opentelemetry::trace::TraceFlags;
use opentelemetry::trace::TraceState;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanEvents;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::trace::SpanLinks;

/// [OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust) reporter for `fastrace`.
///
/// `OpenTelemetryReporter` exports trace records to remote agents that implements the
/// OpenTelemetry protocol, such as Jaeger, Zipkin, etc.
///
/// ## Span Kind
///
/// The reporter automatically maps the `span.kind` property from fastrace spans to OpenTelemetry
/// span kinds. Supported values are: "client", "server", "producer", "consumer", and "internal"
/// (case-insensitive). If no `span.kind` property is provided, spans default to
/// `SpanKind::Internal`.
///
/// ## Span Status
///
/// The reporter maps the `span.status_code` and `span.status_description` properties from fastrace
/// spans to OpenTelemetry span status. Supported codes are: "unset", "ok", and "error"
/// (case-insensitive). If no `span.status_code` property is provided, spans default to
/// `Status::Unset`. If the code is "error", the `span.status_description` property is used as the
/// error description.
///
/// ## Parent Span Is Remote
///
/// The reporter maps the `span.parent_span_is_remote` property from fastrace spans to indicate
/// whether the parent span is remote. Supported values are "true" and "false" (case-insensitive).
/// If no `span.parent_span_is_remote` property is provided, it defaults to `false`.
pub struct OpenTelemetryReporter {
    exporter: Box<dyn DynSpanExporter>,
    instrumentation_scope: InstrumentationScope,
}

/// Returns the OpenTelemetry [`SpanContext`] of the current fastrace local parent span.
///
/// This helper bridges fastrace's **thread-local parent stack** (set via
/// [`Span::set_local_parent`]) into an OpenTelemetry
/// `SpanContext` so you can interoperate with OpenTelemetry-based instrumentation.
///
/// It returns `None` when:
/// - fastrace's `enable` feature is disabled (the local parent stack is inert), or
/// - no local parent is currently set for the thread.
///
/// The returned span context is **non-recording** (it does not create an OpenTelemetry span on
/// its own). To make it usable as a parent for OpenTelemetry spans, attach it to an
/// OpenTelemetry [`Context`](opentelemetry::Context) via
/// [`TraceContextExt::with_remote_span_context`](opentelemetry::trace::TraceContextExt::with_remote_span_context).
///
/// # Examples
///
/// ```rust, no_run
/// use fastrace::prelude::*;
/// use opentelemetry::Context;
/// use opentelemetry::trace::TraceContextExt;
///
/// let root = Span::root("root", SpanContext::random());
/// let _g = root.set_local_parent();
///
/// // Make the fastrace span the "current" OpenTelemetry parent for this thread.
/// let _otel_guard = fastrace_opentelemetry::current_opentelemetry_context()
///     .map(|cx| Context::current().with_remote_span_context(cx).attach());
///
/// // Any OpenTelemetry instrumentation that reads `Context::current()` can now
/// // treat the fastrace span as its parent.
/// ```
pub fn current_opentelemetry_context() -> Option<OtelSpanContext> {
    let span_context = fastrace::collector::SpanContext::current_local_parent()?;

    let trace_flags = if span_context.sampled {
        TraceFlags::SAMPLED
    } else {
        TraceFlags::default()
    };

    Some(OtelSpanContext::new(
        span_context.trace_id.0.into(),
        span_context.span_id.0.into(),
        trace_flags,
        false,
        TraceState::default(),
    ))
}

pub const SPAN_KIND: &str = "span.kind";
pub const SPAN_STATUS_CODE: &str = "span.status_code";
pub const SPAN_STATUS_DESCRIPTION: &str = "span.status_description";
pub const SPAN_PARENT_SPAN_IS_REMOTE: &str = "span.parent_span_is_remote";

static OTEL_PROPERTIES: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    HashSet::from([
        SPAN_KIND,
        SPAN_STATUS_CODE,
        SPAN_STATUS_DESCRIPTION,
        SPAN_PARENT_SPAN_IS_REMOTE,
    ])
});

/// Convert a list of properties to a list of key-value pairs.
fn map_props_to_kvs(props: Vec<(Cow<'static, str>, Cow<'static, str>)>) -> Vec<KeyValue> {
    props
        .into_iter()
        .filter(|(k, _)| !OTEL_PROPERTIES.contains(k.as_ref()))
        .map(|(k, v)| KeyValue::new(k, v))
        .collect()
}

/// Convert a list of [`EventRecord`] to OpenTelemetry [`SpanEvents`].
fn map_events(events: Vec<EventRecord>) -> SpanEvents {
    let mut queue = SpanEvents::default();
    queue.events.reserve(events.len());

    for EventRecord {
        name,
        timestamp_unix_ns,
        properties,
    } in events
    {
        let time = SystemTime::UNIX_EPOCH + Duration::from_nanos(timestamp_unix_ns);
        let attributes = map_props_to_kvs(properties);
        queue.events.push(Event::new(name, time, attributes, 0));
    }

    queue
}

trait DynSpanExporter: Send + Sync + Debug {
    fn export(
        &self,
        batch: Vec<SpanData>,
    ) -> Pin<Box<dyn Future<Output = OTelSdkResult> + Send + '_>>;
}

impl<T: SpanExporter> DynSpanExporter for T {
    fn export(
        &self,
        batch: Vec<SpanData>,
    ) -> Pin<Box<dyn Future<Output = OTelSdkResult> + Send + '_>> {
        Box::pin(SpanExporter::export(self, batch))
    }
}

impl OpenTelemetryReporter {
    pub fn new(
        mut exporter: impl SpanExporter + 'static,
        resource: Cow<'static, Resource>,
        instrumentation_scope: InstrumentationScope,
    ) -> Self {
        exporter.set_resource(&resource);
        OpenTelemetryReporter {
            exporter: Box::new(exporter),
            instrumentation_scope,
        }
    }

    fn convert(&self, spans: Vec<SpanRecord>) -> Vec<SpanData> {
        spans
            .into_iter()
            .map(
                |SpanRecord {
                     trace_id,
                     span_id,
                     parent_id,
                     begin_time_unix_ns,
                     duration_ns,
                     name,
                     properties,
                     events,
                 }| {
                    let parent_span_id = parent_id.0.into();
                    let span_kind = span_kind(&properties);
                    let status = span_status(&properties);
                    let parent_span_is_remote = parent_span_is_remote(&properties);
                    let instrumentation_scope = self.instrumentation_scope.clone();
                    let start_time =
                        SystemTime::UNIX_EPOCH + Duration::from_nanos(begin_time_unix_ns);
                    let end_time = SystemTime::UNIX_EPOCH
                        + Duration::from_nanos(begin_time_unix_ns + duration_ns);
                    let attributes = map_props_to_kvs(properties);
                    let events = map_events(events);

                    SpanData {
                        span_context: OtelSpanContext::new(
                            trace_id.0.into(),
                            span_id.0.into(),
                            TraceFlags::default(),
                            parent_span_is_remote,
                            TraceState::default(),
                        ),
                        parent_span_id,
                        parent_span_is_remote,
                        span_kind,
                        name,
                        start_time,
                        end_time,
                        attributes,
                        dropped_attributes_count: 0,
                        events,
                        links: SpanLinks::default(),
                        status,
                        instrumentation_scope,
                    }
                },
            )
            .collect()
    }

    fn try_report(&mut self, spans: Vec<SpanRecord>) -> Result<(), Box<dyn std::error::Error>> {
        let spans = self.convert(spans);
        pollster::block_on(self.exporter.export(spans))?;
        Ok(())
    }
}

impl Reporter for OpenTelemetryReporter {
    fn report(&mut self, spans: Vec<SpanRecord>) {
        if spans.is_empty() {
            return;
        }

        if let Err(err) = self.try_report(spans) {
            log::error!("failed to report to opentelemetry: {err}");
        }
    }
}

fn span_kind(properties: &[(Cow<'static, str>, Cow<'static, str>)]) -> SpanKind {
    properties
        .iter()
        .find(|(k, _)| k == SPAN_KIND)
        .and_then(|(_, v)| match v.to_lowercase().as_str() {
            "client" => Some(SpanKind::Client),
            "server" => Some(SpanKind::Server),
            "producer" => Some(SpanKind::Producer),
            "consumer" => Some(SpanKind::Consumer),
            "internal" => Some(SpanKind::Internal),
            _ => None,
        })
        .unwrap_or(SpanKind::Internal)
}

fn span_status(properties: &[(Cow<'static, str>, Cow<'static, str>)]) -> Status {
    let status_description = properties
        .iter()
        .find(|(k, _)| k == SPAN_STATUS_DESCRIPTION)
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();
    properties
        .iter()
        .find(|(k, _)| k == SPAN_STATUS_CODE)
        .and_then(|(_, v)| match v.to_lowercase().as_str() {
            "unset" => Some(Status::Unset),
            "ok" => Some(Status::Ok),
            "error" => Some(Status::Error {
                description: status_description.into(),
            }),
            _ => None,
        })
        .unwrap_or(Status::Unset)
}

fn parent_span_is_remote(properties: &[(Cow<'static, str>, Cow<'static, str>)]) -> bool {
    properties
        .iter()
        .find(|(k, _)| k == SPAN_PARENT_SPAN_IS_REMOTE)
        .map(|(_, v)| v.to_lowercase().as_str() == "true")
        .unwrap_or(false)
}
