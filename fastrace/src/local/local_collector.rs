// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use fastant::Instant;

use crate::local::local_span_stack::LocalSpanStack;
use crate::local::local_span_stack::SpanLineHandle;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::prelude::SpanContext;
use crate::prelude::SpanRecord;
use crate::util::CollectToken;
use crate::util::RawSpans;

/// A collector to collect [`LocalSpan`].
///
/// `LocalCollector` allows to collect `LocalSpan` manually without a local parent. The collected
/// `LocalSpan` can later be attached to a parent.
///
/// Generally, [`Span`] and `LocalSpan` are sufficient. However, use `LocalCollector` when the span
/// might initiate before its parent span. This is particularly useful for tracing prior tasks that
/// may be obstructing the current request.
///
/// # Examples
///
/// ```
/// use fastrace::local::LocalCollector;
/// use fastrace::prelude::*;
///
/// // Collect local spans manually without a parent
/// let collector = LocalCollector::start();
/// let span = LocalSpan::enter_with_local_parent("a child span");
/// drop(span);
/// let local_spans = collector.collect();
///
/// // Attach the local spans to a parent
/// let root = Span::root("root", SpanContext::random());
/// root.push_child_spans(local_spans);
/// ```
///
/// [`Span`]: crate::Span
/// [`LocalSpan`]: crate::local::LocalSpan
#[must_use]
#[derive(Default)]
pub struct LocalCollector {
    #[cfg(feature = "enable")]
    inner: Option<LocalCollectorInner>,
}

struct LocalCollectorInner {
    stack: Rc<RefCell<LocalSpanStack>>,
    span_line_handle: SpanLineHandle,
}

/// A collection of [`LocalSpan`] instances.
///
/// This struct is typically used to group together multiple `LocalSpan` instances that have been
/// collected from a [`LocalCollector`]. These spans can then be associated with a parent span using
/// the [`Span::push_child_spans()`] method on the parent span.
///
/// Internally, it is implemented as an `Arc<[LocalSpan]>`, which allows it to be cloned and shared
/// across threads at a low cost.
///
/// # Examples
///
/// ```
/// use fastrace::local::LocalCollector;
/// use fastrace::local::LocalSpans;
/// use fastrace::prelude::*;
///
/// // Collect local spans manually without a parent
/// let collector = LocalCollector::start();
/// let span = LocalSpan::enter_with_local_parent("a child span");
/// drop(span);
///
/// // Collect local spans into a LocalSpans instance
/// let local_spans: LocalSpans = collector.collect();
///
/// // Attach the local spans to a parent
/// let root = Span::root("root", SpanContext::random());
/// root.push_child_spans(local_spans);
/// ```
///
/// [`Span::push_child_spans()`]: crate::Span::push_child_spans()
/// [`LocalSpan`]: crate::local::LocalSpan
/// [`LocalCollector`]: crate::local::LocalCollector
#[derive(Debug, Clone)]
pub struct LocalSpans {
    #[cfg(feature = "enable")]
    pub(crate) inner: Arc<LocalSpansInner>,
}

#[derive(Debug)]
pub struct LocalSpansInner {
    pub spans: RawSpans,
    pub end_time: Instant,
}

impl LocalCollector {
    pub fn start() -> Self {
        #[cfg(not(feature = "enable"))]
        {
            LocalCollector::default()
        }

        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(|stack| Self::new(None, stack.clone()))
                .unwrap_or_default()
        }
    }

    pub fn collect(self) -> LocalSpans {
        #[cfg(not(feature = "enable"))]
        {
            LocalSpans {}
        }

        #[cfg(feature = "enable")]
        {
            LocalSpans {
                inner: Arc::new(self.collect_spans_and_token().0),
            }
        }
    }
}

#[cfg(feature = "enable")]
impl LocalCollector {
    pub(crate) fn new(
        collect_token: Option<CollectToken>,
        stack: Rc<RefCell<LocalSpanStack>>,
    ) -> Self {
        let span_line_epoch = {
            let stack = &mut (*stack).borrow_mut();
            stack.register_span_line(collect_token)
        };

        Self {
            inner: span_line_epoch.map(move |span_line_handle| LocalCollectorInner {
                stack,
                span_line_handle,
            }),
        }
    }

    pub(crate) fn collect_spans_and_token(mut self) -> (LocalSpansInner, Option<CollectToken>) {
        let (spans, collect_token) = self
            .inner
            .take()
            .and_then(
                |LocalCollectorInner {
                     stack,
                     span_line_handle,
                 }| {
                    let s = &mut (*stack).borrow_mut();
                    s.unregister_and_collect(span_line_handle)
                },
            )
            .unwrap_or_default();

        (
            LocalSpansInner {
                spans,
                end_time: Instant::now(),
            },
            collect_token,
        )
    }
}

impl Drop for LocalCollector {
    fn drop(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(LocalCollectorInner {
            stack,
            span_line_handle,
        }) = self.inner.take()
        {
            let s = &mut (*stack).borrow_mut();
            let _ = s.unregister_and_collect(span_line_handle);
        }
    }
}

impl LocalSpans {
    /// Converts the `LocalSpans` to `SpanRecord`s.
    ///
    /// The converted spans will appear as if they were collected within the given parent context.
    /// The parent of the top local span is set to the given parent.
    ///
    /// This function is particularly useful when you want to manually collect the span records
    /// without involving the global collector. This function does not require that the global
    /// collector is set up by [`set_reporter()`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use fastrace::local::LocalCollector;
    /// use fastrace::local::LocalSpans;
    /// use fastrace::prelude::*;
    ///
    /// // Collect local spans manually without a parent
    /// let collector = LocalCollector::start();
    /// let span = LocalSpan::enter_with_local_parent("a child span");
    /// drop(span);
    ///
    /// // Collect local spans into a LocalSpans instance
    /// let local_spans: LocalSpans = collector.collect();
    ///
    /// // Convert LocalSpans to SpanRecords with a given parent context
    /// let parent_context = SpanContext::random();
    /// let span_records = local_spans.to_span_records(parent_context);
    ///
    /// // Now you can manually handle the span records
    /// for record in span_records {
    ///     println!("{:?}", record);
    /// }
    /// ```
    ///
    ///  [`set_reporter()`]: crate::set_reporter
    pub fn to_span_records(&self, parent: SpanContext) -> Vec<SpanRecord> {
        #[cfg(not(feature = "enable"))]
        {
            vec![]
        }

        #[cfg(feature = "enable")]
        {
            self.inner.to_span_records(parent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::collector::SpanId;
    use crate::prelude::LocalSpan;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_raw_spans;
    use crate::util::tree::tree_str_from_span_records;

    #[test]
    fn local_collector_basic() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        let collector1 = LocalCollector::new(None, stack.clone());

        let span1 = stack.borrow_mut().enter_span("span1").unwrap();
        {
            let token2 = CollectTokenItem {
                trace_id: TraceId(1234),
                parent_id: SpanId::default(),
                collect_id: 42,
                is_root: false,
                is_sampled: true,
            };
            let collector2 = LocalCollector::new(Some(token2.into()), stack.clone());
            let span2 = stack.borrow_mut().enter_span("span2").unwrap();
            let span3 = stack.borrow_mut().enter_span("span3").unwrap();
            stack.borrow_mut().exit_span(span3);
            stack.borrow_mut().exit_span(span2);

            let (spans, token) = collector2.collect_spans_and_token();
            assert_eq!(token.unwrap().as_slice(), &[token2]);
            assert_eq!(
                tree_str_from_raw_spans(spans.spans),
                r"
span2 []
    span3 []
"
            );
        }
        stack.borrow_mut().exit_span(span1);
        let spans = collector1.collect();
        assert_eq!(
            tree_str_from_raw_spans(spans.inner.spans.iter().cloned().collect()),
            r"
span1 []
"
        );
    }

    #[test]
    fn drop_without_collect() {
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        let collector1 = LocalCollector::new(None, stack.clone());

        let span1 = stack.borrow_mut().enter_span("span1").unwrap();
        {
            let token2 = CollectTokenItem {
                trace_id: TraceId(1234),
                parent_id: SpanId::default(),
                collect_id: 42,
                is_root: false,
                is_sampled: true,
            };
            let collector2 = LocalCollector::new(Some(token2.into()), stack.clone());
            let span2 = stack.borrow_mut().enter_span("span2").unwrap();
            let span3 = stack.borrow_mut().enter_span("span3").unwrap();
            stack.borrow_mut().exit_span(span3);
            stack.borrow_mut().exit_span(span2);
            drop(collector2);
        }
        stack.borrow_mut().exit_span(span1);
        let spans = collector1.collect();
        assert_eq!(
            tree_str_from_raw_spans(spans.inner.spans.iter().cloned().collect()),
            r"
span1 []
"
        );
    }

    #[test]
    fn local_spans_to_span_record() {
        let collector = LocalCollector::start();
        let span1 = LocalSpan::enter_with_local_parent("span1").with_property(|| ("k1", "v1"));
        let span2 = LocalSpan::enter_with_local_parent("span2").with_property(|| ("k2", "v2"));
        drop(span2);
        drop(span1);

        let local_spans: LocalSpans = collector.collect();

        let parent_context = SpanContext::random();
        let span_records = local_spans.to_span_records(parent_context);

        assert_eq!(
            tree_str_from_span_records(span_records),
            r#"
span1 [("k1", "v1")]
    span2 [("k2", "v2")]
"#
        );
    }
}
