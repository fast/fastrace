// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use crate::Span;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;

thread_local! {
    static LOCAL_ID_GENERATOR: Cell<(u32, u32)> = Cell::new((rand::random(), 0))
}

/// An identifier for a trace, which groups a set of related spans together.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TraceId(pub u128);

impl TraceId {
    /// Create a random `TraceId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let trace_id = TraceId::random();
    /// ```
    pub fn random() -> Self {
        TraceId(rand::random())
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl FromStr for TraceId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u128::from_str_radix(s, 16).map(TraceId)
    }
}

impl serde::Serialize for TraceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:032x}", self.0))
    }
}

impl<'de> serde::Deserialize<'de> for TraceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        u128::from_str_radix(&s, 16)
            .map(TraceId)
            .map_err(serde::de::Error::custom)
    }
}

/// An identifier for a span within a trace.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SpanId(pub u64);

impl SpanId {
    /// Create a random `SpanId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_id = SpanId::random();
    /// ```
    pub fn random() -> Self {
        SpanId(rand::random())
    }

    #[inline]
    #[doc(hidden)]
    /// Create a non-zero `SpanId`
    pub fn next_id() -> SpanId {
        LOCAL_ID_GENERATOR
            .try_with(|g| {
                let (prefix, mut suffix) = g.get();

                suffix = suffix.wrapping_add(1);

                g.set((prefix, suffix));

                SpanId(((prefix as u64) << 32) | (suffix as u64))
            })
            .unwrap_or_else(|_| SpanId(rand::random()))
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl FromStr for SpanId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str_radix(s, 16).map(SpanId)
    }
}

impl serde::Serialize for SpanId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}

impl<'de> serde::Deserialize<'de> for SpanId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        u64::from_str_radix(&s, 16)
            .map(SpanId)
            .map_err(serde::de::Error::custom)
    }
}

/// A struct representing the context of a span, including its [`TraceId`] and [`SpanId`].
///
/// [`TraceId`]: crate::collector::TraceId
/// [`SpanId`]: crate::collector::SpanId
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub sampled: bool,
}

impl SpanContext {
    /// Creates a new `SpanContext` with the given [`TraceId`] and [`SpanId`].
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_context = SpanContext::new(TraceId(12), SpanId::default());
    /// ```
    ///
    /// [`TraceId`]: crate::collector::TraceId
    /// [`SpanId`]: crate::collector::SpanId
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        Self {
            trace_id,
            span_id,
            sampled: true,
        }
    }

    /// Create a new `SpanContext` with a random trace id.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// ```
    pub fn random() -> Self {
        Self {
            trace_id: TraceId::random(),
            span_id: SpanId(0),
            sampled: true,
        }
    }

    /// Sets the `sampled` flag of the `SpanContext`.
    ///
    /// When the `sampled` flag is `false`, the spans will not be collected, but the parent-child
    /// relationship will still be maintained and the `SpanContext` can still be propagated.
    ///
    /// The default value is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_context = SpanContext::new(TraceId(12), SpanId(34)).sampled(false);
    /// ```
    pub fn sampled(mut self, sampled: bool) -> Self {
        self.sampled = sampled;
        self
    }

    /// Creates a `SpanContext` from the given [`Span`]. If the `Span` is a noop span,
    /// this function will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span = Span::root("root", SpanContext::random());
    /// let span_context = SpanContext::from_span(&span);
    /// ```
    ///
    /// [`Span`]: crate::Span
    pub fn from_span(span: &Span) -> Option<Self> {
        #[cfg(not(feature = "enable"))]
        {
            None
        }

        #[cfg(feature = "enable")]
        {
            let inner = span.inner.as_ref()?;
            let collect_token = inner.issue_collect_token();

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
                sampled: collect_token.is_sampled,
            })
        }
    }

    /// Creates a `SpanContext` from the current local parent span. If there is no
    /// local parent span, this function will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span = Span::root("root", SpanContext::random());
    /// let _guard = span.set_local_parent();
    ///
    /// let span_context = SpanContext::current_local_parent();
    /// ```
    pub fn current_local_parent() -> Option<Self> {
        #[cfg(not(feature = "enable"))]
        {
            None
        }

        #[cfg(feature = "enable")]
        {
            let stack = LOCAL_SPAN_STACK.try_with(Rc::clone).ok()?;

            let mut stack = stack.borrow_mut();
            let collect_token = stack.current_collect_token()?;

            Some(Self {
                trace_id: collect_token.trace_id,
                span_id: collect_token.parent_id,
                sampled: collect_token.is_sampled,
            })
        }
    }

    /// Decodes the `SpanContext` from a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_context = SpanContext::decode_w3c_traceparent(
    ///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(
    ///     span_context.trace_id,
    ///     TraceId(0x0af7651916cd43dd8448eb211c80319c)
    /// );
    /// assert_eq!(span_context.span_id, SpanId(0xb7ad6b7169203331));
    /// ```
    pub fn decode_w3c_traceparent(traceparent: &str) -> Option<Self> {
        let mut parts = traceparent.split('-');

        match (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            (Some("00"), Some(trace_id), Some(span_id), Some(sampled), None) => {
                let trace_id = u128::from_str_radix(trace_id, 16).ok()?;
                let span_id = u64::from_str_radix(span_id, 16).ok()?;
                let sampled = u8::from_str_radix(sampled, 16).ok()? & 1 == 1;
                if trace_id == 0 || span_id == 0 {
                    return None;
                }
                Some(Self::new(TraceId(trace_id), SpanId(span_id)).sampled(sampled))
            }
            _ => None,
        }
    }

    /// Encodes the `SpanContext` into a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_context = SpanContext::new(TraceId(12), SpanId(34));
    /// let traceparent = span_context.encode_w3c_traceparent();
    ///
    /// assert_eq!(
    ///     traceparent,
    ///     "00-0000000000000000000000000000000c-0000000000000022-01"
    /// );
    /// ```
    pub fn encode_w3c_traceparent(&self) -> String {
        format!(
            "00-{:032x}-{:016x}-{:02x}",
            self.trace_id.0, self.span_id.0, self.sampled as u8,
        )
    }

    /// Encodes the `SpanContext` as a [W3C Trace Context](https://www.w3.org/TR/trace-context/)
    /// `traceparent` header string with a sampled flag.
    #[deprecated(since = "0.7.0", note = "Please use `SpanContext::sampled()` instead")]
    pub fn encode_w3c_traceparent_with_sampled(&self, sampled: bool) -> String {
        self.sampled(sampled).encode_w3c_traceparent()
    }
}

impl Default for SpanContext {
    fn default() -> Self {
        Self::random()
    }
}

impl serde::Serialize for SpanContext {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.encode_w3c_traceparent().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for SpanContext {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        SpanContext::decode_w3c_traceparent(&s)
            .ok_or_else(|| serde::de::Error::custom("invalid w3c traceparent"))
    }
}

/// A complete [W3C Trace Context](https://www.w3.org/TR/trace-context/) representation
/// carrying both the `traceparent` and `tracestate` headers.
///
/// [`SpanContext`] is `Copy` and only stores trace-id, span-id, and the sampled flag
/// (the `traceparent` portion). `W3CTraceContext` extends it with an optional
/// `tracestate` string so that vendor-specific key-value pairs survive propagation
/// across process boundaries.
///
/// **Important:** `W3CTraceContext` is a **boundary/header wrapper** for encoding and
/// decoding W3C headers at RPC injection/extraction points. The `tracestate` field is
/// **not** carried through fastrace's internal span machinery — converting to a
/// [`SpanContext`] (e.g. via [`Span::root`] or field access) discards the tracestate.
/// **If you extract the `span_context`, create spans, and later need to inject outgoing
/// headers via `SpanContext::from_span` or `SpanContext::current_local_parent()`, the
/// original `tracestate` will be lost.** Store the `W3CTraceContext` or `tracestate`
/// value separately if you need to preserve it for outbound propagation.
///
/// # Examples
///
/// ```
/// use fastrace::collector::W3CTraceContext;
/// use fastrace::prelude::*;
///
/// // Decode from incoming HTTP headers.
/// let ctx = W3CTraceContext::decode(
///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
///     Some("rw=frontend,congo=t61rcWkgMzE"),
/// )
/// .unwrap();
///
/// assert_eq!(
///     ctx.span_context.trace_id,
///     TraceId(0x0af7651916cd43dd8448eb211c80319c)
/// );
/// assert_eq!(ctx.tracestate(), Some("rw=frontend,congo=t61rcWkgMzE"));
///
/// // Encode for outgoing HTTP headers.
/// let traceparent = ctx.encode_traceparent();
/// let tracestate = ctx.encode_tracestate();
/// assert_eq!(
///     traceparent,
///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
/// );
/// assert_eq!(tracestate, Some("rw=frontend,congo=t61rcWkgMzE"));
///
/// // Start a root span (note: tracestate is not retained in the span).
/// let root = Span::root("server", ctx.span_context);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct W3CTraceContext {
    /// The core span context (trace-id, span-id, sampled flag).
    pub span_context: SpanContext,
    tracestate: Option<String>,
}

impl W3CTraceContext {
    /// Creates a new `W3CTraceContext` from a [`SpanContext`] with no `tracestate`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::W3CTraceContext;
    /// use fastrace::prelude::*;
    ///
    /// let ctx = W3CTraceContext::new(SpanContext::new(TraceId(12), SpanId(34)));
    /// assert_eq!(ctx.tracestate(), None);
    /// ```
    pub fn new(span_context: SpanContext) -> Self {
        Self {
            span_context,
            tracestate: None,
        }
    }

    /// Attaches a `tracestate` string. Replaces any existing value.
    ///
    /// An empty string is normalized to `None` (no tracestate), consistent
    /// with [`W3CTraceContext::decode`].
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::W3CTraceContext;
    /// use fastrace::prelude::*;
    ///
    /// let ctx = W3CTraceContext::new(SpanContext::random()).with_tracestate("rw=frontend");
    /// assert_eq!(ctx.tracestate(), Some("rw=frontend"));
    ///
    /// // Empty string is normalized to None.
    /// let ctx = ctx.with_tracestate("");
    /// assert_eq!(ctx.tracestate(), None);
    /// ```
    pub fn with_tracestate(mut self, tracestate: impl Into<String>) -> Self {
        let ts = tracestate.into();
        self.tracestate = if ts.is_empty() { None } else { Some(ts) };
        self
    }

    /// Returns the `tracestate` value, if any.
    pub fn tracestate(&self) -> Option<&str> {
        self.tracestate.as_deref()
    }

    /// Encodes the `traceparent` header value.
    ///
    /// This delegates to [`SpanContext::encode_w3c_traceparent`].
    pub fn encode_traceparent(&self) -> String {
        self.span_context.encode_w3c_traceparent()
    }

    /// Returns the `tracestate` header value, if present.
    ///
    /// Returns `None` when no tracestate was set, meaning the header should be
    /// omitted from the outgoing request.
    pub fn encode_tracestate(&self) -> Option<&str> {
        self.tracestate.as_deref()
    }

    /// Decodes a `W3CTraceContext` from `traceparent` and optional `tracestate`
    /// header values.
    ///
    /// Returns `None` if the `traceparent` string is malformed.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::W3CTraceContext;
    /// use fastrace::prelude::*;
    ///
    /// let ctx = W3CTraceContext::decode(
    ///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    ///     Some("rw=frontend"),
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(ctx.tracestate(), Some("rw=frontend"));
    ///
    /// // Without tracestate.
    /// let ctx2 = W3CTraceContext::decode(
    ///     "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    ///     None,
    /// )
    /// .unwrap();
    /// assert_eq!(ctx2.tracestate(), None);
    /// ```
    pub fn decode(traceparent: &str, tracestate: Option<&str>) -> Option<Self> {
        let span_context = SpanContext::decode_w3c_traceparent(traceparent)?;
        Some(Self {
            span_context,
            tracestate: tracestate.filter(|s| !s.is_empty()).map(|s| s.to_string()),
        })
    }

    /// Encodes both `traceparent` and `tracestate` into a list of header
    /// key-value pairs suitable for HTTP propagation.
    ///
    /// The returned vector always contains the `traceparent` entry. The
    /// `tracestate` entry is included only when a tracestate value is present.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::W3CTraceContext;
    /// use fastrace::prelude::*;
    ///
    /// let ctx = W3CTraceContext::new(SpanContext::new(TraceId(1), SpanId(2)))
    ///     .with_tracestate("rw=frontend");
    /// let headers = ctx.encode_headers();
    /// assert_eq!(headers.len(), 2);
    /// assert_eq!(headers[0].0, "traceparent");
    /// assert_eq!(headers[1].0, "tracestate");
    /// assert_eq!(headers[1].1, "rw=frontend");
    /// ```
    pub fn encode_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![("traceparent".to_string(), self.encode_traceparent())];
        if let Some(ts) = &self.tracestate {
            headers.push(("tracestate".to_string(), ts.clone()));
        }
        headers
    }

    /// Decodes a `W3CTraceContext` from an iterator of header key-value pairs.
    ///
    /// The lookup is case-insensitive for the header names, per the W3C spec.
    /// **If multiple `tracestate` headers are present, they are joined with commas
    /// in the order encountered, per [W3C Trace Context §3.3.1.1](https://www.w3.org/TR/trace-context/#tracestate-header).**
    /// Empty or whitespace-only `tracestate` values are ignored.
    /// Returns `None` if no valid `traceparent` header is found.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::collector::W3CTraceContext;
    ///
    /// let headers = vec![
    ///     (
    ///         "traceparent",
    ///         "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    ///     ),
    ///     ("tracestate", "rw=frontend,congo=t61rcWkgMzE"),
    /// ];
    ///
    /// let ctx = W3CTraceContext::decode_headers(headers).unwrap();
    /// assert_eq!(ctx.tracestate(), Some("rw=frontend,congo=t61rcWkgMzE"));
    /// ```
    pub fn decode_headers<'a>(
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Option<Self> {
        let mut traceparent = None;
        let mut tracestate_parts: Vec<&str> = Vec::new();

        for (key, value) in headers {
            if key.eq_ignore_ascii_case("traceparent") {
                traceparent = Some(value);
            } else if key.eq_ignore_ascii_case("tracestate") {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    tracestate_parts.push(trimmed);
                }
            }
        }

        let tracestate = if tracestate_parts.is_empty() {
            None
        } else {
            Some(tracestate_parts.join(","))
        };

        Self::decode(traceparent?, tracestate.as_deref())
    }
}

impl From<SpanContext> for W3CTraceContext {
    fn from(span_context: SpanContext) -> Self {
        Self::new(span_context)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    #[allow(clippy::needless_collect)]
    fn unique_id() {
        let handles = std::iter::repeat_with(|| {
            std::thread::spawn(|| {
                std::iter::repeat_with(SpanId::next_id)
                    .take(1000)
                    .collect::<Vec<_>>()
            })
        })
        .take(32)
        .collect::<Vec<_>>();

        let k = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect::<HashSet<_>>();

        assert_eq!(k.len(), 32 * 1000);
    }

    #[test]
    fn w3c_trace_context_decode_with_tracestate() {
        let ctx = W3CTraceContext::decode(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            Some("rw=frontend,congo=t61rcWkgMzE"),
        )
        .unwrap();

        assert_eq!(
            ctx.span_context.trace_id,
            TraceId(0x0af7651916cd43dd8448eb211c80319c)
        );
        assert_eq!(ctx.span_context.span_id, SpanId(0xb7ad6b7169203331));
        assert!(ctx.span_context.sampled);
        assert_eq!(ctx.tracestate(), Some("rw=frontend,congo=t61rcWkgMzE"));
    }

    #[test]
    fn w3c_trace_context_decode_without_tracestate() {
        let ctx = W3CTraceContext::decode(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            None,
        )
        .unwrap();

        assert_eq!(ctx.tracestate(), None);
    }

    #[test]
    fn w3c_trace_context_decode_empty_tracestate() {
        let ctx = W3CTraceContext::decode(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            Some(""),
        )
        .unwrap();

        assert_eq!(ctx.tracestate(), None);
    }

    #[test]
    fn w3c_trace_context_decode_invalid_traceparent() {
        assert!(W3CTraceContext::decode("invalid", Some("rw=frontend")).is_none());
    }

    #[test]
    fn w3c_trace_context_encode_roundtrip() {
        let original = W3CTraceContext::decode(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            Some("rw=frontend"),
        )
        .unwrap();

        let traceparent = original.encode_traceparent();
        let tracestate = original.encode_tracestate();

        let decoded = W3CTraceContext::decode(&traceparent, tracestate).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn w3c_trace_context_encode_roundtrip_no_tracestate() {
        let original = W3CTraceContext::decode(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00",
            None,
        )
        .unwrap();

        let traceparent = original.encode_traceparent();
        let tracestate = original.encode_tracestate();

        let decoded = W3CTraceContext::decode(&traceparent, tracestate).unwrap();

        assert_eq!(original, decoded);
        assert!(!decoded.span_context.sampled);
    }

    #[test]
    fn w3c_trace_context_with_tracestate() {
        let ctx = W3CTraceContext::new(SpanContext::new(TraceId(1), SpanId(2)));
        assert_eq!(ctx.tracestate(), None);

        let ctx = ctx.with_tracestate("rw=frontend");
        assert_eq!(ctx.tracestate(), Some("rw=frontend"));

        // Replace existing.
        let ctx = ctx.with_tracestate("rw=backend");
        assert_eq!(ctx.tracestate(), Some("rw=backend"));

        // Empty string normalizes to None.
        let ctx = ctx.with_tracestate("");
        assert_eq!(ctx.tracestate(), None);
    }

    #[test]
    fn w3c_trace_context_encode_headers() {
        let ctx = W3CTraceContext::new(SpanContext::new(TraceId(1), SpanId(2)))
            .with_tracestate("rw=frontend");

        let headers = ctx.encode_headers();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, "traceparent");
        assert_eq!(
            headers[0].1,
            "00-00000000000000000000000000000001-0000000000000002-01"
        );
        assert_eq!(headers[1].0, "tracestate");
        assert_eq!(headers[1].1, "rw=frontend");
    }

    #[test]
    fn w3c_trace_context_encode_headers_no_tracestate() {
        let ctx = W3CTraceContext::new(SpanContext::new(TraceId(1), SpanId(2)));
        let headers = ctx.encode_headers();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "traceparent");
    }

    #[test]
    fn w3c_trace_context_decode_headers() {
        let headers = vec![
            (
                "traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            ),
            ("tracestate", "rw=frontend,congo=t61rcWkgMzE"),
        ];

        let ctx = W3CTraceContext::decode_headers(headers).unwrap();
        assert_eq!(
            ctx.span_context.trace_id,
            TraceId(0x0af7651916cd43dd8448eb211c80319c)
        );
        assert_eq!(ctx.tracestate(), Some("rw=frontend,congo=t61rcWkgMzE"));
    }

    #[test]
    fn w3c_trace_context_decode_headers_case_insensitive() {
        let headers = vec![
            (
                "Traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            ),
            ("TraceState", "rw=frontend"),
        ];

        let ctx = W3CTraceContext::decode_headers(headers).unwrap();
        assert_eq!(ctx.tracestate(), Some("rw=frontend"));
    }

    #[test]
    fn w3c_trace_context_decode_headers_no_traceparent() {
        let headers = vec![("tracestate", "rw=frontend")];
        assert!(W3CTraceContext::decode_headers(headers).is_none());
    }

    #[test]
    fn w3c_trace_context_decode_headers_repeated_tracestate() {
        // Per W3C spec, multiple tracestate headers should be joined with commas.
        let headers = vec![
            (
                "traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            ),
            ("tracestate", "rw=frontend"),
            ("tracestate", "congo=t61rcWkgMzE"),
        ];

        let ctx = W3CTraceContext::decode_headers(headers).unwrap();
        assert_eq!(ctx.tracestate(), Some("rw=frontend,congo=t61rcWkgMzE"));
    }

    #[test]
    fn w3c_trace_context_decode_headers_repeated_empty_mixed() {
        let headers = vec![
            (
                "traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            ),
            ("tracestate", ""),
            ("tracestate", "rw=frontend"),
            ("tracestate", "  "),
        ];

        let ctx = W3CTraceContext::decode_headers(headers).unwrap();
        assert_eq!(ctx.tracestate(), Some("rw=frontend"));
    }

    #[test]
    fn w3c_trace_context_decode_headers_all_empty_tracestate() {
        let headers = vec![
            (
                "traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            ),
            ("tracestate", ""),
            ("tracestate", "  "),
        ];

        let ctx = W3CTraceContext::decode_headers(headers).unwrap();
        assert_eq!(ctx.tracestate(), None);
    }

    #[test]
    fn w3c_trace_context_header_roundtrip() {
        let original = W3CTraceContext::new(SpanContext::new(TraceId(42), SpanId(99)))
            .with_tracestate("vendor=value,other=data");

        let headers = original.encode_headers();
        let header_refs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let decoded = W3CTraceContext::decode_headers(header_refs).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn w3c_trace_context_from_span_context() {
        let span_ctx = SpanContext::new(TraceId(1), SpanId(2));
        let w3c_ctx: W3CTraceContext = span_ctx.into();
        assert_eq!(w3c_ctx.span_context, span_ctx);
        assert_eq!(w3c_ctx.tracestate(), None);
    }
}
