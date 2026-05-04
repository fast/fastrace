// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! This module provides tools to trace a `Future`.
//!
//! The [`FutureExt`] trait extends `Future` with [`in_span()`]. It is crucial that the
//! outermost future uses `in_span()`, otherwise, the traces inside the `Future` will be lost.
//!
//! # Example
//!
//! ```
//! use fastrace::prelude::*;
//!
//! let root = Span::root("root", SpanContext::random());
//!
//! // Instrument a task.
//! let task = async {
//!     // ...
//! }
//! .in_span(Span::start("task", &root))
//! .with_poll_span("future is polled");
//!
//!     # let runtime = tokio::runtime::Runtime::new().unwrap();
//! runtime.spawn(task);
//! ```
//!
//! [`in_span()`]:(FutureExt::in_span)

use std::borrow::Cow;
use std::task::Poll;

use crate::Span;
use crate::local::LocalSpan;

impl<T: std::future::Future> FutureExt for T {}

/// An extension trait for `Futures` that provides tracing instrument adapters.
pub trait FutureExt: std::future::Future + Sized {
    /// Binds a [`Span`] to the [`Future`] that continues to record until the future is dropped.
    ///
    /// In addition, it sets the span as the local parent at every poll so that `LocalSpan`
    /// becomes available within the future. Internally, it calls [`Span::set_local_parent`] when
    /// the executor [`polls`](std::future::Future::poll) it.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use fastrace::prelude::*;
    ///
    /// let root = Span::root("Root", SpanContext::random());
    /// let task = async {
    ///     // ...
    /// }
    /// .in_span(Span::start("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    ///
    /// [`Future`]:(std::future::Future)
    #[inline]
    fn in_span(self, span: Span) -> InSpan<Self> {
        InSpan {
            inner: self,
            span: Some(span),
            poll_span: None,
        }
    }
}

/// Adapter for [`FutureExt::in_span()`](FutureExt::in_span).
#[pin_project::pin_project]
pub struct InSpan<T> {
    #[pin]
    inner: T,
    span: Option<Span>,
    poll_span: Option<Cow<'static, str>>,
}

impl<T> InSpan<T> {
    /// Starts a [`LocalSpan`] at every [`Future::poll()`].
    ///
    /// If the future gets polled multiple times, it will create multiple short spans.
    /// The poll span is always created under the future span.
    ///
    /// [`Future::poll()`]:(std::future::Future::poll)
    #[inline]
    pub fn with_poll_span(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.poll_span = Some(name.into());
        self
    }
}

impl<T: std::future::Future> std::future::Future for InSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let guard = this.span.as_ref().map(|s| s.set_local_parent());
        let poll_span = if this.span.is_some() {
            this.poll_span
                .as_ref()
                .map(|name| LocalSpan::start(name.clone()))
        } else {
            None
        };
        let res = this.inner.poll(cx);
        drop(poll_span);
        drop(guard);

        match res {
            r @ Poll::Pending => r,
            other => {
                this.poll_span.take();
                this.span.take();
                other
            }
        }
    }
}
