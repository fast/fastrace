// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! This module provides tools to trace a `Future`.
//!
//! The [`FutureExt`] trait extends `Future` with two methods: [`in_span()`] and
//! [`enter_on_poll()`]. It is crucial that the outermost future uses `in_span()`,
//! otherwise, the traces inside the `Future` will be lost.
//!
//! # Example
//!
//! ```
//! use fastrace::prelude::*;
//!
//! let root = Span::root("root", SpanContext::random());
//!
//! // Instrument the a task
//! let task = async {
//!     async {
//!         // ...
//!     }
//!     .enter_on_poll("future is polled")
//!     .await;
//! }
//! .in_span(Span::enter_with_parent("task", &root));
//!
//!     # let runtime = tokio::runtime::Runtime::new().unwrap();
//! runtime.spawn(task);
//! ```
//!
//! [`in_span()`]:(FutureExt::in_span)
//! [`enter_on_poll()`]:(FutureExt::enter_on_poll)

use std::borrow::Cow;
use std::task::Poll;

use crate::local::LocalSpan;
use crate::Span;

impl<T: std::future::Future> FutureExt for T {}

/// An extension trait for `Futures` that provides tracing instrument adapters.
pub trait FutureExt: std::future::Future + Sized {
    /// Binds a [`Span`] to the [`Future`] that continues to record until the future is dropped.
    ///
    /// In addition, it sets the span as the local parent at every poll so that `LocalSpan`
    /// becomes available within the future. Internally, it calls [`Span::set_local_parent`] when
    /// the executor [`poll`](std::future::Future::poll) it.
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
    /// .in_span(Span::enter_with_parent("Task", &root));
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
        }
    }

    /// Starts a [`LocalSpan`] at every [`Future::poll()`]. If the future gets polled multiple
    /// times, it will create multiple _short_ spans.
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
    ///     async {
    ///         // ...
    ///     }
    ///     .enter_on_poll("Sub Task")
    ///     .await
    /// }
    /// .in_span(Span::enter_with_parent("Task", &root));
    ///
    /// tokio::spawn(task);
    /// # }
    /// ```
    ///
    /// [`Future::poll()`]:(std::future::Future::poll)
    #[inline]
    fn enter_on_poll(self, name: impl Into<Cow<'static, str>>) -> EnterOnPoll<Self> {
        EnterOnPoll {
            inner: self,
            name: name.into(),
        }
    }
}

/// Adapter for [`FutureExt::in_span()`](FutureExt::in_span).
#[pin_project::pin_project]
pub struct InSpan<T> {
    #[pin]
    inner: T,
    span: Option<Span>,
}

impl<T: std::future::Future> std::future::Future for InSpan<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let _guard = this.span.as_ref().map(|s| s.set_local_parent());
        let res = this.inner.poll(cx);

        match res {
            r @ Poll::Pending => r,
            other => {
                this.span.take();
                other
            }
        }
    }
}

/// Adapter for [`FutureExt::enter_on_poll()`](FutureExt::enter_on_poll).
#[pin_project::pin_project]
pub struct EnterOnPoll<T> {
    #[pin]
    inner: T,
    name: Cow<'static, str>,
}

impl<T: std::future::Future> std::future::Future for EnterOnPoll<T> {
    type Output = T::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = LocalSpan::enter_with_local_parent(this.name.clone());
        this.inner.poll(cx)
    }
}
