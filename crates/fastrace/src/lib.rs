// Copyright 2024 FastLabs Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This crate is derived from [1] under the original license header:
// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.
// [1]: https://github.com/tikv/minitrace-rust/blob/v0.6.4/minitrace/src/lib.rs

//! `fastrace` is a high-performance, ergonomic, library-level timeline tracing library for Rust.
//!
//! Unlike most tracing libraries which are primarily designed for instrumenting executables,
//! `fastrace` also accommodates the need for library instrumentation. It stands out due to
//! its extreme lightweight and fast performance compared to other tracing libraries. Moreover,
//! it has zero overhead when not enabled in the executable, making it a worry-free choice for
//! libraries concerned about unnecessary performance loss.
//!
//! # Getting Started
//!
//! ## In Libraries
//!
//! Libraries should include `fastrace` as a dependency without enabling any extra features.
//!
//! ```toml
//! [dependencies]
//! fastrace = "0.7"
//! ```
//!
//! Add a [`trace`] attribute to the function you want to trace. In this example, a
//! [`SpanRecord`] will be collected every time the function is called, if a tracing context
//! is set up by the caller.
//!
//! ```
//! # struct HttpRequest;
//! # struct Error;
//!
//! #[fastrace::trace]
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! Libraries are able to set up an individual tracing context, regardless of whether
//! the caller has set up a tracing context or not. This can be achieved by using
//! [`Span::root()`] to start a new trace and [`Span::set_local_parent()`] to set up a
//! local context for the current thread.
//!
//! The [`func_path!()`] macro can detect the function's full name, which is used as
//! the name of the root span.
//!
//! ```
//! # struct HttpRequest;
//! # struct Error;
//!
//! use fastrace::Span;
//! use fastrace::collector::SpanContext;
//! use fastrace::func_path;
//!
//! pub fn send_request(req: HttpRequest) -> Result<(), Error> {
//!     let root = Span::root(func_path!(), SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! ## In Applications
//!
//! Applications should include `fastrace` as a dependency with the `enable` feature
//! set. To disable `fastrace` statically, simply remove the `enable` feature.
//!
//! ```toml
//! [dependencies]
//! fastrace = { version ="0.7", features = ["enable"] }
//! ```
//!
//! Applications should initialize a [`Reporter`] implementation early in the program's runtime.
//! Span records generated before the reporter is initialized will be ignored. Before
//! terminating, [`flush()`] should be called to ensure all collected span records are reported.
//!
//! ```
//! use fastrace::Span;
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::collector::SpanContext;
//!
//! fn main() {
//!     fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//!     loop {
//!         let root = Span::root("worker-loop", SpanContext::random());
//!         let _guard = root.set_local_parent();
//!
//!         handle_request();
//!         # break;
//!     }
//!
//!     fastrace::flush();
//! }
//! # fn handle_request() {}
//! ```
//!
//! # Key Concepts
//!
//! `fastrace` operates through three types: [`Span`], [`LocalSpan`], and [`Event`], each
//! representing a different type of tracing record. The macro [`trace`] is available to
//! manage these types automatically. For [`Future`] instrumentation, necessary utilities
//! are provided by [`FutureExt`].
//!
//! ## Span
//!
//! A [`Span`] represents an individual unit of work. It contains:
//! - A name
//! - A start timestamp and duration
//! - A set of key-value properties
//! - A reference to a parent `Span`
//!
//! A new `Span` can be started through [`Span::root()`], requiring the trace id and the
//! parent span id from a remote source. If there's no remote parent span, the parent span
//! id is typically set to its default value of zero.
//!
//! Once we have the root `Span`, we can create a child `Span` using [`Span::enter_with_parent()`],
//! thereby establishing the reference relationship between the spans.
//!
//! `Span` is thread-safe and can be sent across threads.
//! ```
//! use fastrace::Span;
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::collector::SpanContext;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root_span = Span::root("root", SpanContext::random());
//!
//!     {
//!         let child_span = Span::enter_with_parent("a child span", &root_span);
//!
//!         // ...
//!
//!         // child_span ends here.
//!     }
//!
//!     // root_span ends here.
//! }
//!
//! fastrace::flush();
//! ```
//!
//! Sometimes, passing a `Span` through a function to create a child `Span` can be inconvenient.
//! We can employ a thread-local approach to avoid an explicit argument passing in the function.
//! In fastrace, [`Span::set_local_parent()`] and [`Span::enter_with_local_parent()`] serve this
//! purpose.
//!
//! [`Span::set_local_parent()`] method sets a local context of the `Span` for the current thread.
//! [`Span::enter_with_local_parent()`] accesses the parent `Span` from the local context and
//! creates a child `Span` with it.
//!
//! ```
//! use fastrace::Span;
//! use fastrace::collector::SpanContext;
//!
//! {
//!     let root_span = Span::root("root", SpanContext::random());
//!     let _guard = root_span.set_local_parent();
//!
//!     foo();
//!
//!     // root_span ends here.
//! }
//!
//! fn foo() {
//!     // The parent of this span is `root`.
//!     let _child_span = Span::enter_with_local_parent("a child span");
//!
//!     // ...
//!
//!     // _child_span ends here.
//! }
//! ```
//!
//! ## Local Span
//!
//! In a clear single-thread execution flow, where we can ensure that the `Span` does
//! not cross threads, meaning:
//! - The `Span` is not sent to or shared by other threads
//! - In asynchronous code, the lifetime of the `Span` doesn't cross an `.await` point
//!
//! we can use `LocalSpan` as a substitute for `Span` to effectively reduce overhead
//! and greatly enhance performance.
//!
//! However, there is a precondition: The creation of `LocalSpan` must take place
//! within a local context of a `Span`, which is established by invoking the
//! [`Span::set_local_parent()`] method.
//!
//! If the code spans multiple function calls, this isn't always straightforward to
//! confirm if the precondition is met. As such, it's recommended to invoke
//! [`Span::set_local_parent()`] immediately after the creation of `Span`.
//!
//! After a local context of a `Span` is set using [`Span::set_local_parent()`],
//! use [`LocalSpan::enter_with_local_parent()`] to start a `LocalSpan`, which then
//! becomes the new local parent.
//!
//! If no local context is set, the [`LocalSpan::enter_with_local_parent()`] will do nothing.
//!
//! ```
//! use fastrace::Span;
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::collector::SpanContext;
//! use fastrace::local::LocalSpan;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     {
//!         // The parent of this span is `root`.
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         foo();
//!     }
//! }
//!
//! fn foo() {
//!     // The parent of this span is `span1`.
//!     let _span2 = LocalSpan::enter_with_local_parent("a child span of child span");
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Event
//!
//! [`Event`] represents a single point in time where something occurred during the execution of a
//! program.
//!
//! An `Event` can be seen as a log record attached to a span.
//!
//! ```
//! use fastrace::Event;
//! use fastrace::Span;
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::collector::SpanContext;
//! use fastrace::local::LocalSpan;
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     root.add_event(Event::new("event in root"));
//!
//!     {
//!         let _span1 = LocalSpan::enter_with_local_parent("a child span");
//!
//!         LocalSpan::add_event(Event::new("event in span1"));
//!     }
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Macro
//!
//! The attribute-macro [`trace`] helps to reduce boilerplate.
//!
//! Note: For successful tracing a function using the [`trace`] macro, the function call should
//! occur within a local context of a `Span`.
//!
//! For more detailed usage instructions, please refer to [`trace`].
//!
//! ```
//! use fastrace::Span;
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//! use fastrace::collector::SpanContext;
//! use fastrace::future::FutureExt;
//!
//! #[fastrace::trace]
//! fn do_something(i: u64) {
//!     std::thread::sleep(std::time::Duration::from_millis(i));
//! }
//!
//! #[fastrace::trace]
//! async fn do_something_async(i: u64) {
//!     futures_timer::Delay::new(std::time::Duration::from_millis(i)).await;
//! }
//!
//! fastrace::set_reporter(ConsoleReporter, Config::default());
//!
//! {
//!     let root = Span::root("root", SpanContext::random());
//!     let _guard = root.set_local_parent();
//!
//!     do_something(100);
//!
//!     pollster::block_on(
//!         async {
//!             do_something_async(100).await;
//!         }
//!         .in_span(Span::enter_with_local_parent("async_job")),
//!     );
//! }
//!
//! fastrace::flush();
//! ```
//!
//! ## Reporter
//!
//! [`Reporter`] is responsible for reporting the span records to a remote agent.
//!
//! Executables should initialize a reporter at the very beginning of the program's lifetime.
//! Span records generated before the reporter is initialized will be ignored.
//!
//! For an easy start, `fastrace` offers a [`ConsoleReporter`] that prints span records to stderr.
//! For more advanced use, crates like `fastrace-opentelemetry` are available.
//!
//! The reporter runs in a background collector thread. [`Config::report_interval()`] controls the
//! *maximum* interval between report cycles, but the reporter may be invoked earlier. Do not rely
//! on it for precise scheduling or batching.
//!
//! ```
//! use std::time::Duration;
//!
//! use fastrace::collector::Config;
//! use fastrace::collector::ConsoleReporter;
//!
//! fastrace::set_reporter(
//!     ConsoleReporter,
//!     Config::default().report_interval(Duration::from_secs(1)),
//! );
//!
//! fastrace::flush();
//! ```
//!
//! # Performance
//!
//! `fastrace` is designed to be fast and lightweight, considering four scenarios:
//!
//! - **No Tracing**: If the feature `enable` is not set in the application, `fastrace` will be
//!   completely optimized away from the final executable binary, achieving zero overhead.
//!
//! - **Sample Tracing**: If `enable` is set in the application, but only a small portion of the
//!   traces are enabled via [`Span::root()`], while the other portions are started with
//!   placeholders using [`Span::noop()`]. The overhead in this case is very small - merely an
//!   integer load, comparison, and jump.
//!
//! - **Full Tracing with Tail Sampling**: If `enable` is set in the application, and all traces are
//!   enabled, however, only a select few interesting tracing records (e.g., P99) are reported,
//!   while normal traces are dismissed by using [`Span::cancel()`] to avoid being reported, the
//!   overhead of collecting traces is still very small. This could be useful when you are
//!   interested in examining program's tail latency.
//!
//! - **Full Tracing**: If `enable` is set in the application, and all traces are reported,
//!   `fastrace` performs 10x to 100x faster than other tracing libraries in this case.
//!
//! [`LocalSpan`]: local::LocalSpan
//! [`SpanRecord`]: collector::SpanRecord
//! [`FutureExt`]: future::FutureExt
//! [`LocalSpan::enter_with_local_parent()`]: local::LocalSpan::enter_with_local_parent
//! [`Reporter`]: collector::Reporter
//! [`ConsoleReporter`]: collector::ConsoleReporter
//! [`Config::report_interval()`]: collector::Config::report_interval

// Suppress a false-positive lint from clippy
#![allow(clippy::needless_doctest_main)]
#![cfg_attr(not(feature = "enable"), allow(dead_code))]
#![cfg_attr(not(feature = "enable"), allow(unused_mut))]
#![cfg_attr(not(feature = "enable"), allow(unused_imports))]
#![cfg_attr(not(feature = "enable"), allow(unused_variables))]
#![cfg_attr(target_family = "wasm", allow(dead_code))]

pub mod collector;
mod event;
pub mod future;
pub mod local;
mod macros;
mod span;
#[doc(hidden)]
pub mod util;

/// An attribute macro designed to eliminate boilerplate code.
///
/// This macro automatically creates a span for the annotated function. The span name defaults
/// to the full path of the function (use `short_name = true` to use only the function name),
/// but can be customized by passing a string literal via the `name` parameter.
///
/// The `#[fastrace::trace]` attribute requires a local parent context to function correctly.
/// Ensure that the function annotated with `#[fastrace::trace]` is called within a local
/// context of a `Span`, which is established by invoking the `Span::set_local_parent()`
/// method.
///
/// ## Arguments
///
/// * `name`: The name of the span. Defaults to the full path of the function.
/// * `short_name`: Whether to use the function name without path as the span name. Defaults to
///   `false`.
/// * `enter_on_poll`: Whether to enter the span on poll. If set to `false`, `in_span` will be
///   used. Only available for `async fn`. Defaults to `false`.
/// * `properties`: A list of key-value pairs to be added as properties to the span. The value
///   can be a format string, where the function arguments are accessible. Defaults to `{}`.
/// * `crate`: The path to the fastrace crate. Defaults to `::fastrace`.
///
/// # Examples
///
/// ```
/// #[fastrace::trace]
/// fn simple() {
///     // ...
/// }
///
/// #[fastrace::trace(short_name = true)]
/// async fn simple_async() {
///     // ...
/// }
///
/// #[fastrace::trace(name = "qux", enter_on_poll = true)]
/// async fn baz() {
///     // ...
/// }
///
/// #[fastrace::trace(properties = { "k1": "v1", "a": "argument `a` is {a:?}" })]
/// async fn properties(a: u64) {
///     // ...
/// }
/// ```
///
/// The code snippets above will be expanded to:
///
/// ```
/// use fastrace::Span;
/// use fastrace::future::FutureExt;
/// use fastrace::local::LocalSpan;
///
/// fn simple() {
///     let _g = LocalSpan::enter_with_local_parent("example::simple");
///     // ...
/// }
///
/// async fn simple_async() {
///     let span = Span::enter_with_local_parent("simple_async");
///     async {
///         // ...
///     }
///     .in_span(span)
///     .await
/// }
///
/// async fn baz() {
///     async {
///         // ...
///     }
///     .enter_on_poll("qux")
///     .await
/// }
///
/// async fn properties(a: u64) {
///     let span = Span::enter_with_local_parent("example::properties").with_properties(|| {
///         [
///             (std::borrow::Cow::from("k1"), std::borrow::Cow::from("v1")),
///             (
///                 std::borrow::Cow::from("a"),
///                 std::borrow::Cow::from(format!("argument `a` is {a:?}")),
///             ),
///         ]
///     });
///     async {
///         // ...
///     }
///     .in_span(span)
///     .await
/// }
/// ```
pub use fastrace_macro::trace;

pub use crate::collector::global_collector::flush;
pub use crate::collector::global_collector::set_reporter;
pub use crate::event::Event;
pub use crate::span::Span;

pub mod prelude {
    //! A "prelude" for crates using `fastrace`.
    #[doc(no_inline)]
    pub use crate::collector::SpanContext;
    #[doc(no_inline)]
    pub use crate::collector::SpanId;
    #[doc(no_inline)]
    pub use crate::collector::SpanRecord;
    #[doc(no_inline)]
    pub use crate::collector::TraceId;
    #[doc(no_inline)]
    pub use crate::collector::W3CTraceContext;
    #[doc(no_inline)]
    pub use crate::event::Event;
    #[doc(no_inline)]
    pub use crate::file_location;
    #[allow(deprecated)]
    #[doc(no_inline)]
    pub use crate::full_name;
    #[doc(no_inline)]
    pub use crate::func_name;
    #[doc(no_inline)]
    pub use crate::func_path;
    #[doc(no_inline)]
    pub use crate::future::FutureExt as _;
    #[doc(no_inline)]
    pub use crate::local::LocalSpan;
    #[doc(no_inline)]
    pub use crate::span::Span;
    #[doc(no_inline)]
    pub use crate::trace;
}
