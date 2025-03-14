// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![doc = include_str!("../README.md")]

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use fastrace::Span;
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
where
    T: Stream,
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
where
    T: Sink<I>,
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
