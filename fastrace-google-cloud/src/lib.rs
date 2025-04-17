// Copyright 2025 FastLabs Developers
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

#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use fastrace::collector::{EventRecord, Reporter};
use fastrace::prelude::*;
use google_cloud_rpc::model::Status;
use google_cloud_trace_v2::client::TraceService;
use google_cloud_trace_v2::model::span::time_event::Annotation;
use google_cloud_trace_v2::model::span::{Attributes, SpanKind, TimeEvent, TimeEvents};
use google_cloud_trace_v2::model::{
    AttributeValue, Span as GoogleSpan, StackTrace, TruncatableString,
};
use google_cloud_wkt::Timestamp;
use opentelemetry_semantic_conventions::attribute as attribute_sem;

pub struct GoogleCloudReporter {
    tokio_runtime: std::sync::LazyLock<tokio::runtime::Runtime>,
    client: TraceService,
    trace_project_id: String,
    attribute_name_mappings: Option<HashMap<&'static str, &'static str>>,
    status_converter: fn(&SpanRecord, &mut HashMap<String, AttributeValue>) -> Option<Status>,
    span_kind_converter: fn(&SpanRecord, &mut HashMap<String, AttributeValue>) -> SpanKind,
    stack_trace_converter:
        fn(&SpanRecord, &mut HashMap<String, AttributeValue>) -> Option<StackTrace>,
}

pub fn opentelemetry_semantic_mapping() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (attribute_sem::OTEL_COMPONENT_TYPE, "/component"),
        (attribute_sem::EXCEPTION_MESSAGE, "/error/message"),
        (attribute_sem::EXCEPTION_MESSAGE, "/error/name"),
        (
            attribute_sem::NETWORK_PROTOCOL_VERSION,
            "/http/client_protocol",
        ),
        (attribute_sem::HTTP_HOST, "/http/host"),
        (attribute_sem::HTTP_METHOD, "/http/method"),
        (attribute_sem::HTTP_REQUEST_METHOD, "/http/method"),
        // Not a standard OTEL attribute, but some existing systems have this mapping
        ("http.path", "/http/path"),
        (attribute_sem::URL_PATH, "/http/path"),
        (attribute_sem::HTTP_REQUEST_SIZE, "/http/request/size"),
        (attribute_sem::HTTP_RESPONSE_SIZE, "/http/response/size"),
        (attribute_sem::HTTP_ROUTE, "/http/route"),
        (
            attribute_sem::HTTP_RESPONSE_STATUS_CODE,
            "/http/status_code",
        ),
        (attribute_sem::HTTP_STATUS_CODE, "/http/status_code"),
        (attribute_sem::HTTP_USER_AGENT, "/http/user_agent"),
        (attribute_sem::USER_AGENT_ORIGINAL, "/http/user_agent"),
        (
            attribute_sem::K8S_CLUSTER_NAME,
            "g.co/r/k8s_container/cluster_name",
        ),
        (
            attribute_sem::K8S_NAMESPACE_NAME,
            "g.co/r/k8s_container/namespace",
        ),
        (attribute_sem::K8S_POD_NAME, "g.co/r/k8s_container/pod_name"),
        (
            attribute_sem::K8S_CONTAINER_NAME,
            "g.co/r/k8s_container/container_name",
        ),
    ])
}

impl GoogleCloudReporter {
    pub fn new(client: TraceService, trace_project_id: String) -> Self {
        Self {
            tokio_runtime: LazyLock::new(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .unwrap()
            }),
            client,
            trace_project_id,
            attribute_name_mappings: None,
            status_converter: |_, _| None,
            span_kind_converter: |_, attribute_map| {
                let span_kind = attribute_map.remove("span.kind");

                span_kind
                    .as_ref()
                    .and_then(|value| value.string_value())
                    .and_then(|s| SpanKind::from_str_name(&s.value))
                    .unwrap_or(SpanKind::INTERNAL)
            },
            stack_trace_converter: |_, _| None,
        }
    }

    pub fn attribute_name_mappings(
        mut self,
        attribute_name_mappings: Option<HashMap<&'static str, &'static str>>,
    ) -> Self {
        self.attribute_name_mappings = attribute_name_mappings;
        self
    }

    pub fn status_converter(
        mut self,
        status_converter: fn(&SpanRecord, &mut HashMap<String, AttributeValue>) -> Option<Status>,
    ) -> Self {
        self.status_converter = status_converter;
        self
    }

    pub fn span_kind_converter(
        mut self,
        span_kind_converter: fn(&SpanRecord, &mut HashMap<String, AttributeValue>) -> SpanKind,
    ) -> Self {
        self.span_kind_converter = span_kind_converter;
        self
    }

    pub fn stack_trace_converter(
        mut self,
        stack_trace_converter: fn(
            &SpanRecord,
            &mut HashMap<String, AttributeValue>,
        ) -> Option<StackTrace>,
    ) -> Self {
        self.stack_trace_converter = stack_trace_converter;
        self
    }

    fn convert_span(&self, span: SpanRecord) -> GoogleSpan {
        let span_id = convert_span_id(span.span_id);

        let mut attributes =
            convert_properties(&span.properties, self.attribute_name_mappings.as_ref());
        let status = (self.status_converter)(&span, &mut attributes.attribute_map);
        let span_kind = (self.span_kind_converter)(&span, &mut attributes.attribute_map);
        let stack_trace = (self.stack_trace_converter)(&span, &mut attributes.attribute_map);

        let mut google_span = GoogleSpan::new()
            .set_name(format!(
                "projects/{}/traces/{:016x}/spans/{}",
                self.trace_project_id, span.trace_id.0, span_id
            ))
            .set_span_id(span_id)
            .set_display_name(TruncatableString::new().set_value(span.name))
            .set_start_time(convert_unix_ns(span.begin_time_unix_ns))
            .set_end_time(convert_unix_ns(span.begin_time_unix_ns + span.duration_ns))
            .set_attributes(attributes)
            .set_status(status)
            .set_span_kind(span_kind)
            .set_stack_trace(stack_trace)
            .set_time_events(
                TimeEvents::new()
                    .set_time_event(span.events.into_iter().map(|e| self.convert_event(e))),
            );

        if let Some(parent_span_id) = convert_parent_span_id(span.parent_id) {
            google_span = google_span.set_parent_span_id(parent_span_id);
        }

        google_span
    }

    fn convert_event(&self, event: EventRecord) -> TimeEvent {
        TimeEvent::new()
            .set_time(convert_unix_ns(event.timestamp_unix_ns))
            .set_annotation(
                Annotation::new()
                    .set_attributes(convert_properties(
                        &event.properties,
                        self.attribute_name_mappings.as_ref(),
                    ))
                    .set_description(TruncatableString::new().set_value(event.name)),
            )
    }

    fn try_report(&self, spans: Vec<SpanRecord>) -> google_cloud_trace_v2::Result<()> {
        let spans = spans
            .into_iter()
            .map(|s| self.convert_span(s))
            .collect::<Vec<_>>();
        log::error!(spans:serde; "Reporting these spans");
        self.tokio_runtime.block_on(
            self.client
                .batch_write_spans(format!("projects/{}", self.trace_project_id))
                .set_spans(spans)
                .send(),
        )
    }
}

impl Reporter for GoogleCloudReporter {
    fn report(&mut self, spans: Vec<SpanRecord>) {
        if spans.is_empty() {
            return;
        }

        if let Err(err) = self.try_report(spans) {
            log::error!("report to Google Cloud Trace failed: {}", err);
        }
    }
}

fn convert_properties(
    properties: &[(Cow<'static, str>, Cow<'static, str>)],
    attribute_name_mappings: Option<&HashMap<&'static str, &'static str>>,
) -> Attributes {
    let attributes = properties.iter().map(|(k, v)| {
        let key = attribute_name_mappings
            .as_ref()
            .and_then(|m| m.get(k.as_ref()).cloned())
            .unwrap_or(k.as_ref());
        (
            key.to_string(),
            AttributeValue::new()
                .set_string_value(TruncatableString::new().set_value(v.to_string())),
        )
    });

    Attributes::new().set_attribute_map(attributes)
}

fn convert_unix_ns(unix_time: u64) -> Timestamp {
    Timestamp::clamp(
        (unix_time / 1_000_000_000) as i64,
        (unix_time % 1_000_000_000) as i32,
    )
}

/// Convert a span ID to a string representation.
fn convert_span_id(span_id: SpanId) -> String {
    format!("{:08x}", span_id.0)
}

/// Convert a parent span ID to a string representation.
///
/// Returns `None` if the parent span ID is invalid (zero).
fn convert_parent_span_id(span_id: SpanId) -> Option<String> {
    if span_id.0 == 0 {
        None
    } else {
        Some(convert_span_id(span_id))
    }
}
