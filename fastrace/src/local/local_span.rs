// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::local::local_span_line::LocalSpanHandle;
use crate::local::local_span_stack::LocalSpanStack;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;

/// An optimized [`Span`] for tracing operations within a single thread.
///
/// [`Span`]: crate::Span
#[must_use]
#[derive(Default)]
pub struct LocalSpan {
    #[cfg(feature = "enable")]
    inner: Option<LocalSpanInner>,
}

struct LocalSpanInner {
    stack: Rc<RefCell<LocalSpanStack>>,
    span_handle: LocalSpanHandle,
}

impl LocalSpan {
    /// Create a new child span associated with the current local span in the current thread, and
    /// then it will become the new local parent.
    ///
    /// If no local span is active, this function is no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// let _g = root.set_local_parent();
    ///
    /// let child = LocalSpan::enter_with_local_parent("child");
    /// ```
    #[inline]
    pub fn enter_with_local_parent(name: impl Into<Cow<'static, str>>) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            LocalSpan::default()
        }

        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(|stack| Self::enter_with_stack(name, stack.clone()))
                .unwrap_or_default()
        }
    }

    /// Add a single property to the current local parent. If the local parent is a [`Span`],
    /// the property will be added to the `Span`.
    ///
    /// A property is an arbitrary key-value pair associated with a span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// LocalSpan::add_property(|| ("key", "value"));
    /// ```
    ///
    /// [`Span`]: crate::Span
    #[inline]
    pub fn add_property<K, V, F>(property: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        F: FnOnce() -> (K, V),
    {
        Self::add_properties(|| [property()])
    }

    /// Add multiple properties to the current local parent. If the local parent is a [`Span`],
    /// the properties will be added to the `Span`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// LocalSpan::add_properties(|| [("key1", "value1"), ("key2", "value2")]);
    /// ```
    ///
    /// [`Span`]: crate::Span
    #[inline]
    pub fn add_properties<K, V, I, F>(properties: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(|s| {
                    let span_stack = &mut *s.borrow_mut();
                    span_stack.add_properties(properties);
                    Some(())
                })
                .ok();
        }
    }

    /// Add a single property to the `LocalSpan` and return the modified `LocalSpan`.
    ///
    /// A property is an arbitrary key-value pair associated with a span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span =
    ///     LocalSpan::enter_with_local_parent("a child span").with_property(|| ("key", "value"));
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

    /// Add multiple properties to the `LocalSpan` and return the modified `LocalSpan`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let span = LocalSpan::enter_with_local_parent("a child span")
    ///     .with_properties(|| [("key1", "value1"), ("key2", "value2")]);
    /// ```
    #[inline]
    pub fn with_properties<K, V, I, F>(self, properties: F) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        #[cfg(feature = "enable")]
        if let Some(LocalSpanInner { stack, span_handle }) = &self.inner {
            let span_stack = &mut *stack.borrow_mut();
            span_stack.with_properties(span_handle, properties);
        }

        self
    }
}

#[cfg(feature = "enable")]
impl LocalSpan {
    #[inline]
    pub(crate) fn enter_with_stack(
        name: impl Into<Cow<'static, str>>,
        stack: Rc<RefCell<LocalSpanStack>>,
    ) -> Self {
        let span_handle = {
            let mut stack = stack.borrow_mut();
            stack.enter_span(name)
        };

        let inner = span_handle.map(|span_handle| LocalSpanInner { stack, span_handle });

        Self { inner }
    }
}

impl Drop for LocalSpan {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(LocalSpanInner { stack, span_handle }) = self.inner.take() {
            let mut span_stack = stack.borrow_mut();
            span_stack.exit_span(span_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::collector::SpanId;
    use crate::local::LocalCollector;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_raw_spans;

    #[test]
    fn local_span_basic() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

        let token = CollectTokenItem {
            trace_id: TraceId(1234),
            parent_id: SpanId::default(),
            collect_id: 42,
            is_root: false,
            is_sampled: true,
        };
        let collector = LocalCollector::new(Some(token.into()), stack.clone());

        {
            let _g = LocalSpan::enter_with_stack("span1", stack.clone());
            {
                let _span =
                    LocalSpan::enter_with_stack("span2", stack).with_property(|| ("k1", "v1"));
            }
        }

        let (spans, collect_token) = collector.collect_spans_and_token();
        assert_eq!(collect_token.unwrap().as_slice(), &[token]);
        assert_eq!(
            tree_str_from_raw_spans(spans.spans),
            r#"
span1 []
    span2 [("k1", "v1")]
"#
        );
    }

    #[test]
    fn local_span_noop() {
        let _span1 = LocalSpan::enter_with_local_parent("span1").with_property(|| ("k1", "v1"));
    }

    #[test]
    #[should_panic]
    fn drop_out_of_order() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

        let token = CollectTokenItem {
            trace_id: TraceId(1234),
            parent_id: SpanId::default(),
            collect_id: 42,
            is_root: false,
            is_sampled: true,
        };
        let collector = LocalCollector::new(Some(token.into()), stack.clone());

        {
            let span1 = LocalSpan::enter_with_stack("span1", stack.clone());
            {
                let _span2 =
                    LocalSpan::enter_with_stack("span2", stack).with_property(|| ("k1", "v1"));

                drop(span1);
            }
        }

        let _ = collector.collect_spans_and_token();
    }
}
