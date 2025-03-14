// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use crate::Span;
use crate::local::LocalSpan;

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
    #[deprecated(since = "0.7.7", note = "use `Span::add_event` instead")]
    pub fn add_to_parent<I, F>(name: impl Into<Cow<'static, str>>, parent: &Span, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        Span::add_event(parent, name, properties);
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
    #[deprecated(since = "0.7.7", note = "use `LocalSpan::add_event` instead")]
    pub fn add_to_local_parent<I, F>(name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        LocalSpan::add_event(name, properties);
    }
}
