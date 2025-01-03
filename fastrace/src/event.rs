// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use crate::Span;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::local::raw_span::RawKind;

/// An event that represents a single point in time during the execution of a span.
pub struct Event {
    _private: (),
}

impl Event {
    /// Adds an event to the parent span with the given name and properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    ///
    /// Event::add_to_parent("event in root", &root, || [("key".into(), "value".into())]);
    /// ```
    pub fn add_to_parent<I, F>(name: impl Into<Cow<'static, str>>, parent: &Span, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        {
            let mut span = Span::enter_with_parent(name, parent).with_properties(properties);
            if let Some(mut inner) = span.inner.take() {
                inner.raw_span.raw_kind = RawKind::Event;
                inner.submit_spans();
            }
        }
    }

    /// Adds an event to the current local parent span with the given name and properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// let _guard = root.set_local_parent();
    ///
    /// Event::add_to_local_parent("event in root", || [("key".into(), "value".into())]);
    /// ```
    pub fn add_to_local_parent<I, F>(name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(|stack| stack.borrow_mut().add_event(name, properties))
                .ok();
        }
    }
}
