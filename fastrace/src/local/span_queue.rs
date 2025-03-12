// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use fastant::Instant;

use super::raw_span::RawKind;
use crate::collector::SpanId;
use crate::local::raw_span::RawSpan;
use crate::util::RawSpans;

pub struct SpanQueue {
    span_queue: RawSpans,
    capacity: usize,
    next_parent_id: Option<SpanId>,
}

pub struct SpanHandle {
    index: usize,
}

impl SpanQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            span_queue: RawSpans::default(),
            capacity,
            next_parent_id: None,
        }
    }

    #[inline]
    pub fn start_span(&mut self, name: impl Into<Cow<'static, str>>) -> Option<SpanHandle> {
        if self.span_queue.len() >= self.capacity {
            return None;
        }

        let span = RawSpan::begin_with(
            SpanId::next_id(),
            self.next_parent_id.unwrap_or_default(),
            Instant::now(),
            name,
            RawKind::Span,
        );
        self.next_parent_id = Some(span.id);

        let index = self.span_queue.len();
        self.span_queue.push(span);

        Some(SpanHandle { index })
    }

    #[inline]
    pub fn finish_span(&mut self, span_handle: SpanHandle) {
        debug_assert!(span_handle.index < self.span_queue.len());
        debug_assert_eq!(
            self.next_parent_id,
            Some(self.span_queue[span_handle.index].id)
        );

        let span = &mut self.span_queue[span_handle.index];
        span.end_with(Instant::now());

        self.next_parent_id = Some(span.parent_id).filter(|id| *id != SpanId::default());
    }

    #[inline]
    pub fn add_event<I, F>(&mut self, name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        if self.span_queue.len() >= self.capacity {
            return;
        }

        let mut span = RawSpan::begin_with(
            SpanId::next_id(),
            self.next_parent_id.unwrap_or_default(),
            Instant::now(),
            name,
            RawKind::Event,
        );
        span.properties.extend(properties());

        self.span_queue.push(span);
    }

    #[inline]
    pub fn add_properties<K, V, I>(&mut self, properties: I)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
    {
        if self.span_queue.len() >= self.capacity {
            return;
        }

        let mut span = RawSpan::begin_with(
            SpanId::next_id(),
            self.next_parent_id.unwrap_or_default(),
            Instant::ZERO,
            Cow::Borrowed(""),
            RawKind::Properties,
        );
        span.properties
            .extend(properties.into_iter().map(|(k, v)| (k.into(), v.into())));

        self.span_queue.push(span);
    }

    #[inline]
    pub fn with_properties<K, V, I>(&mut self, span_handle: &SpanHandle, properties: I)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
    {
        debug_assert!(span_handle.index < self.span_queue.len());

        let span = &mut self.span_queue[span_handle.index];
        span.properties
            .extend(properties.into_iter().map(|(k, v)| (k.into(), v.into())));
    }

    #[inline]
    pub fn take_queue(self) -> RawSpans {
        self.span_queue
    }

    #[inline]
    pub fn current_parent_id(&self) -> Option<SpanId> {
        self.next_parent_id
    }

    #[cfg(test)]
    pub fn get_raw_span(&self, handle: &SpanHandle) -> &RawSpan {
        &self.span_queue[handle.index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::tree::tree_str_from_raw_spans;

    #[test]
    fn span_queue_basic() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let span1 = queue.start_span("span1").unwrap();
            {
                let span2 = queue.start_span("span2").unwrap();
                {
                    let span3 = queue.start_span("span3").unwrap();
                    queue.finish_span(span3);
                }
                queue.finish_span(span2);
            }
            queue.finish_span(span1);
        }
        assert_eq!(
            tree_str_from_raw_spans(queue.take_queue()),
            r"
span1 []
    span2 []
        span3 []
"
        );
    }

    #[test]
    fn span_add_properties() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let span1 = queue.start_span("span1").unwrap();
            queue.with_properties(&span1, [("k1", "v1"), ("k2", "v2")]);
            {
                let span2 = queue.start_span("span2").unwrap();
                queue.with_properties(&span2, [("k1", "v1")]);
                queue.finish_span(span2);
            }
            queue.finish_span(span1);
        }
        assert_eq!(
            tree_str_from_raw_spans(queue.take_queue()),
            r#"
span1 [("k1", "v1"), ("k2", "v2")]
    span2 [("k1", "v1")]
"#
        );
    }

    #[test]
    fn span_not_finished() {
        let mut queue = SpanQueue::with_capacity(16);
        {
            let _span1 = queue.start_span("span1").unwrap();
            {
                let _span2 = queue.start_span("span2").unwrap();
                {
                    let _span3 = queue.start_span("span3").unwrap();
                }
            }
        }
        assert_eq!(
            tree_str_from_raw_spans(queue.take_queue()),
            r"
span1 []
    span2 []
        span3 []
"
        );
    }

    #[test]
    #[should_panic]
    fn finish_span_out_of_order() {
        let mut queue = SpanQueue::with_capacity(16);
        let span1 = queue.start_span("span1").unwrap();
        let span2 = queue.start_span("span2").unwrap();
        queue.finish_span(span1);
        queue.finish_span(span2);
    }

    #[test]
    fn span_queue_out_of_size() {
        let mut queue = SpanQueue::with_capacity(4);
        {
            let span1 = queue.start_span("span1").unwrap();
            {
                let span2 = queue.start_span("span2").unwrap();
                {
                    let span3 = queue.start_span("span3").unwrap();
                    {
                        let span4 = queue.start_span("span4").unwrap();
                        assert!(queue.start_span("span5").is_none());
                        queue.finish_span(span4);
                    }
                    assert!(queue.start_span("span5").is_none());
                    queue.finish_span(span3);
                }
                assert!(queue.start_span("span5").is_none());
                queue.finish_span(span2);
            }
            assert!(queue.start_span("span5").is_none());
            queue.finish_span(span1);
        }
        assert!(queue.start_span("span5").is_none());
        assert_eq!(
            tree_str_from_raw_spans(queue.take_queue()),
            r"
span1 []
    span2 []
        span3 []
            span4 []
"
        );
    }

    #[test]
    fn last_span_id() {
        let mut queue = SpanQueue::with_capacity(16);

        assert_eq!(queue.current_parent_id(), None);
        {
            let span1 = queue.start_span("span1").unwrap();
            assert_eq!(
                queue.current_parent_id().unwrap(),
                queue.get_raw_span(&span1).id
            );
            queue.finish_span(span1);
            assert_eq!(queue.current_parent_id(), None);
        }
        {
            let span2 = queue.start_span("span2").unwrap();
            assert_eq!(
                queue.current_parent_id().unwrap(),
                queue.get_raw_span(&span2).id
            );
            {
                let span3 = queue.start_span("span3").unwrap();
                assert_eq!(
                    queue.current_parent_id().unwrap(),
                    queue.get_raw_span(&span3).id
                );
                queue.finish_span(span3);
                assert_eq!(
                    queue.current_parent_id().unwrap(),
                    queue.get_raw_span(&span2).id
                );
            }
            {
                let span4 = queue.start_span("span4").unwrap();
                assert_eq!(
                    queue.current_parent_id().unwrap(),
                    queue.get_raw_span(&span4).id
                );
                {
                    let span5 = queue.start_span("span5").unwrap();
                    assert_eq!(
                        queue.current_parent_id().unwrap(),
                        queue.get_raw_span(&span5).id
                    );
                    {
                        let span6 = queue.start_span("span6").unwrap();
                        assert_eq!(
                            queue.current_parent_id().unwrap(),
                            queue.get_raw_span(&span6).id
                        );
                        queue.finish_span(span6);
                        assert_eq!(
                            queue.current_parent_id().unwrap(),
                            queue.get_raw_span(&span5).id
                        );
                    }
                    queue.finish_span(span5);
                    assert_eq!(
                        queue.current_parent_id().unwrap(),
                        queue.get_raw_span(&span4).id
                    );
                }
                queue.finish_span(span4);
                assert_eq!(
                    queue.current_parent_id().unwrap(),
                    queue.get_raw_span(&span2).id
                );
            }
            queue.finish_span(span2);
            assert_eq!(queue.current_parent_id(), None);
        }
        {
            let span7 = queue.start_span("span7").unwrap();
            assert_eq!(
                queue.current_parent_id().unwrap(),
                queue.get_raw_span(&span7).id
            );
            queue.finish_span(span7);
            assert_eq!(queue.current_parent_id(), None);
        }
        assert_eq!(
            tree_str_from_raw_spans(queue.take_queue()),
            r"
span1 []

span2 []
    span3 []
    span4 []
        span5 []
            span6 []

span7 []
"
        );
    }
}
