// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::borrow::Cow;

use crate::collector::CollectTokenItem;
use crate::local::span_queue::SpanHandle;
use crate::local::span_queue::SpanQueue;
use crate::util::CollectToken;
use crate::util::RawSpans;

pub struct SpanLine {
    span_queue: SpanQueue,
    epoch: usize,
    collect_token: Option<CollectToken>,
    is_sampled: bool,
}

impl SpanLine {
    pub fn new(
        capacity: usize,
        span_line_epoch: usize,
        collect_token: Option<CollectToken>,
    ) -> Self {
        let is_sampled = match &collect_token {
            Some(token) => token.iter().any(|item| item.is_sampled),
            None => true,
        };

        Self {
            span_queue: SpanQueue::with_capacity(capacity),
            epoch: span_line_epoch,
            collect_token,
            is_sampled,
        }
    }

    #[inline]
    pub fn span_line_epoch(&self) -> usize {
        self.epoch
    }

    #[inline]
    pub fn start_span(&mut self, name: impl Into<Cow<'static, str>>) -> Option<LocalSpanHandle> {
        if !self.is_sampled {
            return None;
        }

        Some(LocalSpanHandle {
            span_handle: self.span_queue.start_span(name)?,
            span_line_epoch: self.epoch,
        })
    }

    #[inline]
    pub fn finish_span(&mut self, handle: LocalSpanHandle) {
        if self.epoch == handle.span_line_epoch {
            self.span_queue.finish_span(handle.span_handle);
        }
    }

    #[inline]
    pub fn add_event<I, F>(&mut self, name: impl Into<Cow<'static, str>>, properties: F)
    where
        I: IntoIterator<Item = (Cow<'static, str>, Cow<'static, str>)>,
        F: FnOnce() -> I,
    {
        if !self.is_sampled {
            return;
        }

        self.span_queue.add_event(name, properties);
    }

    #[inline]
    pub fn add_properties<K, V, I, F>(&mut self, properties: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        if !self.is_sampled {
            return;
        }

        self.span_queue.add_properties(properties());
    }

    #[inline]
    pub fn with_properties<K, V, I, F>(&mut self, handle: &LocalSpanHandle, properties: F)
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
        I: IntoIterator<Item = (K, V)>,
        F: FnOnce() -> I,
    {
        if !self.is_sampled {
            return;
        }

        if self.epoch == handle.span_line_epoch {
            self.span_queue
                .with_properties(&handle.span_handle, properties());
        }
    }

    #[inline]
    pub fn current_collect_token(&self) -> Option<CollectToken> {
        self.collect_token.as_ref().map(|collect_token| {
            collect_token
                .iter()
                .map(|item| CollectTokenItem {
                    trace_id: item.trace_id,
                    parent_id: self
                        .span_queue
                        .current_parent_id()
                        .unwrap_or(item.parent_id),
                    collect_id: item.collect_id,
                    is_root: false,
                    is_sampled: true,
                })
                .collect()
        })
    }

    #[inline]
    pub fn collect(self, span_line_epoch: usize) -> Option<(RawSpans, Option<CollectToken>)> {
        (self.epoch == span_line_epoch)
            .then(move || (self.span_queue.take_queue(), self.collect_token))
    }
}

pub struct LocalSpanHandle {
    pub span_line_epoch: usize,
    span_handle: SpanHandle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::SpanId;
    use crate::prelude::TraceId;
    use crate::util::tree::tree_str_from_raw_spans;

    #[test]
    fn span_line_basic() {
        let mut span_line = SpanLine::new(16, 1, None);
        {
            let span1 = span_line.start_span("span1").unwrap();
            {
                let span2 = span_line.start_span("span2").unwrap();
                {
                    let span3 = span_line.start_span("span3").unwrap();
                    span_line.with_properties(&span3, || [("k1", "v1")]);
                    span_line.finish_span(span3);
                }
                span_line.finish_span(span2);
            }
            span_line.finish_span(span1);
        }
        let (spans, collect_token) = span_line.collect(1).unwrap();
        assert!(collect_token.is_none());
        assert_eq!(
            tree_str_from_raw_spans(spans),
            r#"
span1 []
    span2 []
        span3 [("k1", "v1")]
"#
        );
    }

    #[test]
    fn current_collect_token() {
        let token1 = CollectTokenItem {
            trace_id: TraceId(1234),
            parent_id: SpanId::default(),
            collect_id: 42,
            is_root: false,
            is_sampled: true,
        };
        let token2 = CollectTokenItem {
            trace_id: TraceId(1235),
            parent_id: SpanId::default(),
            collect_id: 43,
            is_root: false,
            is_sampled: true,
        };
        let token = [token1, token2].iter().collect();
        let mut span_line = SpanLine::new(16, 1, Some(token));

        let current_token = span_line.current_collect_token().unwrap();
        assert_eq!(current_token.as_slice(), &[token1, token2]);

        let span = span_line.start_span("span").unwrap();
        let current_token = span_line.current_collect_token().unwrap();
        assert_eq!(current_token.len(), 2);
        assert_eq!(current_token.as_slice(), &[
            CollectTokenItem {
                trace_id: TraceId(1234),
                parent_id: span_line.span_queue.current_parent_id().unwrap(),
                collect_id: 42,
                is_root: false,
                is_sampled: true,
            },
            CollectTokenItem {
                trace_id: TraceId(1235),
                parent_id: span_line.span_queue.current_parent_id().unwrap(),
                collect_id: 43,
                is_root: false,
                is_sampled: true,
            }
        ]);
        span_line.finish_span(span);

        let current_token = span_line.current_collect_token().unwrap();
        assert_eq!(current_token.as_slice(), &[token1, token2]);

        let (spans, collect_token) = span_line.collect(1).unwrap();
        assert_eq!(collect_token.unwrap().as_slice(), &[token1, token2]);
        assert_eq!(
            tree_str_from_raw_spans(spans),
            r#"
span []
"#
        );
    }

    #[test]
    fn unmatched_epoch_add_properties() {
        let mut span_line1 = SpanLine::new(16, 1, None);
        let mut span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);

        let span = span_line1.start_span("span").unwrap();
        span_line2.with_properties(&span, || [("k1", "v1")]);
        span_line1.finish_span(span);

        let raw_spans = span_line1.collect(1).unwrap().0.into_inner();
        assert_eq!(raw_spans.len(), 1);
        assert_eq!(raw_spans[0].properties.len(), 0);

        let raw_spans = span_line2.collect(2).unwrap().0.into_inner();
        assert!(raw_spans.is_empty());
    }

    #[test]
    fn unmatched_epoch_finish_span() {
        let item = CollectTokenItem {
            trace_id: TraceId(1234),
            parent_id: SpanId::default(),
            collect_id: 42,
            is_root: false,
            is_sampled: true,
        };
        let mut span_line1 = SpanLine::new(16, 1, Some(item.into()));
        let mut span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);

        let span = span_line1.start_span("span").unwrap();
        let token_before_finish = span_line1.current_collect_token().unwrap();
        span_line2.finish_span(span);

        let token_after_finish = span_line1.current_collect_token().unwrap();
        // the span failed to finish
        assert_eq!(
            token_before_finish.as_slice(),
            token_after_finish.as_slice()
        );

        let (spans, collect_token) = span_line1.collect(1).unwrap();
        let collect_token = collect_token.unwrap();
        assert_eq!(collect_token.as_slice(), &[item]);
        assert_eq!(spans.into_inner().len(), 1);

        let (spans, collect_token) = span_line2.collect(2).unwrap();
        assert!(collect_token.is_none());
        assert!(spans.into_inner().is_empty());
    }

    #[test]
    fn unmatched_epoch_collect() {
        let span_line1 = SpanLine::new(16, 1, None);
        let span_line2 = SpanLine::new(16, 2, None);
        assert_eq!(span_line1.span_line_epoch(), 1);
        assert_eq!(span_line2.span_line_epoch(), 2);
        assert!(span_line1.collect(2).is_none());
        assert!(span_line2.collect(1).is_none());
    }
}
