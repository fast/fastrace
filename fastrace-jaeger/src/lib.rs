#![doc = include_str!("../README.md")]

mod thrift;

use std::error::Error;
use std::net::SocketAddr;
use std::net::UdpSocket;

use fastrace::collector::Reporter;
use fastrace::prelude::*;
use thrift::Log;
use thrift_codec::message::Message;
use thrift_codec::CompactEncode;

use crate::thrift::Batch;
use crate::thrift::EmitBatchNotification;
use crate::thrift::JaegerSpan;
use crate::thrift::Process;
use crate::thrift::Tag;

/// [Jaeger](https://www.jaegertracing.io/) reporter for `fastrace` via UDP endpoint.
pub struct JaegerReporter {
    agent_addr: SocketAddr,
    service_name: String,
    socket: UdpSocket,
}

impl JaegerReporter {
    pub fn new(
        agent_addr: SocketAddr,
        service_name: impl Into<String>,
    ) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let local_addr: SocketAddr = if agent_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }
        .parse()
        .unwrap();
        let socket = std::net::UdpSocket::bind(local_addr)?;

        Ok(Self {
            agent_addr,
            service_name: service_name.into(),
            socket,
        })
    }

    fn convert(&self, spans: &[SpanRecord]) -> Vec<JaegerSpan> {
        spans
            .iter()
            .map(move |s| JaegerSpan {
                trace_id_high: (s.trace_id.0 >> 64) as i64,
                trace_id_low: s.trace_id.0 as i64,
                span_id: s.span_id.0 as i64,
                parent_span_id: s.parent_id.0 as i64,
                operation_name: s.name.to_string(),
                references: vec![],
                flags: 1,
                start_time: (s.begin_time_unix_ns / 1_000) as i64,
                duration: (s.duration_ns / 1_000) as i64,
                tags: s
                    .properties
                    .iter()
                    .map(|(k, v)| Tag::String {
                        key: k.to_string(),
                        value: v.to_string(),
                    })
                    .collect(),
                logs: s
                    .events
                    .iter()
                    .map(|event| Log {
                        timestamp: (event.timestamp_unix_ns / 1_000) as i64,
                        fields: [("name".into(), event.name.clone())]
                            .iter()
                            .chain(&event.properties)
                            .map(|(k, v)| Tag::String {
                                key: k.to_string(),
                                value: v.to_string(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    fn serialize(&self, spans: Vec<JaegerSpan>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let bn = EmitBatchNotification {
            batch: Batch {
                process: Process {
                    service_name: self.service_name.clone(),
                    tags: vec![],
                },
                spans,
            },
        };

        let mut bytes = Vec::new();
        let msg = Message::from(bn);
        msg.compact_encode(&mut bytes)?;

        Ok(bytes)
    }

    fn try_report(&self, spans: &[SpanRecord]) -> Result<(), Box<dyn std::error::Error>> {
        const MAX_UDP_PACKAGE_SIZE: usize = 8000;

        let mut spans_per_batch = spans.len();
        let mut sent_spans = 0;

        while sent_spans < spans.len() {
            let batch_size = spans_per_batch.min(spans.len() - sent_spans);
            let jaeger_spans = self.convert(&spans[sent_spans..sent_spans + batch_size]);
            let bytes = self.serialize(jaeger_spans)?;
            if bytes.len() >= MAX_UDP_PACKAGE_SIZE {
                if batch_size <= 1 {
                    sent_spans += 1;
                } else {
                    spans_per_batch /= 2;
                }
                continue;
            }
            self.socket.send_to(&bytes, self.agent_addr)?;
            sent_spans += batch_size;
        }

        Ok(())
    }
}

impl Reporter for JaegerReporter {
    fn report(&mut self, spans: &[SpanRecord]) {
        if spans.is_empty() {
            return;
        }

        if let Err(err) = self.try_report(spans) {
            log::error!("report to jaeger failed: {}", err);
        }
    }
}
