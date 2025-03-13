// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use fastant::Instant;

use crate::Event;
use crate::collector::CollectTokenItem;
use crate::collector::GlobalCollect;
use crate::collector::SpanContext;
use crate::collector::SpanId;
use crate::collector::SpanSet;
use crate::collector::global_collector::NOT_SAMPLED_COLLECT_ID;
use crate::collector::global_collector::reporter_ready;
use crate::local::LocalCollector;
use crate::local::LocalSpans;
use crate::local::local_collector::LocalSpansInner;
use crate::local::local_span_stack::LOCAL_SPAN_STACK;
use crate::local::local_span_stack::LocalSpanStack;
use crate::local::raw_span::RawKind;
use crate::local::raw_span::RawSpan;
use crate::util::CollectToken;

/// A thread-safe span.
#[must_use]
#[derive(Default)]
pub struct Span {
    #[cfg(feature = "enable")]
    pub(crate) inner: Option<SpanInner>,
}

pub(crate) struct SpanInner {
    pub(crate) raw_span: RawSpan,
    collect_token: CollectToken,
    // If the span is not a root span, this field will be `None`.
    collect_id: Option<usize>,
    collect: GlobalCollect,
}

impl Span {
    /// Create a place-holder span that never starts recording.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let mut root = Span::noop();
    /// ```
    #[inline]
    pub fn noop() -> Self {
        Self {
            #[cfg(feature = "enable")]
            inner: None,
        }
    }

    /// Create a new trace and return its root span.
    ///
    /// Once dropped, the root span automatically submits all associated child spans to the
    /// reporter.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let mut root = Span::root("root", SpanContext::random());
    /// ```
    #[inline]
    pub fn root(name: impl Into<Cow<'static, str>>, parent: SpanContext) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            if !reporter_ready() {
                return Self::noop();
            }

            let collect_id = if parent.sampled {
                current_collect().start_collect()
            } else {
                NOT_SAMPLED_COLLECT_ID
            };

            let token = CollectTokenItem {
                trace_id: parent.trace_id,
                parent_id: parent.span_id,
                collect_id,
                is_root: true,
                is_sampled: parent.sampled,
            }
            .into();

            Self::new(token, name, Some(collect_id))
        }
    }

    /// Create a new child span associated with the specified parent span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    ///
    /// let child = Span::enter_with_parent("child", &root);
    #[inline]
    pub fn enter_with_parent(name: impl Into<Cow<'static, str>>, parent: &Span) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            match &parent.inner {
                Some(_inner) => Self::enter_with_parents(name, [parent]),
                None => Span::noop(),
            }
        }
    }

    /// Create a new child span associated with multiple parent spans.
    ///
    /// This function is particularly useful when a single operation amalgamates multiple requests.
    /// It enables the creation of a unique child span that is interconnected with all the parent
    /// spans related to the requests, thereby obviating the need to generate individual child
    /// spans for each parent span.
    ///
    /// The newly created child span, and its children, will have a replica for each trace of parent
    /// spans.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let parent1 = Span::root("parent1", SpanContext::random());
    /// let parent2 = Span::root("parent2", SpanContext::random());
    ///
    /// let child = Span::enter_with_parents("child", [&parent1, &parent2]);
    #[inline]
    pub fn enter_with_parents<'a>(
        name: impl Into<Cow<'static, str>>,
        parents: impl IntoIterator<Item = &'a Span>,
    ) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            let token = parents
                .into_iter()
                .filter_map(|span| span.inner.as_ref())
                .flat_map(|inner| inner.issue_collect_token())
                .collect();
            Self::new(token, name, None)
        }
    }

    /// Create a new child span associated with the current local span in the current thread.
    ///
    /// If no local span is active, this function returns a no-op span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// let _g = root.set_local_parent();
    ///
    /// let child = Span::enter_with_local_parent("child");
    /// ```
    #[inline]
    pub fn enter_with_local_parent(name: impl Into<Cow<'static, str>>) -> Self {
        #[cfg(not(feature = "enable"))]
        {
            Self::noop()
        }

        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(move |stack| Self::enter_with_stack(name, &mut (*stack).borrow_mut()))
                .unwrap_or_default()
        }
    }

    /// Adds an event to the span with the given name and properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    ///
    /// root.add_event("event in root", || [("key".into(), "value".into())])
    ///     .add_event("event2 in root", || [("key2".into(), "value2".into())]);
    /// ```
    #[inline]
    pub fn add_event<I, F>(&self, name: impl Into<Cow<'static, str>>, properties: F) -> &Self
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        Event::add_to_parent(name, self, properties);
        self
    }

    /// Sets the current `Span` as the local parent for the current thread.
    ///
    /// This method is used to establish a `Span` as the local parent within the current scope.
    ///
    /// A local parent is necessary for creating a [`LocalSpan`] using
    /// [`LocalSpan::enter_with_local_parent()`]. If no local parent is set,
    /// `enter_with_local_parent()` will not perform any action.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// let _guard = root.set_local_parent(); // root is now the local parent
    ///
    /// // Now we can create a LocalSpan with root as the local parent.
    /// let _span = LocalSpan::enter_with_local_parent("a child span");
    /// ```
    ///
    /// [`LocalSpan`]: crate::local::LocalSpan
    /// [`LocalSpan::enter_with_local_parent()`]: crate::local::LocalSpan::enter_with_local_parent
    pub fn set_local_parent(&self) -> LocalParentGuard {
        #[cfg(not(feature = "enable"))]
        {
            LocalParentGuard::noop()
        }

        #[cfg(feature = "enable")]
        {
            LOCAL_SPAN_STACK
                .try_with(|s| self.attach_into_stack(s))
                .unwrap_or_default()
        }
    }

    /// Add a single property to the `Span` and return the modified `Span`.
    ///
    /// A property is an arbitrary key-value pair associated with a span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random()).with_property(|| ("key", "value"));
    /// ```
    #[inline]
    pub fn with_property<K, V, F>(self, property: F) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        F: FnOnce() -> (K, V),
    {
        self.with_properties(move || [property()])
    }

    /// Add multiple properties to the `Span` and return the modified `Span`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("root", SpanContext::random())
    ///     .with_properties(|| [("key1", "value1"), ("key2", "value2")]);
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
        if let Some(inner) = self.inner.as_mut() {
            inner.add_properties(properties);
        }

        self
    }

    /// Attach a collection of [`LocalSpan`] instances as child spans to the current span.
    ///
    /// This method allows you to associate previously collected `LocalSpan` instances with the
    /// current span. This is particularly useful when the `LocalSpan` instances were initiated
    /// before their parent span, and were collected manually using a [`LocalCollector`].
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
    /// [`LocalSpan`]: crate::local::LocalSpan
    /// [`LocalSpans`]: crate::local::LocalSpans
    /// [`LocalCollector`]: crate::local::LocalCollector
    #[inline]
    pub fn push_child_spans(&self, local_spans: LocalSpans) {
        #[cfg(feature = "enable")]
        {
            if let Some(inner) = self.inner.as_ref() {
                inner.push_child_spans(local_spans.inner)
            }
        }
    }

    /// Returns the elapsed time since the span was created. If the `Span` is a noop span,
    /// this function will return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    /// use std::time::Duration;
    ///
    /// let mut root = Span::root("root", SpanContext::random());
    ///
    /// // ...
    ///
    /// if root
    ///     .elapsed()
    ///     .map(|elapsed| elapsed < Duration::from_secs(1))
    ///     .unwrap_or(false)
    /// {
    ///     root.cancel();
    /// }
    #[inline]
    pub fn elapsed(&self) -> Option<Duration> {
        #[cfg(feature = "enable")]
        if let Some(inner) = self.inner.as_ref() {
            return Some(inner.raw_span.begin_instant.elapsed());
        }

        None
    }

    /// Dismisses the trace, preventing the reporting of any span records associated with it.
    ///
    /// This is particularly useful when focusing on the tail latency of a program. For instant,
    /// you can dismiss all traces finishes within the 99th percentile.
    ///
    /// # Note
    ///
    /// This method only dismisses the entire trace when called on the root span.
    /// If called on a non-root span, it will only cancel the reporting of that specific span.
    ///
    /// # Examples
    ///
    /// ```
    /// use fastrace::prelude::*;
    ///
    /// let mut root = Span::root("root", SpanContext::random());
    ///
    /// root.cancel();
    /// ```
    #[inline]
    pub fn cancel(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(inner) = self.inner.take() {
            if let Some(collect_id) = inner.collect_id {
                inner.collect.drop_collect(collect_id);
            }
        }
    }
}

#[cfg(feature = "enable")]
impl Span {
    #[inline]
    fn new(
        collect_token: CollectToken,
        name: impl Into<Cow<'static, str>>,
        collect_id: Option<usize>,
    ) -> Self {
        let span_id = SpanId::next_id();
        let begin_instant = Instant::now();
        let raw_span = RawSpan::begin_with(
            span_id,
            SpanId::default(),
            begin_instant,
            name,
            RawKind::Span,
        );
        let collect = current_collect();

        Self {
            inner: Some(SpanInner {
                raw_span,
                collect_token,
                collect_id,
                collect,
            }),
        }
    }

    pub(crate) fn enter_with_stack(
        name: impl Into<Cow<'static, str>>,
        stack: &mut LocalSpanStack,
    ) -> Self {
        match stack.current_collect_token() {
            Some(token) => Span::new(token, name, None),
            None => Self::noop(),
        }
    }

    pub(crate) fn attach_into_stack(
        &self,
        stack: &Rc<RefCell<LocalSpanStack>>,
    ) -> LocalParentGuard {
        self.inner
            .as_ref()
            .map(move |inner| inner.capture_local_spans(stack.clone()))
            .unwrap_or_else(LocalParentGuard::noop)
    }
}

#[cfg(feature = "enable")]
impl SpanInner {
    #[inline]
    fn add_properties<K, V, I, F>(&mut self, properties: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        self.raw_span
            .properties
            .extend(properties().into_iter().map(|(k, v)| (k.into(), v.into())));
    }

    #[inline]
    fn capture_local_spans(&self, stack: Rc<RefCell<LocalSpanStack>>) -> LocalParentGuard {
        let token = self.issue_collect_token().collect();
        let collector = LocalCollector::new(Some(token), stack);

        LocalParentGuard::new(collector, self.collect.clone())
    }

    #[inline]
    fn push_child_spans(&self, local_spans: Arc<LocalSpansInner>) {
        if local_spans.spans.is_empty() {
            return;
        }

        self.collect.submit_spans(
            SpanSet::SharedLocalSpans(local_spans),
            self.issue_collect_token().collect(),
        );
    }

    #[inline]
    pub(crate) fn issue_collect_token(&self) -> impl Iterator<Item = CollectTokenItem> + '_ {
        self.collect_token
            .iter()
            .map(move |collect_item| CollectTokenItem {
                trace_id: collect_item.trace_id,
                parent_id: self.raw_span.id,
                collect_id: collect_item.collect_id,
                is_root: false,
                is_sampled: collect_item.is_sampled,
            })
    }

    #[inline]
    pub(crate) fn submit_spans(self) {
        self.collect
            .submit_spans(SpanSet::Span(self.raw_span), self.collect_token);
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(mut inner) = self.inner.take() {
            let collect_id = inner.collect_id.take();
            let collect = inner.collect.clone();

            let end_instant = Instant::now();
            inner.raw_span.end_with(end_instant);
            inner.submit_spans();

            if let Some(collect_id) = collect_id {
                collect.commit_collect(collect_id);
            }
        }
    }
}

/// A guard created by [`Span::set_local_parent()`].
#[derive(Default)]
pub struct LocalParentGuard {
    #[cfg(feature = "enable")]
    inner: Option<LocalParentGuardInner>,
}

struct LocalParentGuardInner {
    collector: LocalCollector,
    collect: GlobalCollect,
}

impl LocalParentGuard {
    pub(crate) fn noop() -> LocalParentGuard {
        LocalParentGuard {
            #[cfg(feature = "enable")]
            inner: None,
        }
    }

    pub(crate) fn new(collector: LocalCollector, collect: GlobalCollect) -> LocalParentGuard {
        LocalParentGuard {
            #[cfg(feature = "enable")]
            inner: Some(LocalParentGuardInner { collector, collect }),
        }
    }
}

impl Drop for LocalParentGuard {
    fn drop(&mut self) {
        #[cfg(feature = "enable")]
        if let Some(inner) = self.inner.take() {
            let (spans, token) = inner.collector.collect_spans_and_token();
            debug_assert!(token.is_some());
            if let Some(token) = token {
                inner
                    .collect
                    .submit_spans(SpanSet::LocalSpansInner(spans), token);
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static MOCK_COLLECT: RefCell<GlobalCollect> = RefCell::new(GlobalCollect::default());
}

#[cfg(test)]
fn current_collect() -> GlobalCollect {
    MOCK_COLLECT.with(|mock| mock.borrow().clone())
}

#[cfg(test)]
fn set_mock_collect(collect: GlobalCollect) {
    MOCK_COLLECT.with(|mock| *mock.borrow_mut() = collect);
}

#[cfg(not(test))]
fn current_collect() -> GlobalCollect {
    GlobalCollect
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use mockall::Sequence;
    use mockall::predicate;
    use rand::rng;
    use rand::seq::SliceRandom;

    use super::*;
    use crate::collector::ConsoleReporter;
    use crate::collector::MockGlobalCollect;
    use crate::local::LocalSpan;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_span_sets;

    #[test]
    fn noop_basic() {
        let span = Span::noop();
        let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
        assert!(span.attach_into_stack(&stack).inner.is_none());
        assert!(stack.borrow_mut().enter_span("span1").is_none());
    }

    #[test]
    fn root_collect() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_submit_spans()
            .times(1)
            .in_sequence(&mut seq)
            .with(
                predicate::always(),
                predicate::eq::<CollectToken>(
                    CollectTokenItem {
                        trace_id: TraceId(12),
                        parent_id: SpanId::default(),
                        collect_id: 42,
                        is_root: true,
                        is_sampled: true,
                    }
                    .into(),
                ),
            )
            .return_const(());
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        let _root = Span::root("root", SpanContext::new(TraceId(12), SpanId::default()));
    }

    #[test]
    fn root_cancel() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_drop_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_commit_collect().times(0);
        mock.expect_submit_spans().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        let mut root = Span::root("root", SpanContext::random());
        root.cancel();
    }

    #[test]
    fn span_with_parent() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let routine = || {
            let parent_ctx = SpanContext::random();
            let root = Span::root("root", parent_ctx);
            let child1 =
                Span::enter_with_parent("child1", &root).with_properties(|| [("k1", "v1")]);
            let grandchild = Span::enter_with_parent("grandchild", &child1);
            let child2 = Span::enter_with_parent("child2", &root);

            crossbeam::scope(move |scope| {
                let mut rng = rng();
                let mut spans = [child1, grandchild, child2];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_submit_spans()
            .times(4)
            .in_sequence(&mut seq)
            .withf(|_, collect_token| collect_token.len() == 1 && collect_token[0].collect_id == 42)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        routine();

        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#42
root []
    child1 [("k1", "v1")]
        grandchild []
    child2 []
"#
        );
    }

    #[test]
    fn span_with_parents() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let routine = || {
            let parent_ctx = SpanContext::random();
            let parent1 = Span::root("parent1", parent_ctx);
            let parent2 = Span::root("parent2", parent_ctx);
            let parent3 = Span::root("parent3", parent_ctx);
            let parent4 = Span::root("parent4", parent_ctx);
            let parent5 = Span::root("parent5", parent_ctx);
            let child1 = Span::enter_with_parent("child1", &parent5);
            let child2 = Span::enter_with_parents("child2", [
                &parent1, &parent2, &parent3, &parent4, &parent5, &child1,
            ])
            .with_property(|| ("k1", "v1"));

            crossbeam::scope(move |scope| {
                let mut rng = rng();
                let mut spans = [child1, child2];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
            crossbeam::scope(move |scope| {
                let mut rng = rng();
                let mut spans = [parent1, parent2, parent3, parent4, parent5];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(5)
            .in_sequence(&mut seq)
            .returning({
                let id = Arc::new(AtomicUsize::new(1));
                move || id.fetch_add(1, Ordering::SeqCst)
            });
        mock.expect_submit_spans()
            .times(7)
            .in_sequence(&mut seq)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(5)
            .with(predicate::in_iter([1_usize, 2, 3, 4, 5]))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        routine();

        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#1
parent1 []
    child2 [("k1", "v1")]

#2
parent2 []
    child2 [("k1", "v1")]

#3
parent3 []
    child2 [("k1", "v1")]

#4
parent4 []
    child2 [("k1", "v1")]

#5
parent5 []
    child1 []
        child2 [("k1", "v1")]
    child2 [("k1", "v1")]
"#
        );
    }

    #[test]
    fn span_push_child_spans() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let routine = || {
            let parent_ctx = SpanContext::random();
            let parent1 = Span::root("parent1", parent_ctx);
            let parent2 = Span::root("parent2", parent_ctx);
            let parent3 = Span::root("parent3", parent_ctx);
            let parent4 = Span::root("parent4", parent_ctx);
            let parent5 = Span::root("parent5", parent_ctx);

            let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));
            let collector = LocalCollector::new(None, stack.clone());
            {
                let _s = LocalSpan::enter_with_stack("child", stack);
            }
            let spans = collector.collect();

            for parent in [&parent1, &parent2, &parent3, &parent4, &parent5] {
                parent.push_child_spans(spans.clone());
            }

            crossbeam::scope(move |scope| {
                let mut rng = rng();
                let mut spans = [parent1, parent2, parent3, parent4, parent5];
                spans.shuffle(&mut rng);
                for span in spans {
                    scope.spawn(|_| drop(span));
                }
            })
            .unwrap();
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(5)
            .in_sequence(&mut seq)
            .returning({
                let id = Arc::new(AtomicUsize::new(1));
                move || id.fetch_add(1, Ordering::SeqCst)
            });
        mock.expect_submit_spans()
            .times(10)
            .in_sequence(&mut seq)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(5)
            .with(predicate::in_iter([1_usize, 2, 3, 4, 5]))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        routine();

        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r"
#1
parent1 []
    child []

#2
parent2 []
    child []

#3
parent3 []
    child []

#4
parent4 []
    child []

#5
parent5 []
    child []
"
        );
    }

    #[test]
    fn span_communicate_via_stack() {
        crate::set_reporter(ConsoleReporter, crate::collector::Config::default());

        let routine = || {
            let stack = Rc::new(RefCell::new(LocalSpanStack::with_capacity(16)));

            {
                let parent_ctx = SpanContext::random();
                let root = Span::root("root", parent_ctx);
                let _g = root.attach_into_stack(&stack);
                let child = Span::enter_with_stack("child", &mut stack.borrow_mut());
                {
                    let _g = child.attach_into_stack(&stack);
                    let _s = Span::enter_with_stack("grandchild", &mut stack.borrow_mut());
                }
                let _s = LocalSpan::enter_with_stack("local", stack);
            }
        };

        let mut mock = MockGlobalCollect::new();
        let mut seq = Sequence::new();
        let span_sets = Arc::new(Mutex::new(Vec::new()));
        mock.expect_start_collect()
            .times(1)
            .in_sequence(&mut seq)
            .return_const(42_usize);
        mock.expect_submit_spans()
            .times(5)
            .in_sequence(&mut seq)
            .withf(|_, collect_token| collect_token.len() == 1 && collect_token[0].collect_id == 42)
            .returning({
                let span_sets = span_sets.clone();
                move |span_set, token| span_sets.lock().unwrap().push((span_set, token))
            });
        mock.expect_commit_collect()
            .times(1)
            .in_sequence(&mut seq)
            .with(predicate::eq(42_usize))
            .return_const(());
        mock.expect_drop_collect().times(0);

        let mock = Arc::new(mock);
        set_mock_collect(mock);

        routine();

        let span_sets = std::mem::take(&mut *span_sets.lock().unwrap());
        assert_eq!(
            tree_str_from_span_sets(span_sets.as_slice()),
            r#"
#42
root []
    child []
        grandchild []
    local []
"#
        );
    }
}
