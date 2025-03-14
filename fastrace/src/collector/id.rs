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
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
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

#[cfg(feature = "serde")]
impl serde::Serialize for TraceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:032x}", self.0))
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TraceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        u128::from_str_radix(&s, 16)
            .map(TraceId)
            .map_err(serde::de::Error::custom)
    }
}

/// An identifier for a span within a trace.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
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
    /// Create a non-zero `SpanId`
    pub(crate) fn next_id() -> SpanId {
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

#[cfg(feature = "serde")]
impl serde::Serialize for SpanId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}

#[cfg(feature = "serde")]
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
#[derive(Clone, Copy, Debug, Default)]
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
            span_id: SpanId::default(),
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
            let collect_token = inner.issue_collect_token().next()?;

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
            let collect_token = stack.current_collect_token()?[0];

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
}
