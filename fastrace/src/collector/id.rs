// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use serde::ser::SerializeStruct;

use crate::Span;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;

thread_local! {
    static LOCAL_ID_GENERATOR: Cell<(u32, u32)> = Cell::new((rand::random(), 0))
}

/// Error returned when a trace id is malformed or all zeroes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidTraceId;

impl fmt::Display for InvalidTraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid trace id")
    }
}

impl std::error::Error for InvalidTraceId {}

/// Error returned when a span id is malformed or all zeroes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSpanId;

impl fmt::Display for InvalidSpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid span id")
    }
}

impl std::error::Error for InvalidSpanId {}

fn is_all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

/// An identifier for a trace, which groups a set of related spans together.
///
/// A valid `TraceId` contains at least one non-zero byte.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraceId(u128);

impl TraceId {
    /// Create a random non-zero `TraceId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let trace_id = TraceId::random();
    /// ```
    pub fn random() -> Self {
        loop {
            let value = rand::random();
            if value != 0 {
                return Self(value);
            }
        }
    }

    /// Creates a `TraceId` from bytes.
    ///
    /// Returns `None` if all bytes are zero.
    pub fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
        let value = u128::from_be_bytes(bytes);
        if value != 0 { Some(Self(value)) } else { None }
    }

    /// Creates a `TraceId` from a hexadecimal string.
    ///
    /// The input may contain fewer than 32 hexadecimal characters. Short inputs
    /// are interpreted as if they were left-padded with zeroes. Returns an error
    /// if the input is empty, longer than 32 hexadecimal characters, contains
    /// non-hexadecimal characters, or represents all zeroes.
    pub fn from_hex(hex: &str) -> Result<Self, InvalidTraceId> {
        if hex.is_empty() || hex.len() > 32 {
            return Err(InvalidTraceId);
        }

        match u128::from_str_radix(hex, 16) {
            Ok(value) if value != 0 => Ok(Self(value)),
            _ => Err(InvalidTraceId),
        }
    }

    /// Returns this trace id as bytes.
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraceId({self})")
    }
}

impl FromStr for TraceId {
    type Err = InvalidTraceId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl serde::Serialize for TraceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for TraceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        TraceId::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// An identifier for a span within a trace.
///
/// A valid `SpanId` is exactly 8 bytes and contains at least one non-zero byte.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpanId([u8; 8]);

impl SpanId {
    /// Create a random non-zero `SpanId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span_id = SpanId::random();
    /// ```
    pub fn random() -> Self {
        loop {
            let bytes = rand::random::<u64>().to_be_bytes();
            if let Some(span_id) = Self::from_bytes(bytes) {
                return span_id;
            }
        }
    }

    /// Creates a `SpanId` from bytes.
    ///
    /// Returns `None` if all bytes are zero.
    pub fn from_bytes(bytes: [u8; 8]) -> Option<Self> {
        (!is_all_zero(&bytes)).then_some(Self(bytes))
    }

    /// Creates a `SpanId` from a hexadecimal string.
    ///
    /// The input may contain fewer than 16 hexadecimal characters. Short inputs
    /// are interpreted as if they were left-padded with zeroes. Returns an error
    /// if the input is empty, longer than 16 hexadecimal characters, contains
    /// non-hexadecimal characters, or represents all zeroes.
    pub fn from_hex(hex: &str) -> Result<Self, InvalidSpanId> {
        if hex.is_empty() || hex.len() > 16 {
            return Err(InvalidSpanId);
        }

        match u64::from_str_radix(hex, 16) {
            Ok(value) if value != 0 => Ok(Self(value.to_be_bytes())),
            _ => Err(InvalidSpanId),
        }
    }

    /// Returns this span id as bytes.
    pub const fn to_bytes(self) -> [u8; 8] {
        self.0
    }

    #[inline]
    #[doc(hidden)]
    /// Create a non-zero `SpanId`.
    pub fn next_id() -> SpanId {
        LOCAL_ID_GENERATOR
            .try_with(|g| {
                let (prefix, mut suffix) = g.get();

                suffix = suffix.wrapping_add(1);

                g.set((prefix, suffix));

                let raw = ((prefix as u64) << 32) | (suffix as u64);
                SpanId::from_bytes(raw.to_be_bytes()).unwrap_or_else(SpanId::random)
            })
            .unwrap_or_else(|_| SpanId::random())
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", u64::from_be_bytes(self.0))
    }
}

impl FromStr for SpanId {
    type Err = InvalidSpanId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl serde::Serialize for SpanId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for SpanId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        SpanId::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Flags carried by a W3C `traceparent` header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TraceFlags(u8);

impl TraceFlags {
    /// Trace flags with no bits set.
    pub const NOT_SAMPLED: TraceFlags = TraceFlags(0x00);

    /// Trace flags with the W3C sampled bit set.
    pub const SAMPLED: TraceFlags = TraceFlags(0x01);

    /// Trace flags with the W3C random-trace-id bit set.
    pub const RANDOM_TRACE_ID: TraceFlags = TraceFlags(0x02);

    /// Constructs trace flags from raw W3C trace flag bits.
    pub const fn new(flags: u8) -> Self {
        TraceFlags(flags)
    }

    /// Returns the raw W3C trace flag bits.
    pub const fn to_u8(self) -> u8 {
        self.0
    }

    /// Returns `true` if the sampled bit is set.
    pub fn is_sampled(self) -> bool {
        self.0 & Self::SAMPLED.0 == Self::SAMPLED.0
    }

    /// Returns a copy of these flags with the sampled bit set or cleared.
    pub fn with_sampled(self, sampled: bool) -> Self {
        if sampled {
            Self(self.0 | Self::SAMPLED.0)
        } else {
            Self(self.0 & !Self::SAMPLED.0)
        }
    }

    /// Returns `true` if the random-trace-id bit is set.
    pub fn is_random_trace_id(self) -> bool {
        self.0 & Self::RANDOM_TRACE_ID.0 == Self::RANDOM_TRACE_ID.0
    }

    /// Returns a copy of these flags with the random-trace-id bit set or cleared.
    pub fn with_random_trace_id(self, random_trace_id: bool) -> Self {
        if random_trace_id {
            Self(self.0 | Self::RANDOM_TRACE_ID.0)
        } else {
            Self(self.0 & !Self::RANDOM_TRACE_ID.0)
        }
    }
}

impl fmt::LowerHex for TraceFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

/// A cheap pass-through representation of a W3C `tracestate` header.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TraceState(Option<Arc<str>>);

impl TraceState {
    /// An empty tracestate.
    pub const EMPTY: TraceState = TraceState(None);

    /// Creates an empty tracestate.
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    /// Creates a tracestate from a header value.
    ///
    /// Empty strings are normalized to `None`.
    pub fn from_header_value(header: impl Into<Arc<str>>) -> Self {
        let header = header.into();
        if header.is_empty() {
            Self::EMPTY
        } else {
            Self(Some(header))
        }
    }

    /// Returns the header value if this tracestate is present.
    pub fn as_header_value(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// Returns `true` when no tracestate is present.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
}

/// A cheap parent/link context for span creation and reporting.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpanContext {
    pub trace_id: TraceId,
    pub span_id: Option<SpanId>,
    pub trace_flags: TraceFlags,
    pub trace_state: TraceState,
}

impl SpanContext {
    /// W3C `traceparent` header name.
    pub const TRACEPARENT_HEADER_NAME: &'static str = "traceparent";

    /// W3C `tracestate` header name.
    pub const TRACESTATE_HEADER_NAME: &'static str = "tracestate";

    /// Creates a `SpanContext` from a trace id and a valid parent span id.
    ///
    /// Use [`SpanContext::root`] or [`SpanContext::random`] when starting a
    /// trace without a remote parent.
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        Self {
            trace_id,
            span_id: Some(span_id),
            trace_flags: TraceFlags::SAMPLED,
            trace_state: TraceState::EMPTY,
        }
    }

    /// Creates a root `SpanContext` with no remote parent span.
    pub fn root(trace_id: TraceId) -> Self {
        Self {
            trace_id,
            span_id: None,
            trace_flags: TraceFlags::SAMPLED,
            trace_state: TraceState::EMPTY,
        }
    }

    /// Create a new root `SpanContext` with a random trace id and no remote parent.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// ```
    pub fn random() -> Self {
        Self::root(TraceId::random())
            .with_trace_flags(TraceFlags::SAMPLED.with_random_trace_id(true))
    }

    /// Returns the trace id.
    pub fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    /// Returns the parent or linked span id, if present.
    pub fn span_id(&self) -> Option<SpanId> {
        self.span_id
    }

    /// Returns the trace flags.
    pub fn trace_flags(&self) -> TraceFlags {
        self.trace_flags
    }

    /// Returns the tracestate header value, if present.
    pub fn trace_state(&self) -> Option<&str> {
        self.trace_state.as_header_value()
    }

    /// Returns `true` when the sampled trace flag is set.
    pub fn is_sampled(&self) -> bool {
        self.trace_flags.is_sampled()
    }

    /// Sets the sampled flag of the `SpanContext`.
    ///
    /// When the sampled flag is `false`, the spans will not be collected, but the parent-child
    /// relationship will still be maintained and the `SpanContext` can still be propagated.
    pub fn sampled(mut self, sampled: bool) -> Self {
        self.trace_flags = self.trace_flags.with_sampled(sampled);
        self
    }

    /// Sets the trace flags of the `SpanContext`.
    pub fn with_trace_flags(mut self, trace_flags: TraceFlags) -> Self {
        self.trace_flags = trace_flags;
        self
    }

    /// Sets the tracestate of the `SpanContext`.
    ///
    /// Empty strings are normalized to no tracestate.
    pub fn with_trace_state(mut self, trace_state: impl Into<Arc<str>>) -> Self {
        self.trace_state = TraceState::from_header_value(trace_state);
        self
    }

    /// Encodes this span context into a W3C `traceparent` header value.
    ///
    /// Returns `None` if this context represents a root with no remote parent span id.
    pub fn encode_traceparent(&self) -> Option<String> {
        let span_id = self.span_id?;
        Some(format!(
            "00-{}-{}-{:02x}",
            self.trace_id, span_id, self.trace_flags
        ))
    }

    /// Decodes a span context from a W3C `traceparent` header value.
    pub fn decode_traceparent(traceparent: &str) -> Option<Self> {
        let mut parts = traceparent.split('-');

        match (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            (Some("00"), Some(trace_id), Some(span_id), Some(trace_flags), None) => {
                if trace_id.len() != 32 || span_id.len() != 16 || trace_flags.len() != 2 {
                    return None;
                }
                Some(Self {
                    trace_id: TraceId::from_hex(trace_id).ok()?,
                    span_id: Some(SpanId::from_hex(span_id).ok()?),
                    trace_flags: TraceFlags::new(u8::from_str_radix(trace_flags, 16).ok()?),
                    trace_state: TraceState::EMPTY,
                })
            }
            _ => None,
        }
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
                trace_flags: collect_token.trace_flags,
                trace_state: collect_token.trace_state,
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
                trace_flags: collect_token.trace_flags,
                trace_state: collect_token.trace_state,
            })
        }
    }
}

impl serde::Serialize for SpanContext {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut fields = serializer.serialize_struct("SpanContext", 4)?;
        fields.serialize_field("trace_id", &self.trace_id)?;
        fields.serialize_field("span_id", &self.span_id)?;
        fields.serialize_field("trace_flags", &self.trace_flags.to_u8())?;
        fields.serialize_field("trace_state", &self.trace_state.as_header_value())?;
        fields.end()
    }
}

impl<'de> serde::Deserialize<'de> for SpanContext {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Fields {
            trace_id: TraceId,
            span_id: Option<SpanId>,
            trace_flags: u8,
            trace_state: Option<String>,
        }

        let fields = Fields::deserialize(deserializer)?;

        Ok(SpanContext {
            trace_id: fields.trace_id,
            span_id: fields.span_id,
            trace_flags: TraceFlags::new(fields.trace_flags),
            trace_state: fields
                .trace_state
                .map(TraceState::from_header_value)
                .unwrap_or(TraceState::EMPTY),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn trace_id(value: u128) -> TraceId {
        TraceId::from_bytes(value.to_be_bytes()).unwrap()
    }

    fn span_id(value: u64) -> SpanId {
        SpanId::from_bytes(value.to_be_bytes()).unwrap()
    }

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
    fn zero_ids_are_rejected() {
        assert!(TraceId::from_bytes([0; 16]).is_none());
        assert!(SpanId::from_bytes([0; 8]).is_none());
        assert!(TraceId::from_hex("").is_err());
        assert!(SpanId::from_hex("").is_err());
        assert!(TraceId::from_hex("0").is_err());
        assert!(SpanId::from_hex("0").is_err());
        assert!(TraceId::from_hex("00000000000000000000000000000000").is_err());
        assert!(SpanId::from_hex("0000000000000000").is_err());
        assert!(
            "00000000000000000000000000000000"
                .parse::<TraceId>()
                .is_err()
        );
        assert!("0000000000000000".parse::<SpanId>().is_err());
    }

    #[test]
    fn short_hex_ids_are_left_padded() {
        let trace = TraceId::from_hex("abc").unwrap();
        assert_eq!(trace.to_string(), "00000000000000000000000000000abc");
        assert_eq!(TraceId::from_bytes(0x0abcu128.to_be_bytes()), Some(trace));

        let span = SpanId::from_hex("abc").unwrap();
        assert_eq!(span.to_string(), "0000000000000abc");
        assert_eq!(SpanId::from_bytes(0x0abcu64.to_be_bytes()), Some(span));

        assert!(TraceId::from_hex("000000000000000000000000000000001").is_err());
        assert!(SpanId::from_hex("00000000000000001").is_err());
    }

    #[test]
    fn valid_ids_roundtrip() {
        let trace = TraceId::from_hex("0af7651916cd43dd8448eb211c80319c").unwrap();
        assert_eq!(trace.to_string(), "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(TraceId::from_bytes(trace.to_bytes()), Some(trace));
        assert_eq!(
            TraceId::from_bytes(0x0af7651916cd43dd8448eb211c80319cu128.to_be_bytes()),
            Some(trace)
        );

        let span = SpanId::from_hex("b7ad6b7169203331").unwrap();
        assert_eq!(span.to_string(), "b7ad6b7169203331");
        assert_eq!(SpanId::from_bytes(span.to_bytes()), Some(span));
        assert_eq!(
            SpanId::from_bytes(0xb7ad6b7169203331u64.to_be_bytes()),
            Some(span)
        );
    }

    #[test]
    fn span_context_decode_traceparent() {
        let ctx = SpanContext::decode_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .unwrap()
        .with_trace_state("rw=frontend,congo=t61rcWkgMzE");

        assert_eq!(ctx.trace_id, trace_id(0x0af7651916cd43dd8448eb211c80319c));
        assert_eq!(ctx.span_id, Some(span_id(0xb7ad6b7169203331)));
        assert!(ctx.is_sampled());
        assert_eq!(ctx.trace_state(), Some("rw=frontend,congo=t61rcWkgMzE"));
    }

    #[test]
    fn span_context_decode_without_tracestate() {
        let ctx = SpanContext::decode_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .unwrap();

        assert_eq!(ctx.trace_state(), None);
    }

    #[test]
    fn span_context_empty_tracestate() {
        let ctx = SpanContext::new(trace_id(1), span_id(2)).with_trace_state("");

        assert_eq!(ctx.trace_state(), None);
    }

    #[test]
    fn span_context_decode_invalid_traceparent() {
        assert!(SpanContext::decode_traceparent("invalid").is_none());
        assert!(SpanContext::decode_traceparent("00-abc-b7ad6b7169203331-01").is_none());
        assert!(
            SpanContext::decode_traceparent("00-0af7651916cd43dd8448eb211c80319c-abc-01").is_none()
        );
        assert!(
            SpanContext::decode_traceparent(
                "00-00000000000000000000000000000000-b7ad6b7169203331-01",
            )
            .is_none()
        );
        assert!(
            SpanContext::decode_traceparent(
                "00-0af7651916cd43dd8448eb211c80319c-0000000000000000-01",
            )
            .is_none()
        );
    }

    #[test]
    fn span_context_encode_roundtrip() {
        let original = SpanContext::decode_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-03",
        )
        .unwrap()
        .with_trace_state("rw=frontend");

        assert!(original.trace_flags().is_sampled());
        assert!(original.trace_flags().is_random_trace_id());

        let traceparent = original.encode_traceparent().unwrap();
        let decoded = SpanContext::decode_traceparent(&traceparent)
            .unwrap()
            .with_trace_state(original.trace_state().unwrap());

        assert_eq!(original, decoded);
    }

    #[test]
    fn trace_flags_sampled_roundtrip() {
        let sampled = SpanContext::decode_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .unwrap();
        assert!(sampled.trace_flags().is_sampled());
        assert_eq!(
            sampled.encode_traceparent().unwrap(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );

        let not_sampled = SpanContext::decode_traceparent(
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00",
        )
        .unwrap();
        assert!(!not_sampled.trace_flags().is_sampled());
        assert_eq!(
            not_sampled.encode_traceparent().unwrap(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00"
        );
    }

    #[test]
    fn span_context_with_tracestate() {
        let ctx = SpanContext::new(trace_id(1), span_id(2));
        assert_eq!(ctx.trace_state(), None);

        let ctx = ctx.with_trace_state("rw=frontend");
        assert_eq!(ctx.trace_state(), Some("rw=frontend"));

        let ctx = ctx.with_trace_state("rw=backend");
        assert_eq!(ctx.trace_state(), Some("rw=backend"));

        let ctx = ctx.with_trace_state("");
        assert_eq!(ctx.trace_state(), None);
    }

    #[test]
    fn span_context_header_name_constants() {
        assert_eq!(SpanContext::TRACEPARENT_HEADER_NAME, "traceparent");
        assert_eq!(SpanContext::TRACESTATE_HEADER_NAME, "tracestate");
    }

    #[test]
    fn root_span_context_cannot_encode_traceparent() {
        let root_ctx = SpanContext::root(trace_id(1));
        assert!(root_ctx.encode_traceparent().is_none());
    }

    #[test]
    fn span_context_serde_preserves_trace_state_and_root() {
        let ctx = SpanContext::root(trace_id(1))
            .with_trace_flags(TraceFlags::new(0x03))
            .with_trace_state("vendor=value");

        let json = serde_json::to_string(&ctx).unwrap();
        let decoded: SpanContext = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, ctx);
        assert_eq!(decoded.span_id(), None);
        assert_eq!(decoded.trace_flags().to_u8(), 0x03);
        assert_eq!(decoded.trace_state(), Some("vendor=value"));
    }

    #[test]
    fn span_context_serde_rejects_zero_ids() {
        assert!(
            serde_json::from_str::<SpanContext>(
                r#"{"trace_id":"0","span_id":null,"trace_flags":1,"trace_state":null}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<SpanContext>(
                r#"{"trace_id":"1","span_id":"0","trace_flags":1,"trace_state":null}"#
            )
            .is_err()
        );
    }
}
