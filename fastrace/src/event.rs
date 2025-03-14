// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use crate::Span;
use crate::local::LocalSpan;
use crate::util::Properties;

/// An event that represents a single point in time during the execution of a span.
pub struct Event {
    pub(crate) name: Cow<'static, str>,
    pub(crate) properties: Option<Properties>,
}

impl Event {
    /// Create a new event with the given name.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// LocalSpan::add_event(Event::new("event"));
    /// ```
    #[inline]
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Event {
            name: name.into(),
            properties: None,
        }
    }

    /// Add a single property to the `Event` and return the modified `Event`.
    ///
    /// A property is an arbitrary key-value pair associated with an event.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// LocalSpan::add_event(Event::new("event").with_property(|| ("key", "value")));
    /// ```
    #[inline]
    pub fn with_property<K, V, F>(self, property: F) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        F: FnOnce() -> (K, V),
    {
        self.with_properties(|| [property()])
    }

    /// Add multiple properties to the `Event` and return the modified `Event`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// LocalSpan::add_event(Event::new("event").with_properties(|| [("key1", "value")]));
    /// ```
    #[inline]
    pub fn with_properties<K, V, I, F>(mut self, properties: F) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        {
            self.properties
                .get_or_insert_with(Properties::default)
                .extend(properties().into_iter().map(|(k, v)| (k.into(), v.into())))
        }
        self
    }

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
    #[deprecated(since = "0.7.8", note = "use `Span::add_event` instead")]
    pub fn add_to_parent<I, F>(name: impl Into<Cow<'static, str>>, parent: &Span, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        let event = Event::new(name).with_properties(properties);
        parent.add_event(event);
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
    #[deprecated(since = "0.7.8", note = "use `LocalSpan::add_event` instead")]
    pub fn add_to_local_parent<I, F>(name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        let event = Event::new(name).with_properties(properties);
        LocalSpan::add_event(event);
    }
}
