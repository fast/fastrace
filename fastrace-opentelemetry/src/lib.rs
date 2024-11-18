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
use std::time::Duration;
use std::time::SystemTime;

use fastrace::collector::EventRecord;
use fastrace::collector::Reporter;
use fastrace::prelude::*;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry::trace::Event;
use opentelemetry::trace::SpanContext;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status;
use opentelemetry::trace::TraceFlags;
use opentelemetry::trace::TraceState;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::export::trace::SpanExporter;
use opentelemetry_sdk::trace::SpanEvents;
use opentelemetry_sdk::trace::SpanLinks;

/// [OpenTelemetry](https://github.com/open-telemetry/opentelemetry-rust) reporter for `fastrace`.
///
/// `OpenTelemetryReporter` exports trace records to remote agents that implements the
/// OpenTelemetry protocol, such as Jaeger, Zipkin, etc.
pub struct OpenTelemetryReporter {
    exporter: Box<dyn SpanExporter>,
    span_kind: SpanKind,
    instrumentation_scope: InstrumentationScope,
}

/// Convert a list of properties to a list of key-value pairs.
fn map_props_to_kvs(props: Vec<(Cow<'static, str>, Cow<'static, str>)>) -> Vec<KeyValue> {
    props
        .into_iter()
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

impl OpenTelemetryReporter {
    pub fn new(
        mut exporter: impl SpanExporter + 'static,
        span_kind: SpanKind,
        resource: Cow<'static, Resource>,
        instrumentation_scope: InstrumentationScope,
    ) -> Self {
        exporter.set_resource(&resource);
        OpenTelemetryReporter {
            exporter: Box::new(exporter),
            span_kind,
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
                    let span_context = SpanContext::new(
                        trace_id.0.into(),
                        span_id.0.into(),
                        TraceFlags::default(),
                        false,
                        TraceState::default(),
                    );
                    let parent_span_id = parent_id.0.into();
                    let span_kind = self.span_kind.clone();
                    let instrumentation_scope = self.instrumentation_scope.clone();
                    let start_time =
                        SystemTime::UNIX_EPOCH + Duration::from_nanos(begin_time_unix_ns);
                    let end_time = SystemTime::UNIX_EPOCH
                        + Duration::from_nanos(begin_time_unix_ns + duration_ns);
                    let attributes = map_props_to_kvs(properties);
                    let events = map_events(events);
                    SpanData {
                        span_context,
                        parent_span_id,
                        span_kind,
                        name,
                        start_time,
                        end_time,
                        attributes,
                        dropped_attributes_count: 0,
                        events,
                        links: SpanLinks::default(),
                        status: Status::default(),
                        instrumentation_scope,
                    }
                },
            )
            .collect()
    }

    fn try_report(&mut self, spans: Vec<SpanRecord>) -> Result<(), Box<dyn std::error::Error>> {
        let spans = self.convert(spans);
        futures::executor::block_on(self.exporter.export(spans))?;
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
