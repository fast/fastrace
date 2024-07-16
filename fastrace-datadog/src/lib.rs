#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::net::SocketAddr;

use fastrace::collector::Reporter;
use fastrace::prelude::*;
use rmp_serde::Serializer;
use serde::Serialize;

/// [Datadog](https://docs.datadoghq.com/tracing/) reporter for `fastrace` in msgpack format.
pub struct DatadogReporter {
    agent_addr: SocketAddr,
    service_name: String,
    resource: String,
    trace_type: String,
}

impl DatadogReporter {
    pub fn new(
        agent_addr: SocketAddr,
        service_name: impl Into<String>,
        resource: impl Into<String>,
        trace_type: impl Into<String>,
    ) -> DatadogReporter {
        DatadogReporter {
            agent_addr,
            service_name: service_name.into(),
            resource: resource.into(),
            trace_type: trace_type.into(),
        }
    }

    fn convert<'a>(&'a self, spans: &'a [SpanRecord]) -> Vec<DatadogSpan<'a>> {
        spans
            .iter()
            .map(move |s| DatadogSpan {
                name: &s.name,
                service: &self.service_name,
                trace_type: &self.trace_type,
                resource: &self.resource,
                start: s.begin_time_unix_ns as i64,
                duration: s.duration_ns as i64,
                meta: if s.properties.is_empty() {
                    None
                } else {
                    Some(
                        s.properties
                            .iter()
                            .map(|(k, v)| (k.as_ref(), v.as_ref()))
                            .collect(),
                    )
                },
                error_code: 0,
                span_id: s.span_id.0,
                trace_id: s.trace_id.0 as u64,
                parent_id: s.parent_id.0,
            })
            .collect()
    }

    fn serialize(&self, spans: Vec<DatadogSpan>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buf = vec![0b10010001];
        spans.serialize(&mut Serializer::new(&mut buf).with_struct_map())?;
        Ok(buf)
    }

    fn try_report(&self, spans: &[SpanRecord]) -> Result<(), Box<dyn std::error::Error>> {
        let datadog_spans = self.convert(spans);
        let bytes = self.serialize(datadog_spans)?;
        let client = reqwest::blocking::Client::new();
        let _rep = client
            .post(format!("http://{}/v0.4/traces", self.agent_addr))
            .header("Datadog-Meta-Tracer-Version", "v1.27.0")
            .header("Content-Type", "application/msgpack")
            .body(bytes)
            .send()?;
        Ok(())
    }
}

impl Reporter for DatadogReporter {
    fn report(&mut self, spans: &[SpanRecord]) {
        if spans.is_empty() {
            return;
        }

        if let Err(err) = self.try_report(spans) {
            log::error!("report to datadog failed: {}", err);
        }
    }
}

#[derive(Serialize)]
struct DatadogSpan<'a> {
    name: &'a str,
    service: &'a str,
    #[serde(rename = "type")]
    trace_type: &'a str,
    resource: &'a str,
    start: i64,
    duration: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<HashMap<&'a str, &'a str>>,
    error_code: i32,
    span_id: u64,
    trace_id: u64,
    parent_id: u64,
}
