// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use fastrace::Span;
use fastrace::local::LocalSpan;
use futures_core::Stream;
use futures_sink::Sink;

/// An extension trait for [`Stream`] that provides tracing instrument adapters.
pub trait StreamExt: Stream + Sized {
    /// Binds a [`Span`] to the [`Stream`] that continues to record until the stream is
    /// **finished**.
    ///
    /// In addition, it sets the span as the local parent at every poll so that
    /// [`fastrace::local::LocalSpan`] becomes available within the future. Internally, it
    /// calls [`Span::set_local_parent`] when the executor polls it.
    ///
    /// # Examples:
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use async_stream::stream;
    /// use fastrace::prelude::*;
    /// use fastrace_futures::StreamExt as _;
    /// use futures::StreamExt;
    ///
    /// let root = Span::root("root", SpanContext::random());
    /// let s = stream! {
    ///     for i in 0..2 {
    ///         yield i;
    ///     }
    /// }
    /// .in_span(Span::enter_with_parent("task", &root));
    ///
    /// tokio::pin!(s);
    ///
    /// assert_eq!(s.next().await.unwrap(), 0);
    /// assert_eq!(s.next().await.unwrap(), 1);
    /// assert_eq!(s.next().await, None);
    /// // span ends here.
    /// # }
    /// ```
    fn in_span(self, span: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(span),
        }
    }

    /// Starts a [`LocalSpan`] at every [`Stream::poll_next()`].
    ///
    /// This is useful for tracing each **poll** of a stream (not each yielded item),
    /// e.g. to observe how often an async stream is woken. If you need a single span
    /// that covers the whole stream lifecycle, use [`StreamExt::in_span`] instead.
    ///
    /// The span name can be any `impl Into<Cow<'static, str>>`.
    ///
    /// # Important: Local parent required
    ///
    /// `enter_on_poll` creates [`LocalSpan`]s, which require an existing local parent
    /// context at the time of each poll. Without one, the spans will be no-ops.
    ///
    /// The typical way to provide a local parent is to wrap the stream with
    /// [`StreamExt::in_span`] **after** `enter_on_poll`:
    ///
    /// ```text
    /// stream.enter_on_poll("poll").in_span(span)
    /// ```
    ///
    /// ⚠️ Do **not** reverse the order:
    ///
    /// ```text
    /// // WRONG: in_span sets the local parent *after* enter_on_poll tries to create
    /// // the LocalSpan, so the poll spans will be no-ops.
    /// stream.in_span(span).enter_on_poll("poll")
    /// ```
    ///
    /// # Examples:
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use async_stream::stream;
    /// use fastrace::prelude::*;
    /// use fastrace_futures::StreamExt as _;
    /// use futures::StreamExt;
    ///
    /// let root = Span::root("root", SpanContext::random());
    ///
    /// let s = stream! {
    ///     for i in 0..2 {
    ///         yield i;
    ///     }
    /// }
    /// .enter_on_poll("poll")
    /// .in_span(Span::enter_with_parent("stream", &root));
    ///
    /// tokio::pin!(s);
    ///
    /// assert_eq!(s.next().await.unwrap(), 0);
    /// assert_eq!(s.next().await.unwrap(), 1);
    /// assert_eq!(s.next().await, None);
    /// # }
    /// ```
    fn enter_on_poll(self, name: impl Into<Cow<'static, str>>) -> EnterOnPollStream<Self> {
        EnterOnPollStream {
            inner: self,
            name: name.into(),
        }
    }
}

impl<T> StreamExt for T where T: Stream {}

/// An extension trait for [`Sink`] that provides tracing instrument adapters.
pub trait SinkExt<Item>: Sink<Item> + Sized {
    /// Binds a [`Span`] to the [`Sink`] that continues to record until the sink is **closed**.
    ///
    /// In addition, it sets the span as the local parent at every poll so that
    /// [`fastrace::local::LocalSpan`] becomes available within the future. Internally, it
    /// calls [`Span::set_local_parent`] when the executor polls it.
    ///
    /// # Examples:
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use fastrace::prelude::*;
    /// use fastrace_futures::SinkExt as _;
    /// use futures::sink;
    /// use futures::sink::SinkExt;
    ///
    /// let root = Span::root("root", SpanContext::random());
    ///
    /// let mut drain = sink::drain().in_span(Span::enter_with_parent("task", &root));
    ///
    /// drain.send(1).await.unwrap();
    /// drain.send(2).await.unwrap();
    /// drain.close().await.unwrap();
    /// // span ends here.
    /// # }
    /// ```
    fn in_span(self, span: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(span),
        }
    }
}

impl<T, Item> SinkExt<Item> for T where T: Sink<Item> {}

/// Adapter for [`StreamExt::in_span()`](StreamExt::in_span) and
/// [`SinkExt::in_span()`](SinkExt::in_span).
#[pin_project::pin_project]
pub struct InSpan<T> {
    #[pin]
    inner: T,
    span: Option<Span>,
}

impl<T> Stream for InSpan<T>
where T: Stream
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        let res = this.inner.poll_next(cx);

        match res {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                // finished
                this.span.take();
                Poll::Ready(None)
            }
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
        }
    }
}

impl<T, I> Sink<I> for InSpan<T>
where T: Sink<I>
{
    type Error = T::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        this.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        let this = self.project();
        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        this.inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        this.inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();

        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        let res = this.inner.poll_close(cx);

        match res {
            r @ Poll::Pending => r,
            other => {
                // closed
                this.span.take();
                other
            }
        }
    }
}

/// Adapter for [`StreamExt::enter_on_poll()`](StreamExt::enter_on_poll).
#[pin_project::pin_project]
pub struct EnterOnPollStream<T> {
    #[pin]
    inner: T,
    name: Cow<'static, str>,
}

impl<T> Stream for EnterOnPollStream<T>
where T: Stream
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let _guard = LocalSpan::enter_with_local_parent(this.name.clone());
        this.inner.poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use fastrace::local::LocalCollector;
    use fastrace::prelude::*;
    use futures::StreamExt as _;
    use futures::stream;

    use crate::StreamExt as _;

    #[tokio::test]
    async fn test_enter_on_poll_creates_spans() {
        let collector = LocalCollector::start();

        let s = stream::iter(vec![1, 2]).enter_on_poll("poll");
        tokio::pin!(s);
        assert_eq!(s.next().await, Some(1));
        assert_eq!(s.next().await, Some(2));
        assert_eq!(s.next().await, None);

        let local_spans = collector.collect();
        let parent_ctx = SpanContext::random();
        let spans = local_spans.to_span_records(parent_ctx);

        let poll_count = spans.iter().filter(|s| s.name == "poll").count();
        assert!(
            poll_count >= 2,
            "expected at least 2 poll spans, got {}",
            poll_count
        );
    }

    #[tokio::test]
    async fn test_enter_on_poll_pending_then_ready() {
        use std::pin::Pin;
        use std::task::Context;
        use std::task::Poll;

        use futures::stream::Stream;

        struct PendOnce {
            polled: bool,
        }

        impl Stream for PendOnce {
            type Item = i32;
            fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<i32>> {
                if self.polled {
                    Poll::Ready(Some(42))
                } else {
                    self.polled = true;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }

        let collector = LocalCollector::start();

        let s = PendOnce { polled: false }.enter_on_poll("poll");
        tokio::pin!(s);
        assert_eq!(s.next().await, Some(42));

        let local_spans = collector.collect();
        let parent_ctx = SpanContext::random();
        let spans = local_spans.to_span_records(parent_ctx);

        let poll_count = spans.iter().filter(|s| s.name == "poll").count();
        assert!(
            poll_count >= 2,
            "expected at least 2 poll spans (pending + ready), got {}",
            poll_count
        );
    }
}
