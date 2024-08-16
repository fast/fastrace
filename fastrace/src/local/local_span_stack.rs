// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::local::local_span_line::LocalSpanHandle;
use crate::local::local_span_line::SpanLine;
use crate::util::CollectToken;
use crate::util::RawSpans;

const DEFAULT_SPAN_STACK_SIZE: usize = 4096;
const DEFAULT_SPAN_QUEUE_SIZE: usize = 10240;

thread_local! {
    pub static LOCAL_SPAN_STACK: Rc<RefCell<LocalSpanStack>> = Rc::new(RefCell::new(LocalSpanStack::with_capacity(DEFAULT_SPAN_STACK_SIZE)));
}

pub struct LocalSpanStack {
    span_lines: Vec<SpanLine>,
    capacity: usize,
    next_span_line_epoch: usize,
}

impl LocalSpanStack {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_lines: Vec::with_capacity(capacity / 8),
            capacity,
            next_span_line_epoch: 0,
        }
    }

    #[inline]
    pub fn enter_span(&mut self, name: impl Into<Cow<'static, str>>) -> Option<LocalSpanHandle> {
        let span_line = self.current_span_line()?;
        span_line.start_span(name)
    }

    #[inline]
    pub fn exit_span(&mut self, local_span_handle: LocalSpanHandle) {
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.finish_span(local_span_handle);
        }
    }

    #[inline]
    pub fn add_event<I, F>(&mut self, name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        if let Some(span_line) = self.current_span_line() {
            span_line.add_event(name, properties);
        }
    }

    /// Register a new span line to the span stack. If succeed, return a span line epoch which can
    /// be used to unregister the span line via [`LocalSpanStack::unregister_and_collect`]. If
    /// the size of the span stack is greater than the `capacity`, registration will fail
    /// and a `None` will be returned.
    ///
    /// [`LocalSpanStack::unregister_and_collect`](LocalSpanStack::unregister_and_collect)
    #[inline]
    pub fn register_span_line(
        &mut self,
        collect_token: Option<CollectToken>,
    ) -> Option<SpanLineHandle> {
        if self.span_lines.len() >= self.capacity {
            return None;
        }

        let epoch = self.next_span_line_epoch;
        self.next_span_line_epoch = self.next_span_line_epoch.wrapping_add(1);

        let span_line = SpanLine::new(DEFAULT_SPAN_QUEUE_SIZE, epoch, collect_token);
        self.span_lines.push(span_line);
        Some(SpanLineHandle {
            span_line_epoch: epoch,
        })
    }

    pub fn unregister_and_collect(
        &mut self,
        span_line_handle: SpanLineHandle,
    ) -> Option<(RawSpans, Option<CollectToken>)> {
        debug_assert_eq!(
            self.current_span_line().unwrap().span_line_epoch(),
            span_line_handle.span_line_epoch,
        );
        let span_line = self.span_lines.pop()?;
        span_line.collect(span_line_handle.span_line_epoch)
    }

    #[inline]
    pub fn add_properties<K, V, I, F>(&mut self, properties: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        if let Some(span_line) = self.current_span_line() {
            span_line.add_properties(properties);
        }
    }

    #[inline]
    pub fn with_properties<K, V, I, F>(
        &mut self,
        local_span_handle: &LocalSpanHandle,
        properties: F,
    ) where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        debug_assert!(self.current_span_line().is_some());
        if let Some(span_line) = self.current_span_line() {
            debug_assert_eq!(
                span_line.span_line_epoch(),
                local_span_handle.span_line_epoch
            );
            span_line.with_properties(local_span_handle, properties);
        }
    }

    pub fn current_collect_token(&mut self) -> Option<CollectToken> {
        let span_line = self.current_span_line()?;
        span_line.current_collect_token()
    }

    #[inline]
    pub fn current_span_line(&mut self) -> Option<&mut SpanLine> {
        self.span_lines.last_mut()
    }
}

pub struct SpanLineHandle {
    span_line_epoch: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectTokenItem;
    use crate::collector::SpanId;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_raw_spans;

    #[test]
    fn span_stack_basic() {
        let mut span_stack = LocalSpanStack::with_capacity(16);

        let token1 = CollectTokenItem {
            trace_id: TraceId(1234),
            parent_id: SpanId::default(),
            collect_id: 42,
            is_root: false,
            is_sampled: true,
        };
        let span_line1 = span_stack.register_span_line(Some(token1.into())).unwrap();
        {
            {
                let span1 = span_stack.enter_span("span1").unwrap();
                {
                    let span2 = span_stack.enter_span("span2").unwrap();
                    span_stack.exit_span(span2);
                }
                span_stack.exit_span(span1);
            }

            let token2 = CollectTokenItem {
                trace_id: TraceId(1235),
                parent_id: SpanId::default(),
                collect_id: 48,
                is_root: false,
                is_sampled: true,
            };
            let span_line2 = span_stack.register_span_line(Some(token2.into())).unwrap();
            {
                let span3 = span_stack.enter_span("span3").unwrap();
                {
                    let span4 = span_stack.enter_span("span4").unwrap();
                    span_stack.exit_span(span4);
                }
                span_stack.exit_span(span3);
            }

            let (spans, collect_token) = span_stack.unregister_and_collect(span_line2).unwrap();
            assert_eq!(collect_token.unwrap().as_slice(), &[token2]);
            assert_eq!(
                tree_str_from_raw_spans(spans),
                r"
span3 []
    span4 []
"
            );
        }

        let (spans, collect_token) = span_stack.unregister_and_collect(span_line1).unwrap();
        assert_eq!(collect_token.unwrap().as_slice(), &[token1]);
        assert_eq!(
            tree_str_from_raw_spans(spans),
            r"
span1 []
    span2 []
"
        );
    }

    #[test]
    fn span_stack_is_full() {
        let mut span_stack = LocalSpanStack::with_capacity(4);

        let span_line1 = span_stack.register_span_line(None).unwrap();
        {
            let span_line2 = span_stack.register_span_line(None).unwrap();
            {
                let span_line3 = span_stack
                    .register_span_line(Some(
                        CollectTokenItem {
                            trace_id: TraceId(1234),
                            parent_id: SpanId::default(),
                            collect_id: 42,
                            is_root: false,
                            is_sampled: true,
                        }
                        .into(),
                    ))
                    .unwrap();
                {
                    let span_line4 = span_stack.register_span_line(None).unwrap();
                    {
                        assert!(
                            span_stack
                                .register_span_line(Some(
                                    CollectTokenItem {
                                        trace_id: TraceId(1235),
                                        parent_id: SpanId::default(),
                                        collect_id: 43,
                                        is_root: false,
                                        is_sampled: true,
                                    }
                                    .into()
                                ))
                                .is_none()
                        );
                        assert!(span_stack.register_span_line(None).is_none());
                    }
                    let _ = span_stack.unregister_and_collect(span_line4).unwrap();
                }
                {
                    let span_line5 = span_stack.register_span_line(None).unwrap();
                    {
                        assert!(
                            span_stack
                                .register_span_line(Some(
                                    CollectTokenItem {
                                        trace_id: TraceId(1236),
                                        parent_id: SpanId::default(),
                                        collect_id: 44,
                                        is_root: false,
                                        is_sampled: true,
                                    }
                                    .into()
                                ))
                                .is_none()
                        );
                        assert!(span_stack.register_span_line(None).is_none());
                    }
                    let _ = span_stack.unregister_and_collect(span_line5).unwrap();
                }
                let _ = span_stack.unregister_and_collect(span_line3).unwrap();
            }
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    fn current_collect_token() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        assert!(span_stack.current_collect_token().is_none());
        let token1 = CollectTokenItem {
            trace_id: TraceId(1),
            parent_id: SpanId(1),
            collect_id: 1,
            is_root: false,
            is_sampled: true,
        };
        let span_line1 = span_stack.register_span_line(Some(token1.into())).unwrap();
        assert_eq!(span_stack.current_collect_token().unwrap().as_slice(), &[
            token1
        ]);
        {
            let span_line2 = span_stack.register_span_line(None).unwrap();
            assert!(span_stack.current_collect_token().is_none());
            {
                let token3 = CollectTokenItem {
                    trace_id: TraceId(3),
                    parent_id: SpanId(3),
                    collect_id: 3,
                    is_root: false,
                    is_sampled: true,
                };
                let span_line3 = span_stack.register_span_line(Some(token3.into())).unwrap();
                assert_eq!(span_stack.current_collect_token().unwrap().as_slice(), &[
                    token3
                ]);
                let _ = span_stack.unregister_and_collect(span_line3).unwrap();
            }
            assert!(span_stack.current_collect_token().is_none());
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();

            let token4 = CollectTokenItem {
                trace_id: TraceId(4),
                parent_id: SpanId(4),
                collect_id: 4,
                is_root: false,
                is_sampled: true,
            };
            let span_line4 = span_stack.register_span_line(Some(token4.into())).unwrap();
            assert_eq!(span_stack.current_collect_token().unwrap().as_slice(), &[
                token4
            ]);
            let _ = span_stack.unregister_and_collect(span_line4).unwrap();
        }
        assert_eq!(span_stack.current_collect_token().unwrap().as_slice(), &[
            token1
        ]);
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
        assert!(span_stack.current_collect_token().is_none());
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_exit_span() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        let span1 = span_stack.enter_span("span1").unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(Some(
                    CollectTokenItem {
                        trace_id: TraceId(1234),
                        parent_id: SpanId::default(),
                        collect_id: 42,
                        is_root: false,
                        is_sampled: true,
                    }
                    .into(),
                ))
                .unwrap();
            span_stack.exit_span(span1);
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_add_properties() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        let span1 = span_stack.enter_span("span1").unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(Some(
                    CollectTokenItem {
                        trace_id: TraceId(1234),
                        parent_id: SpanId::default(),
                        collect_id: 42,
                        is_root: false,
                        is_sampled: true,
                    }
                    .into(),
                ))
                .unwrap();
            span_stack.with_properties(&span1, || [("k1", "v1")]);
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
        span_stack.exit_span(span1);
        let _ = span_stack.unregister_and_collect(span_line1).unwrap();
    }

    #[test]
    #[should_panic]
    fn unmatched_span_line_collect() {
        let mut span_stack = LocalSpanStack::with_capacity(16);
        let span_line1 = span_stack.register_span_line(None).unwrap();
        {
            let span_line2 = span_stack
                .register_span_line(Some(
                    CollectTokenItem {
                        trace_id: TraceId(1234),
                        parent_id: SpanId::default(),
                        collect_id: 42,
                        is_root: false,
                        is_sampled: true,
                    }
                    .into(),
                ))
                .unwrap();
            let _ = span_stack.unregister_and_collect(span_line1).unwrap();
            let _ = span_stack.unregister_and_collect(span_line2).unwrap();
        }
    }
}
