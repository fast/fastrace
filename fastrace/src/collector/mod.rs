// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! Collector and the collected spans.

#![cfg_attr(test, allow(dead_code))]

pub(crate) mod command;
mod console_reporter;
pub(crate) mod global_collector;
pub(crate) mod id;
mod test_reporter;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

pub use console_reporter::ConsoleReporter;
#[cfg(not(test))]
pub(crate) use global_collector::GlobalCollect;
#[cfg(test)]
pub(crate) use global_collector::MockGlobalCollect;
pub use global_collector::Reporter;
pub use id::SpanContext;
pub use id::SpanId;
pub use id::TraceId;
#[doc(hidden)]
pub use test_reporter::TestReporter;

use crate::local::local_collector::LocalSpansInner;
use crate::local::raw_span::RawSpan;

#[cfg(test)]
pub(crate) type GlobalCollect = Arc<MockGlobalCollect>;

#[doc(hidden)]
#[derive(Debug)]
pub enum SpanSet {
    Span(RawSpan),
    LocalSpansInner(LocalSpansInner),
    SharedLocalSpans(Arc<LocalSpansInner>),
}

/// A record of a span that includes all the information about the span,
/// such as its identifiers, timing information, name, and associated properties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpanRecord {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_id: SpanId,
    pub begin_time_unix_ns: u64,
    pub duration_ns: u64,
    pub name: Cow<'static, str>,
    pub properties: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    pub events: Vec<EventRecord>,
}

/// A record of an event that occurred during the execution of a span.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventRecord {
    pub name: Cow<'static, str>,
    pub timestamp_unix_ns: u64,
    pub properties: Vec<(Cow<'static, str>, Cow<'static, str>)>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CollectTokenItem {
    pub trace_id: TraceId,
    pub parent_id: SpanId,
    pub collect_id: usize,
    pub is_root: bool,
    pub is_sampled: bool,
}

/// Configuration of the behavior of the global collector.
#[must_use]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Config {
    pub(crate) report_interval: Duration,
    pub(crate) tail_sampled: bool,
}

impl Config {
    /// Sets the time duration between two reports. The reporter will be invoked when the specified
    /// duration elapses, even if no spans have been collected. This allows for batching in the
    /// reporter.
    ///
    /// In some scenarios, particularly under high load, you may notice spans being lost. This is
    /// likely due to the channel being full during the reporting interval. To mitigate this issue,
    /// consider reducing the report interval, potentially down to zero, to prevent losing spans.
    ///
    /// Defaults to 10 milliseconds.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::Config;
    ///
    /// let config = Config::default().report_interval(std::time::Duration::from_millis(100));
    /// fastrace::set_reporter(fastrace::collector::ConsoleReporter, config);
    /// ```
    pub fn report_interval(self, report_interval: Duration) -> Self {
        Self {
            report_interval,
            ..self
        }
    }

    /// Configures whether to hold spans before the root span finishes.
    ///
    /// This is useful for tail sampling, where child spans are held and allow the entire trace to
    /// be cancelled before the root span finishes.
    ///
    /// Defaults to `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::Config;
    /// use fastrace::collector::SpanContext;
    ///
    /// let config = Config::default().tail_sampled(true);
    /// fastrace::set_reporter(fastrace::collector::ConsoleReporter, config);
    ///
    /// let root = fastrace::Span::root("root", SpanContext::random());
    ///
    /// root.cancel();
    /// ```
    pub fn tail_sampled(self, tail_sampled: bool) -> Self {
        Self {
            tail_sampled,
            ..self
        }
    }

    /// Sets a soft limit for the total number of spans and events in a trace, typically
    /// used to prevent out-of-memory issues.
    ///
    /// # Note
    ///
    /// This is a no-op since it was deprecated.
    #[deprecated(since = "0.7.10")]
    pub fn max_spans_per_trace(self, _max_spans_per_trace: Option<usize>) -> Self {
        self
    }

    /// Configures whether to report spans before the root span finishes.
    #[deprecated(since = "0.7.10", note = "Use Config::tail_sampled() instead.")]
    pub fn report_before_root_finish(self, report_before_root_finish: bool) -> Self {
        self.tail_sampled(!report_before_root_finish)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            report_interval: Duration::from_millis(10),
            tail_sampled: false,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn w3c_traceparent() {
        let span_context = SpanContext::decode_w3c_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .unwrap();
        assert_eq!(
            span_context.trace_id,
            TraceId(0x0af7651916cd43dd8448eb211c80319c)
        );
        assert_eq!(span_context.span_id, SpanId(0xb7ad6b7169203331));

        assert_eq!(
            span_context.encode_w3c_traceparent(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
        assert_eq!(
            span_context.sampled(false).encode_w3c_traceparent(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00"
        );

        assert!(
            !SpanContext::decode_w3c_traceparent(
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00",
            )
            .unwrap()
            .sampled
        );
        assert!(
            SpanContext::decode_w3c_traceparent(
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            )
            .unwrap()
            .sampled
        );
        assert!(
            !SpanContext::decode_w3c_traceparent(
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-10",
            )
            .unwrap()
            .sampled
        );
    }
}
