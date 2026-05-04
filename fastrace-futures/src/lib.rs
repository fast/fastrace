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
    /// .in_span(Span::start("task", &root));
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
            poll_span: None,
        }
    }
}

impl<T> StreamExt for T where T: Stream {}

/// An extension trait for [`Sink`] that provides tracing instrument adapters.
pub trait SinkExt<Item>: Sink<Item> + Sized {
    /// Binds a [`Span`] to the [`Sink`] that continues to record until the sink is
    /// **closed**.
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
    /// let mut drain = sink::drain().in_span(Span::start("task", &root));
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
            poll_span: None,
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
    poll_span: Option<Cow<'static, str>>,
}

impl<T: Stream> InSpan<T> {
    /// Starts a [`LocalSpan`] at every [`Stream::poll_next()`].
    ///
    /// If the stream gets polled multiple times, it will create multiple short spans.
    /// The poll span is always created under the stream span.
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
    /// .in_span(Span::start("stream", &root))
    /// .with_poll_span("poll");
    ///
    /// tokio::pin!(s);
    ///
    /// assert_eq!(s.next().await.unwrap(), 0);
    /// assert_eq!(s.next().await.unwrap(), 1);
    /// assert_eq!(s.next().await, None);
    /// # }
    /// ```
    #[inline]
    pub fn with_poll_span(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.poll_span = Some(name.into());
        self
    }
}

impl<T> Stream for InSpan<T>
where T: Stream
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        let guard = this.span.as_ref().map(|s| s.set_local_parent());
        let poll_span = if this.span.is_some() {
            this.poll_span
                .as_ref()
                .map(|name| LocalSpan::start(name.clone()))
        } else {
            None
        };
        let res = this.inner.poll_next(cx);
        drop(poll_span);
        drop(guard);

        match res {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                // finished
                this.poll_span.take();
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

        let guard = this.span.as_ref().map(|s| s.set_local_parent());
        let res = this.inner.poll_close(cx);
        drop(guard);

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

#[cfg(test)]
mod tests {
    use fastrace::collector::Config;
    use fastrace::collector::TestReporter;
    use fastrace::prelude::*;
    use futures::StreamExt as _;
    use futures::stream;

    use crate::StreamExt as _;

    static REPORTER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn test_with_poll_span_creates_spans() {
        let _lock = REPORTER_LOCK.lock().await;
        let (reporter, collected_spans) = TestReporter::new();
        fastrace::set_reporter(reporter, Config::default());

        {
            let root = Span::root("root", SpanContext::random());
            let s = stream::iter(vec![1, 2])
                .in_span(Span::start("stream", &root))
                .with_poll_span("poll");
            tokio::pin!(s);
            assert_eq!(s.next().await, Some(1));
            assert_eq!(s.next().await, Some(2));
            assert_eq!(s.next().await, None);
        }

        fastrace::flush();
        let spans = collected_spans.lock();

        let poll_count = spans.iter().filter(|s| s.name == "poll").count();
        assert!(
            poll_count >= 2,
            "expected at least 2 poll spans, got {}",
            poll_count
        );
    }

    #[tokio::test]
    async fn test_with_poll_span_pending_then_ready() {
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

        let _lock = REPORTER_LOCK.lock().await;
        let (reporter, collected_spans) = TestReporter::new();
        fastrace::set_reporter(reporter, Config::default());

        {
            let root = Span::root("root", SpanContext::random());
            let s = PendOnce { polled: false }
                .in_span(Span::start("stream", &root))
                .with_poll_span("poll");
            tokio::pin!(s);
            assert_eq!(s.next().await, Some(42));
        }

        fastrace::flush();
        let spans = collected_spans.lock();

        let poll_count = spans.iter().filter(|s| s.name == "poll").count();
        assert!(
            poll_count >= 2,
            "expected at least 2 poll spans (pending + ready), got {}",
            poll_count
        );
    }
}
